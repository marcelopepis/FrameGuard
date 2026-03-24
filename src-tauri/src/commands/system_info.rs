// Informações do sistema (CPU, GPU, RAM, OS, status de features)
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::System;

use crate::utils::command_runner::run_command;
use crate::utils::registry::{subkey_exists, Hive};

// ─── Cache de hardware estático (nunca muda durante a sessão) ────────────────

/// Cache global para dados de CPU/RAM que não mudam durante a sessão.
static HW_CACHE: OnceLock<StaticHwInfo> = OnceLock::new();

/// Cache global para dados de GPU (pre-warm em setup(), query WMI lenta).
static GPU_CACHE: OnceLock<GpuInfo> = OnceLock::new();

/// Flag para evitar query dupla de GPU — `true` enquanto pre_warm ou get_gpu_info está coletando.
static GPU_COLLECTING: AtomicBool = AtomicBool::new(false);

/// Informações estáticas de CPU e RAM.
/// Cacheadas na primeira chamada — retornam em <100ms (sysinfo, sem PowerShell).
#[derive(Debug, Clone, Serialize)]
pub struct StaticHwInfo {
    /// Nome completo da CPU (ex: "Intel Core i9-13900K")
    pub cpu_name: String,
    /// Número de núcleos físicos
    pub cpu_cores: u32,
    /// Total de RAM instalada em GB
    pub ram_total_gb: f64,
}

/// Informações de GPU (nome + VRAM).
/// Cacheadas e pre-warmed em setup() para não bloquear a abertura do Dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    /// Nome da GPU principal (ex: "NVIDIA GeForce RTX 4080")
    pub gpu_name: String,
    /// VRAM da GPU em GB (0.0 se não disponível)
    pub gpu_vram_gb: f64,
}

/// Vendors de hardware detectados (GPU e CPU).
///
/// Derivado dos caches existentes (`HW_CACHE` e `GPU_CACHE`) — não faz
/// novas queries WMI. Usado pelo frontend para filtrar tweaks vendor-specific
/// e pelo backend para pular tweaks incompatíveis durante execução de planos.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedVendors {
    /// `"nvidia"`, `"amd"`, `"intel"` ou `"unknown"`
    pub gpu_vendor: String,
    /// `"intel"`, `"amd"` ou `"unknown"`
    pub cpu_vendor: String,
    /// Build number do Windows (ex: 22631 para Win11 23H2). >= 22000 = Windows 11.
    pub windows_build: u32,
}

/// Coleta dados estáticos de CPU e RAM (rápido, <100ms via sysinfo).
/// Resultado cacheado em memória — chamadas subsequentes retornam instantaneamente.
#[tauri::command]
pub async fn get_static_hw_info() -> Result<StaticHwInfo, String> {
    if let Some(cached) = HW_CACHE.get() {
        return Ok(cached.clone());
    }

    let info = tokio::task::spawn_blocking(|| {
        use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};

        // Cria System VAZIO e refresca APENAS CPU + RAM.
        // System::new_all() leva 3-8s (coleta processos, discos, rede); isto leva <200ms.
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        // Segundo refresh garante que brand() esteja populado em todas as versões do sysinfo
        sys.refresh_cpu_all();

        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "CPU Desconhecida".to_string());

        let cpu_cores = sys.physical_core_count().unwrap_or(0) as u32;

        let ram_total = sys.total_memory();
        let ram_total_gb = (ram_total as f64 / 1_073_741_824.0 * 10.0).round() / 10.0;

        StaticHwInfo {
            cpu_name,
            cpu_cores,
            ram_total_gb,
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let _ = HW_CACHE.set(info.clone());
    Ok(info)
}

/// Retorna dados da GPU (nome + VRAM). Pre-warmed em setup() via `pre_warm_gpu_cache`.
/// Se o pre-warm está em andamento, aguarda o resultado em vez de iniciar query duplicada.
#[tauri::command]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    if let Some(cached) = GPU_CACHE.get() {
        return Ok(cached.clone());
    }

    // Se o pre-warm já está coletando, espera o resultado dele
    if GPU_COLLECTING.load(Ordering::Acquire) {
        for _ in 0..80 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(cached) = GPU_CACHE.get() {
                return Ok(cached.clone());
            }
        }
    }

    // Fallback: coleta diretamente (pre-warm não rodou ou falhou)
    if GPU_COLLECTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let info = tokio::task::spawn_blocking(collect_gpu_info)
            .await
            .map_err(|e| e.to_string())?;

        let _ = GPU_CACHE.set(info.clone());
        GPU_COLLECTING.store(false, Ordering::Release);
        Ok(info)
    } else {
        // Outra task começou a coletar enquanto esperávamos — aguarda
        for _ in 0..80 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(cached) = GPU_CACHE.get() {
                return Ok(cached.clone());
            }
        }
        Err("Timeout aguardando dados de GPU".to_string())
    }
}

/// Detecta vendors de GPU e CPU a partir dos nomes no cache.
///
/// Retorna imediatamente se os caches já estiverem prontos.
/// Caso contrário, aguarda a coleta (CPU ~100ms, GPU 2-4s — normalmente já pre-warmed).
#[tauri::command]
pub async fn get_detected_vendors() -> Result<DetectedVendors, String> {
    let hw = get_static_hw_info().await?;
    let gpu = get_gpu_info().await?;

    let gpu_lower = gpu.gpu_name.to_lowercase();
    let gpu_vendor = if gpu_lower.contains("nvidia") || gpu_lower.contains("geforce") {
        "nvidia"
    } else if gpu_lower.contains("amd") || gpu_lower.contains("radeon") {
        "amd"
    } else if gpu_lower.contains("intel")
        && (gpu_lower.contains("arc") || gpu_lower.contains("iris") || gpu_lower.contains("uhd"))
    {
        "intel"
    } else {
        "unknown"
    };

    let cpu_lower = hw.cpu_name.to_lowercase();
    let cpu_vendor = if cpu_lower.contains("intel") || cpu_lower.contains("core") {
        "intel"
    } else if cpu_lower.contains("amd")
        || cpu_lower.contains("ryzen")
        || cpu_lower.contains("threadripper")
    {
        "amd"
    } else {
        "unknown"
    };

    // Ler CurrentBuildNumber do registro (rápido, <1ms)
    let windows_build: u32 = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(0);

    Ok(DetectedVendors {
        gpu_vendor: gpu_vendor.to_string(),
        cpu_vendor: cpu_vendor.to_string(),
        windows_build,
    })
}

/// Detecção síncrona de vendor de GPU a partir do cache OnceLock.
/// Retorna `"nvidia"`, `"amd"`, `"intel"` ou `"unknown"`.
/// Se o cache ainda não estiver populado (pre-warm pendente), retorna `"unknown"`.
pub(crate) fn detect_gpu_vendor_sync() -> String {
    let gpu_name = GPU_CACHE
        .get()
        .map(|g| g.gpu_name.to_lowercase())
        .unwrap_or_default();

    if gpu_name.contains("nvidia") || gpu_name.contains("geforce") {
        "nvidia".to_string()
    } else if gpu_name.contains("amd") || gpu_name.contains("radeon") {
        "amd".to_string()
    } else if gpu_name.contains("intel")
        && (gpu_name.contains("arc") || gpu_name.contains("iris") || gpu_name.contains("uhd"))
    {
        "intel".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Verifica se o sistema tem CPU AMD com fTPM ativo e retorna aviso se detectado.
///
/// Retorna `Some(mensagem)` se AMD + TPM ativo, `None` caso contrário.
/// O fTPM em processadores AMD pode causar micro-stutters intermitentes em jogos
/// se o BIOS não estiver atualizado com AGESA 1207+.
#[tauri::command]
pub async fn get_ftpm_warning() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| {
        // 1. Verificar se CPU é AMD (via cache)
        let cpu_name = HW_CACHE
            .get()
            .map(|h| h.cpu_name.to_lowercase())
            .unwrap_or_default();

        let is_amd = cpu_name.contains("amd")
            || cpu_name.contains("ryzen")
            || cpu_name.contains("threadripper");

        if !is_amd {
            return Ok(None);
        }

        // 2. Verificar se TPM está ativo via registry
        // IntegrityServices\WBCL existe quando TPM está habilitado e coletando logs
        let tpm_active = subkey_exists(
            Hive::LocalMachine,
            r"SYSTEM\CurrentControlSet\Control\IntegrityServices",
        )
        .unwrap_or(false);

        if !tpm_active {
            return Ok(None);
        }

        Ok(Some(
            "Seu processador AMD usa fTPM (Firmware TPM), que pode causar micro-stutters \
            intermitentes em jogos. Verifique se há atualização de BIOS disponível para \
            sua placa-mãe com AGESA 1207 ou superior."
                .to_string(),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pre-warm de todos os caches estáticos em background.
/// Chamado no setup do Tauri — não bloqueia a abertura da janela.
/// Dashboard mostra skeletons enquanto os dados são coletados.
pub fn pre_warm_all_caches() {
    // GPU: registro direto, ~50ms
    if GPU_CACHE.get().is_none()
        && GPU_COLLECTING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        tauri::async_runtime::spawn(async {
            let info = tokio::task::spawn_blocking(collect_gpu_info)
                .await
                .unwrap_or(GpuInfo {
                    gpu_name: "GPU Desconhecida".to_string(),
                    gpu_vram_gb: 0.0,
                });
            let _ = GPU_CACHE.set(info);
            GPU_COLLECTING.store(false, Ordering::Release);
        });
    }

    // CPU + RAM: sysinfo targeted, ~200ms
    if HW_CACHE.get().is_none() {
        tauri::async_runtime::spawn(async {
            let _ = get_static_hw_info().await;
        });
    }

    // System summary: registry + hostname, <100ms
    if SUMMARY_CACHE.get().is_none() {
        tauri::async_runtime::spawn(async {
            let _ = get_system_summary().await;
        });
    }

    // System status: registry + powercfg direto, ~100ms
    tauri::async_runtime::spawn(async {
        let _ = get_system_status().await;
    });
}

/// Coleta dados de GPU via registro do Windows (leitura direta, <50ms).
/// Itera subkeys do driver de vídeo e seleciona a GPU com mais VRAM (dedicada > integrada).
/// Fallback para wmic via cmd se o registro não retornar dados.
fn collect_gpu_info() -> GpuInfo {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_path = r"SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

    let mut best_name = String::new();
    let mut best_vram: u64 = 0;

    // Itera subkeys 0000, 0001, 0002... (cada uma é um adaptador de vídeo)
    if let Ok(class_key) = hklm.open_subkey(class_path) {
        for subkey_name in class_key.enum_keys().filter_map(|k| k.ok()) {
            if let Ok(adapter) = class_key.open_subkey(&subkey_name) {
                // Lê nome da GPU
                let name: String = adapter
                    .get_value("DriverDesc")
                    .or_else(|_| adapter.get_value("HardwareInformation.AdapterString"))
                    .unwrap_or_default();

                if name.is_empty() {
                    continue;
                }

                // Lê VRAM (qwMemorySize é QWORD = REG_QWORD = u64)
                let vram: u64 = adapter
                    .get_value("HardwareInformation.qwMemorySize")
                    .unwrap_or(0u64);

                // Fallback para MemorySize (u32, trava em ~4GB para GPUs maiores)
                let vram = if vram == 0 {
                    adapter
                        .get_value::<u32, _>("HardwareInformation.MemorySize")
                        .map(|v| v as u64)
                        .unwrap_or(0)
                } else {
                    vram
                };

                // Prioriza GPU com mais VRAM (=GPU dedicada, não integrada)
                if vram > best_vram || (best_name.is_empty() && !name.is_empty()) {
                    best_name = name;
                    best_vram = vram;
                }
            }
        }
    }

    // Fallback: se o registro não retornou nada, tenta wmic via cmd (sem PowerShell)
    if best_name.is_empty() {
        if let Ok(output) = run_command(
            "cmd.exe",
            &[
                "/c",
                "wmic",
                "path",
                "Win32_VideoController",
                "get",
                "Name",
                "/value",
            ],
        ) {
            if let Some(line) = output.stdout.lines().find(|l| l.starts_with("Name=")) {
                best_name = line.trim_start_matches("Name=").trim().to_string();
            }
        }
    }

    GpuInfo {
        gpu_name: if best_name.is_empty() {
            "GPU Desconhecida".to_string()
        } else {
            best_name
        },
        gpu_vram_gb: if best_vram > 0 {
            (best_vram as f64 / 1_073_741_824.0 * 10.0).round() / 10.0
        } else {
            0.0
        },
    }
}

// ─── Status do sistema (cache com TTL de 5 s) ──────────────────────────────

/// Cache com TTL para SystemStatus — evita re-executar PowerShell a cada chamada.
struct CachedStatus {
    data: SystemStatus,
    fetched_at: Instant,
}

static STATUS_CACHE: OnceLock<Mutex<Option<CachedStatus>>> = OnceLock::new();

/// TTL do cache de status em segundos.
const STATUS_TTL_SECS: u64 = 5;

/// Status de configurações do Windows relevantes para gaming.
/// Cacheado com TTL de 5 s — a primeira chamada pode levar 500-1000 ms (powercfg),
/// chamadas dentro do TTL retornam instantaneamente.
#[derive(Debug, Clone, Serialize)]
pub struct SystemStatus {
    /// `true` se o Windows Game Mode está ativo
    pub game_mode_enabled: bool,
    /// `true` se HAGS (Hardware-Accelerated GPU Scheduling) está ativo
    pub hags_enabled: bool,
    /// `true` se VBS (Virtualization Based Security) está ativo
    pub vbs_enabled: bool,
    /// Status do Game DVR: "disabled", "available" ou "recording"
    pub game_dvr_status: String,
    /// Nome do plano de energia ativo (ex: "Desempenho Máximo")
    pub power_plan_name: String,
    /// Tier do plano: "ultimate", "high" ou "other"
    pub power_plan_tier: String,
    /// `true` se o timer de alta resolução (1 ms) está ativo
    pub timer_resolution_optimized: bool,
}

/// Lê status de configurações do Windows (registro + powercfg).
/// Resultado cacheado por 5 s — evita re-executar PowerShell em navegação rápida.
#[tauri::command]
pub async fn get_system_status() -> Result<SystemStatus, String> {
    // Verifica cache TTL antes de gastar CPU
    let cache = STATUS_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(ref cached) = *guard {
            if cached.fetched_at.elapsed().as_secs() < STATUS_TTL_SECS {
                return Ok(cached.data.clone());
            }
        }
    }

    let status = tokio::task::spawn_blocking(|| {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Game Mode: HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled
        let game_mode_enabled = hkcu
            .open_subkey(r"Software\Microsoft\GameBar")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("AutoGameModeEnabled").ok())
            .map(|v| v != 0)
            .unwrap_or(true);

        // HAGS: HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode (2 = ativo)
        let hags_enabled = hklm
            .open_subkey(r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("HwSchMode").ok())
            .map(|v| v == 2)
            .unwrap_or(true);

        // VBS: HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\EnableVirtualizationBasedSecurity
        let vbs_enabled = hklm
            .open_subkey(r"SYSTEM\CurrentControlSet\Control\DeviceGuard")
            .ok()
            .and_then(|k| {
                k.get_value::<u32, _>("EnableVirtualizationBasedSecurity")
                    .ok()
            })
            .map(|v| v != 0)
            .unwrap_or(false);

        // Game DVR: 3 chaves relevantes para status no Dashboard
        let dvr_enabled = hkcu
            .open_subkey(r"System\GameConfigStore")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("GameDVR_Enabled").ok())
            .unwrap_or(1);
        let historical = hkcu
            .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("HistoricalCaptureEnabled").ok())
            .unwrap_or(0);
        let policy_dvr: Option<u32> = hklm
            .open_subkey(r"SOFTWARE\Policies\Microsoft\Windows\GameDVR")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("AllowGameDVR").ok());

        let game_dvr_status = if policy_dvr == Some(0) || dvr_enabled == 0 {
            "disabled"
        } else if historical == 1 {
            "recording"
        } else {
            "available"
        }
        .to_string();

        // Power Plan: detecção por GUID (independente de idioma do Windows)
        // Chama powercfg.exe diretamente — SEM wrapper PowerShell.
        // powercfg.exe roda em ~50ms; via PowerShell levava 1-3s (cold start do interpretador).
        let powercfg_output = run_command("powercfg.exe", &["/getactivescheme"])
            .ok()
            .map(|o| o.stdout.clone())
            .unwrap_or_default();
        let powercfg_lower = powercfg_output.to_lowercase();

        let power_plan_tier = if powercfg_lower.contains("e9a42b02-d5df-448d-aa00-03f14749eb61") {
            "ultimate"
        } else if powercfg_lower.contains("8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c") {
            "high"
        } else {
            "other"
        }
        .to_string();

        let power_plan_name = powercfg_output
            .rfind('(')
            .and_then(|start| {
                powercfg_output[start + 1..].find(')').map(|end| {
                    powercfg_output[start + 1..start + 1 + end]
                        .trim()
                        .to_string()
                })
            })
            .unwrap_or_else(|| "Desconhecido".to_string());

        // Timer Resolution: HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\kernel
        let timer_resolution_optimized = hklm
            .open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("GlobalTimerResolutionRequests").ok())
            .map(|v| v == 1)
            .unwrap_or(false);

        Ok::<SystemStatus, String>(SystemStatus {
            game_mode_enabled,
            hags_enabled,
            vbs_enabled,
            game_dvr_status,
            power_plan_name,
            power_plan_tier,
            timer_resolution_optimized,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))?;

    // Atualiza o cache
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(CachedStatus {
            data: status.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(status)
}

// ─── Uso atual de CPU e RAM (polling periódico) ──────────────────────────────

/// Instância persistente de `System` — evita recriação cara a cada poll.
static SYS_USAGE: OnceLock<Mutex<System>> = OnceLock::new();

/// Informações de uso atual de CPU e RAM (para polling periódico do dashboard).
#[derive(Debug, Serialize)]
pub struct SystemUsage {
    pub cpu_usage_percent: f32,
    pub ram_usage_percent: f32,
}

/// Retorna o uso atual de CPU e RAM.
/// Usa instância persistente de `System` — muito mais leve que criar `System::new_all()` a cada chamada.
/// A primeira chamada faz sleep(200ms) para baseline; chamadas subsequentes usam o delta
/// natural do intervalo de polling do frontend (2s).
#[tauri::command]
pub async fn get_system_usage() -> Result<SystemUsage, String> {
    tokio::task::spawn_blocking(|| {
        let sys_mutex = SYS_USAGE.get_or_init(|| {
            let mut sys = System::new();
            sys.refresh_cpu_usage();
            std::thread::sleep(std::time::Duration::from_millis(200));
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            Mutex::new(sys)
        });

        // try_lock para não bloquear se outra chamada estiver em andamento
        let mut sys = match sys_mutex.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                // Outra medição em andamento — retorna defaults
                return Ok(SystemUsage {
                    cpu_usage_percent: 0.0,
                    ram_usage_percent: 0.0,
                });
            }
        };

        // Refresh SEM sleep — usa o delta desde a última medição.
        // Como o frontend chama a cada 2s, o delta é suficiente
        // para uma leitura precisa de CPU usage.
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_usage_percent = sys.global_cpu_usage();
        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        let ram_usage_percent = if ram_total > 0 {
            (ram_used as f32 / ram_total as f32) * 100.0
        } else {
            0.0
        };

        Ok::<SystemUsage, String>(SystemUsage {
            cpu_usage_percent,
            ram_usage_percent,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ─── Resumo do sistema (cacheado — nunca muda na sessão) ────────────────────

/// Cache global para dados do sistema que não mudam durante a sessão.
static SUMMARY_CACHE: OnceLock<SystemSummary> = OnceLock::new();

/// Dados essenciais do sistema (OS, hostname, elevação).
/// Cacheados na primeira chamada — retornam instantaneamente nas chamadas seguintes.
#[derive(Debug, Clone, Serialize)]
pub struct SystemSummary {
    pub os_version: String,
    pub hostname: String,
    pub is_elevated: bool,
}

/// Retorna resumo do sistema: versão do Windows, hostname e status de elevação.
/// Async com spawn_blocking para não bloquear a main thread do Tauri (evita freeze ao mover janela).
#[tauri::command]
pub async fn get_system_summary() -> Result<SystemSummary, String> {
    if let Some(cached) = SUMMARY_CACHE.get() {
        return Ok(cached.clone());
    }

    let info = tokio::task::spawn_blocking(|| {
        let os_version = get_windows_version().unwrap_or_else(|_| {
            sysinfo::System::long_os_version().unwrap_or_else(|| "Windows 11".to_string())
        });

        let hostname = sysinfo::System::host_name().unwrap_or_else(|| {
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Desconhecido".to_string())
        });

        let is_elevated = crate::utils::elevated::is_elevated();

        SystemSummary {
            os_version,
            hostname,
            is_elevated,
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let _ = SUMMARY_CACHE.set(info.clone());
    Ok(info)
}

/// Lê a versão do Windows pelo registro.
/// Retorna ex: "Windows 11 Pro 23H2"
fn get_windows_version() -> Result<String, String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .map_err(|e| e.to_string())?;

    let mut product: String = key
        .get_value("ProductName")
        .unwrap_or_else(|_| "Windows".to_string());

    // A Microsoft não atualizou ProductName no registro do Windows 11.
    // Muitas instalações ainda retornam "Windows 10 Pro" mesmo sendo Win11.
    // CurrentBuildNumber >= 22000 = Windows 11.
    let build_number: String = key.get_value("CurrentBuildNumber").unwrap_or_default();
    let build_num: u32 = build_number.trim().parse().unwrap_or(0);

    if build_num >= 22000 && product.contains("Windows 10") {
        product = product.replace("Windows 10", "Windows 11");
    }

    // DisplayVersion contém o canal de lançamento (ex: "23H2", "24H2")
    let display_ver: String = key.get_value("DisplayVersion").unwrap_or_default();

    Ok(if display_ver.is_empty() {
        product
    } else {
        format!("{} {}", product, display_ver)
    })
}
