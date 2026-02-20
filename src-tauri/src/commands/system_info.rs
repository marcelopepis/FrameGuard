// Informações do sistema (CPU, GPU, RAM, OS, status de features)
use serde::Serialize;
use sysinfo::System;

use crate::utils::command_runner::run_command;

/// Informações completas de hardware e status do sistema para o dashboard.
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    /// Nome completo da CPU (ex: "Intel Core i9-13900K")
    pub cpu_name: String,
    /// Número de núcleos físicos
    pub cpu_cores: u32,
    /// Uso atual de CPU em percentual (0–100)
    pub cpu_usage_percent: f32,
    /// Total de RAM instalada em GB
    pub ram_total_gb: f64,
    /// RAM em uso no momento em GB
    pub ram_used_gb: f64,
    /// Percentual de RAM em uso (0–100)
    pub ram_usage_percent: f32,
    /// Nome da GPU principal (ex: "NVIDIA GeForce RTX 4080")
    pub gpu_name: String,
    /// VRAM da GPU em GB (0.0 se não disponível)
    pub gpu_vram_gb: f64,
    /// `true` se o Windows Game Mode está ativo (HKCU\Software\Microsoft\GameBar)
    pub game_mode_enabled: bool,
    /// `true` se HAGS (Hardware-Accelerated GPU Scheduling) está ativo
    pub hags_enabled: bool,
    /// `true` se VBS (Virtualization Based Security) está ativo
    pub vbs_enabled: bool,
}

/// Coleta todas as informações de hardware e status do sistema.
///
/// A coleta é executada em uma thread bloqueante separada (`spawn_blocking`)
/// para não bloquear o event loop do Tauri. Inclui:
/// - CPU: nome e uso atual via sysinfo (com medição de delta de 200 ms)
/// - RAM: uso atual via sysinfo
/// - GPU: nome via PowerShell / Win32_VideoController (prioriza maior VRAM)
/// - Status: Game Mode, HAGS e VBS via registro do Windows
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    tokio::task::spawn_blocking(|| {
        // ── CPU e RAM via sysinfo ──────────────────────────────────────────────
        // new_all() faz o primeiro snapshot de CPU; precisamos de um segundo
        // snapshot após um intervalo para calcular o uso com precisão.
        let mut sys = System::new_all();
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "CPU Desconhecida".to_string());

        let cpu_cores = sys.physical_core_count().unwrap_or(0) as u32;
        let cpu_usage_percent = sys.global_cpu_usage();

        let ram_total = sys.total_memory();
        let ram_used = sys.used_memory();
        // Arredonda para 1 casa decimal
        let ram_total_gb = (ram_total as f64 / 1_073_741_824.0 * 10.0).round() / 10.0;
        let ram_used_gb = (ram_used as f64 / 1_073_741_824.0 * 10.0).round() / 10.0;
        let ram_usage_percent = if ram_total > 0 {
            (ram_used as f32 / ram_total as f32) * 100.0
        } else {
            0.0
        };

        // ── GPU via PowerShell ─────────────────────────────────────────────────
        // Win32_VideoController.AdapterRAM é uint32 → trava em ~4 GB para placas maiores.
        // Lê HardwareInformation.qwMemorySize (QWORD 64-bit) da chave de registro do driver.
        // Retorna "Nome|vram_bytes". Fallback para AdapterRAM se a chave não for encontrada.
        let gpu_raw = run_command(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                 $g = Get-WmiObject Win32_VideoController | Sort-Object AdapterRAM -Descending | Select-Object -First 1; \
                 $vram = (Get-ChildItem 'HKLM:\\SYSTEM\\ControlSet001\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}' \
                   -ErrorAction SilentlyContinue | ForEach-Object { \
                     $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue; \
                     if ($p.DriverDesc -eq $g.Name) { $p.'HardwareInformation.qwMemorySize' } \
                   } | Where-Object { $_ } | Select-Object -First 1); \
                 if (-not $vram) { $vram = [uint64]$g.AdapterRAM }; \
                 \"$($g.Name)|$vram\"",
            ],
        )
        .ok()
        .filter(|r| r.success && !r.stdout.trim().is_empty())
        .map(|r| r.stdout.trim().to_string())
        .unwrap_or_default();

        let mut parts = gpu_raw.splitn(2, '|');
        let gpu_name = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "GPU Desconhecida".to_string());
        let gpu_vram_gb = parts
            .next()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|bytes| (bytes as f64 / 1_073_741_824.0 * 10.0).round() / 10.0)
            .unwrap_or(0.0);

        // ── Status via registro ────────────────────────────────────────────────
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Game Mode: HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled
        // Padrão: ativo (desde Windows 10 Creators Update — chave pode não existir)
        let game_mode_enabled = hkcu
            .open_subkey(r"Software\Microsoft\GameBar")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("AutoGameModeEnabled").ok())
            .map(|v| v != 0)
            .unwrap_or(true);

        // HAGS: HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode (2 = ativo)
        // Padrão: ativo no Windows 11 para GPUs compatíveis — chave só existe se alterado
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

        Ok::<SystemInfo, String>(SystemInfo {
            cpu_name,
            cpu_cores,
            cpu_usage_percent,
            ram_total_gb,
            ram_used_gb,
            ram_usage_percent,
            gpu_name,
            gpu_vram_gb,
            game_mode_enabled,
            hags_enabled,
            vbs_enabled,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

/// Informações de uso atual de CPU e RAM (para polling periódico do dashboard).
#[derive(Debug, Serialize)]
pub struct SystemUsage {
    pub cpu_usage_percent: f32,
    pub ram_usage_percent: f32,
}

/// Retorna o uso atual de CPU e RAM com medição de delta de 200 ms.
#[tauri::command]
pub async fn get_system_usage() -> Result<SystemUsage, String> {
    tokio::task::spawn_blocking(|| {
        let mut sys = System::new_all();
        std::thread::sleep(std::time::Duration::from_millis(200));
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

// ─── Resumo do sistema ────────────────────────────────────────────────────────

/// Dados essenciais do sistema para prova de conceito frontend↔backend
#[derive(Debug, Serialize)]
pub struct SystemSummary {
    pub os_version: String,
    pub hostname: String,
    pub is_elevated: bool,
}

/// Retorna resumo do sistema: versão do Windows, hostname e status de elevação
#[tauri::command]
pub fn get_system_summary() -> Result<SystemSummary, String> {
    let os_version = get_windows_version()
        // Fallback via sysinfo se o registro falhar
        .unwrap_or_else(|_| {
            sysinfo::System::long_os_version()
                .unwrap_or_else(|| "Windows 11".to_string())
        });

    let hostname = sysinfo::System::host_name()
        // Fallback via variável de ambiente do Windows
        .unwrap_or_else(|| {
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Desconhecido".to_string())
        });

    let is_elevated = crate::utils::elevated::is_elevated();

    Ok(SystemSummary {
        os_version,
        hostname,
        is_elevated,
    })
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

    let product: String = key
        .get_value("ProductName")
        .unwrap_or_else(|_| "Windows".to_string());

    // DisplayVersion contém o canal de lançamento (ex: "23H2")
    let display_ver: String = key.get_value("DisplayVersion").unwrap_or_default();

    Ok(if display_ver.is_empty() {
        product
    } else {
        format!("{} {}", product, display_ver)
    })
}
