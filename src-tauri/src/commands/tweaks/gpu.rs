//! Tweaks de GPU: HAGS, MSI Mode, MPO, NVIDIA Telemetry, PowerMizer, Overlay.
//!
//! Cada tweak segue o padrão `TweakMeta` + `build()` para separar metadados
//! estáticos do estado dinâmico do sistema.

use serde_json::{json, Value};
use winreg::enums::KEY_READ;

use crate::commands::optimizations::{
    restore_multi_entries, EvidenceLevel, HardwareFilter, RiskLevel, TweakInfo,
};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::command_runner::run_powershell;
use crate::utils::registry::{
    delete_subkey_all, delete_value, key_exists, read_binary, read_dword, read_string, write_binary,
    write_dword, write_string, Hive,
};
use crate::utils::tweak_builder::TweakMeta;

// ─── Helper: enumeração de subkeys de GPU por vendor ─────────────────────────

/// Base path da classe de Display Adapters no registro.
const GPU_CLASS_PATH: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Enumera as subkeys de GPU (0000, 0001, ...) e retorna o path completo da
/// primeira cuja `DriverDesc` contenha `vendor_substring` (case-insensitive).
///
/// Exemplo: `find_gpu_registry_subkey("NVIDIA")` retorna algo como
/// `SYSTEM\CurrentControlSet\Control\Class\{4d36e968-...}\0000`.
fn find_gpu_registry_subkey(vendor_substring: &str) -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let class_key = hklm.open_subkey_with_flags(GPU_CLASS_PATH, KEY_READ).ok()?;
    let needle = vendor_substring.to_ascii_uppercase();

    for subkey_name in class_key.enum_keys().filter_map(|r| r.ok()) {
        // Subkeys válidas são numéricas: 0000, 0001, etc.
        if !subkey_name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let subkey = class_key.open_subkey_with_flags(&subkey_name, KEY_READ).ok()?;

        // Tentar DriverDesc primeiro, depois HardwareInformation.AdapterString
        let desc: Option<String> = subkey
            .get_value::<String, _>("DriverDesc")
            .ok()
            .or_else(|| subkey.get_value::<String, _>("HardwareInformation.AdapterString").ok());

        if let Some(d) = desc {
            if d.to_ascii_uppercase().contains(&needle) {
                return Some(format!(r"{}\{}", GPU_CLASS_PATH, subkey_name));
            }
        }
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — HAGS (Hardware-Accelerated GPU Scheduling)
//
// HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode
//   2 = habilitado (padrão no Windows 11 para GPUs compatíveis)
//   0 = desabilitado
// ═══════════════════════════════════════════════════════════════════════════════

const HAGS_REG_PATH: &str = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
const HAGS_REG_KEY: &str = "HwSchMode";
const HAGS_ENABLED_VALUE: u32 = 2;
const HAGS_DISABLED_VALUE: u32 = 0;

static HAGS_META: TweakMeta = TweakMeta {
    id: "enable_hags",
    name: "Hardware-Accelerated GPU Scheduling (HAGS)",
    description: "Permite que a GPU gerencie sua própria memória de vídeo diretamente, \
        reduzindo a latência de renderização e a carga sobre a CPU. Recomendado para gaming.",
    category: "gamer",
    requires_restart: true,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description: "Padrão Windows 11: HAGS ativo (HwSchMode = 2)",
    hardware_filter: None,
};

/// Retorna as informações do tweak HAGS com o estado atual do registro.
///
/// Lê `HwSchMode` em `HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers`.
/// Ausência da chave assume HAGS ativo (padrão Windows 11).
#[tauri::command]
pub async fn get_hags_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_enabled = read_dword(Hive::LocalMachine, HAGS_REG_PATH, HAGS_REG_KEY)?
            .map(|v| v == HAGS_ENABLED_VALUE)
            .unwrap_or(true);

        Ok(HAGS_META.build(is_enabled))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Habilita HAGS definindo `HwSchMode = 2` no registro.
#[tauri::command]
pub fn enable_hags() -> Result<(), String> {
    write_dword(
        Hive::LocalMachine,
        HAGS_REG_PATH,
        HAGS_REG_KEY,
        HAGS_ENABLED_VALUE,
    )
}

/// Desabilita HAGS definindo `HwSchMode = 0` no registro.
#[tauri::command]
pub fn disable_hags() -> Result<(), String> {
    write_dword(
        Hive::LocalMachine,
        HAGS_REG_PATH,
        HAGS_REG_KEY,
        HAGS_DISABLED_VALUE,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Habilitar MSI Mode para GPU NVIDIA (+ HD Audio)
//
// Detecta o InstanceId da GPU NVIDIA e do HD Audio Controller associado via
// Get-PnpDevice e configura:
//   HKLM\SYSTEM\CurrentControlSet\Enum\{InstanceId}\Device Parameters\
//       Interrupt Management\MessageSignaledInterruptProperties -> MSISupported = 1
//
// GPUs RTX 40+ já usam MSI por padrão — o tweak detecta isso e reporta
// "já aplicado". Benefício principal em RTX 30 e anteriores.
// Também aplica MSI no HD Audio Controller NVIDIA (corrige crackling HDMI).
// Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

static MSI_MODE_META: TweakMeta = TweakMeta {
    id: "enable_msi_mode_gpu",
    name: "Habilitar MSI Mode para GPU (+ HD Audio)",
    description: "Habilita Message Signaled Interrupts para a GPU e o HD Audio Controller \
        NVIDIA, reduzindo latência de DPC e corrigindo crackling de áudio HDMI. GPUs RTX 40+ \
        já usam MSI por padrão — nesse caso o tweak reporta como já aplicado. Benefício \
        principal em GPUs RTX 30 e anteriores.",
    category: "gpu_display",
    requires_restart: true,
    risk_level: RiskLevel::Medium,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: MSI Mode desabilitado para GPU (MSISupported ausente ou 0)",
    hardware_filter: None,
};

/// Busca InstanceIds de dispositivos NVIDIA: GPU (Display) e HD Audio.
/// Retorna um vetor de (InstanceId, tipo) encontrados.
fn get_nvidia_msi_targets() -> Result<Vec<(String, String)>, String> {
    let output = run_powershell(
        "Get-PnpDevice | Where-Object { \
            $_.FriendlyName -like '*NVIDIA*' -and ($_.Class -eq 'Display' -or $_.Class -eq 'MEDIA') \
        } | Select-Object -Property InstanceId, Class | ForEach-Object { \
            \"$($_.Class)|$($_.InstanceId)\" \
        }",
    )?;

    let mut targets = Vec::new();
    for line in output.stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((class, id)) = trimmed.split_once('|') {
            targets.push((id.to_string(), class.to_string()));
        }
    }
    Ok(targets)
}

/// Monta o caminho de registro MSI Interrupt Properties para o InstanceId fornecido.
fn msi_reg_path(instance_id: &str) -> String {
    format!(
        r"SYSTEM\CurrentControlSet\Enum\{}\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties",
        instance_id
    )
}

/// Verifica se MSI Mode está habilitado para o dispositivo com o InstanceId fornecido.
fn get_msi_mode_is_applied_for(instance_id: &str) -> Result<bool, String> {
    let val = read_dword(
        Hive::LocalMachine,
        &msi_reg_path(instance_id),
        "MSISupported",
    )?
    .unwrap_or(0);
    Ok(val == 1)
}

/// Retorna informações do tweak MSI Mode GPU com estado atual.
///
/// Detecta automaticamente a GPU NVIDIA (e HD Audio) via PowerShell e verifica
/// `MSISupported`. GPUs RTX 40+ que já estão em MSI por padrão reportam como
/// "já aplicado". Se nenhuma GPU NVIDIA estiver presente, `is_applied` retorna `false`.
#[tauri::command]
pub async fn get_msi_mode_gpu_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let targets = get_nvidia_msi_targets()?;

        // Considerar "aplicado" se TODOS os dispositivos NVIDIA encontrados estão em MSI
        let is_applied = if targets.is_empty() {
            false
        } else {
            targets
                .iter()
                .all(|(id, _)| get_msi_mode_is_applied_for(id).unwrap_or(false))
        };

        Ok(MSI_MODE_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Habilita MSI Mode na GPU NVIDIA e HD Audio Controller detectados automaticamente.
///
/// Detecta o estado atual antes de modificar — se o dispositivo já estiver em MSI
/// (comum em RTX 40+), inclui no backup mas não re-aplica.
/// Os caminhos de registro dinâmicos são salvos no backup para reversão segura.
#[tauri::command]
pub fn enable_msi_mode_gpu() -> Result<(), String> {
    let targets = get_nvidia_msi_targets()?;
    if targets.is_empty() {
        return Err("Nenhuma GPU NVIDIA detectada — MSI Mode não pode ser configurado".to_string());
    }

    // Verificar se TODOS os dispositivos já estão em MSI
    let all_applied = targets
        .iter()
        .all(|(id, _)| get_msi_mode_is_applied_for(id).unwrap_or(false));
    if all_applied {
        return Err("Tweak 'enable_msi_mode_gpu' já está aplicado".to_string());
    }

    // Construir backup multi-entry com todos os dispositivos
    let backup_entries: Vec<Value> = targets
        .iter()
        .map(|(id, class)| {
            let reg_path = msi_reg_path(id);
            let original = read_dword(Hive::LocalMachine, &reg_path, "MSISupported")
                .ok()
                .flatten()
                .map(|v| json!(v))
                .unwrap_or(Value::Null);
            json!({
                "hive": "HKLM",
                "path": reg_path,
                "key": "MSISupported",
                "value": original,
                "device_class": class
            })
        })
        .collect();

    backup_before_apply(
        "enable_msi_mode_gpu",
        TweakCategory::Registry,
        "MSI Mode GPU + HD Audio — MSISupported nos dispositivos NVIDIA",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "msi_mode_nvidia_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!(1),
    )?;

    // Aplicar MSI em cada dispositivo que ainda não está habilitado
    for (id, _) in &targets {
        if !get_msi_mode_is_applied_for(id).unwrap_or(false) {
            write_dword(Hive::LocalMachine, &msi_reg_path(id), "MSISupported", 1)?;
        }
    }

    Ok(())
}

/// Reverte MSI Mode para o estado original em todos os dispositivos NVIDIA;
/// usa os caminhos salvos no backup para acessar os mesmos dispositivos configurados.
#[tauri::command]
pub fn disable_msi_mode_gpu() -> Result<(), String> {
    let original = restore_from_backup("enable_msi_mode_gpu")?;
    let entries = original
        .value
        .ok_or("Backup de 'enable_msi_mode_gpu' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'enable_msi_mode_gpu'")?;
    restore_multi_entries(arr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Multiplane Overlay (MPO)
//
// HKLM\SOFTWARE\Microsoft\Windows\Dwm -> OverlayTestMode = 5
//
// Ausência da chave ou valor != 5 significa MPO habilitado (padrão Windows).
// Recomendado para configurações dual-monitor com refresh rates diferentes.
// Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

const MPO_PATH: &str = r"SOFTWARE\Microsoft\Windows\Dwm";
const MPO_KEY: &str = "OverlayTestMode";
const MPO_DISABLED_VALUE: u32 = 5;

static MPO_META: TweakMeta = TweakMeta {
    id: "disable_mpo",
    name: "Desabilitar Multiplane Overlay (MPO)",
    description: "Desabilita o Multiplane Overlay do DWM, que pode causar stuttering e \
        flickering em configurações multi-monitor com refresh rates diferentes. Recomendado \
        se você usa dois monitores com Hz diferentes.",
    category: "gpu_display",
    requires_restart: true,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description: "Padrão Windows: MPO habilitado (OverlayTestMode ausente)",
    hardware_filter: None,
};

/// Verifica se o MPO está desabilitado (OverlayTestMode = 5).
fn get_mpo_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY)?.unwrap_or(0);
    Ok(val == MPO_DISABLED_VALUE)
}

/// Retorna informações do tweak MPO com estado atual do registro.
///
/// Lê `OverlayTestMode` em `HKLM\SOFTWARE\Microsoft\Windows\Dwm`.
#[tauri::command]
pub async fn get_mpo_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_mpo_is_applied()?;
        Ok(MPO_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o MPO escrevendo `OverlayTestMode = 5` em `HKLM\SOFTWARE\Microsoft\Windows\Dwm`.
#[tauri::command]
pub fn disable_mpo() -> Result<(), String> {
    if get_mpo_is_applied()? {
        return Err("Tweak 'disable_mpo' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY)?;

    backup_before_apply(
        "disable_mpo",
        TweakCategory::Registry,
        "MPO — OverlayTestMode em HKLM\\SOFTWARE\\Microsoft\\Windows\\Dwm",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", MPO_PATH),
            key: MPO_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(MPO_DISABLED_VALUE),
    )?;

    write_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY, MPO_DISABLED_VALUE)
}

/// Reverte o MPO para o estado original: remove a chave (se ausente antes) ou restaura o valor.
#[tauri::command]
pub fn revert_mpo() -> Result<(), String> {
    let original = restore_from_backup("disable_mpo")?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, MPO_PATH, MPO_KEY)? {
                delete_value(Hive::LocalMachine, MPO_PATH, MPO_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                MPO_PATH,
                MPO_KEY,
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_mpo': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar NVIDIA Telemetry
//
// Quatro chaves de registro + serviço NvTelemetryContainer:
//   1. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID44231 = 0
//   2. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID64640 = 0
//   3. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID66610 = 0
//   4. HKLM\SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client -> OptInOrOutPreference = 0
//   5. Serviço NvTelemetryContainer -> Disabled (se existir)
//
// Não afeta funcionalidade do driver.
// ═══════════════════════════════════════════════════════════════════════════════

const NVIDIA_FTS_PATH: &str = r"SOFTWARE\NVIDIA Corporation\Global\FTS";
const NVIDIA_FTS_KEY_1: &str = "EnableRID44231";
const NVIDIA_FTS_KEY_2: &str = "EnableRID64640";
const NVIDIA_FTS_KEY_3: &str = "EnableRID66610";
const NVIDIA_CP_PATH: &str = r"SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client";
const NVIDIA_CP_KEY: &str = "OptInOrOutPreference";
const NVIDIA_TELEMETRY_SERVICE: &str = "NvTelemetryContainer";

static NVIDIA_TELEMETRY_META: TweakMeta = TweakMeta {
    id: "disable_nvidia_telemetry",
    name: "Desabilitar Telemetria NVIDIA",
    description: "Desabilita a coleta de telemetria do driver NVIDIA. Remove uso de CPU e \
        rede em segundo plano sem afetar funcionalidade do driver.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão NVIDIA: telemetria habilitada e serviço NvTelemetryContainer ativo",
    hardware_filter: None,
};

/// Consulta o tipo de inicialização atual de um serviço Windows via PowerShell.
/// Retorna `None` se o serviço não existir no sistema.
fn get_service_start_type(name: &str) -> Result<Option<String>, String> {
    let script = format!(
        "(Get-Service -Name '{}' -ErrorAction SilentlyContinue).StartType",
        name
    );
    let output = run_powershell(&script)?;
    let trimmed = output.stdout.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

/// Verifica se a telemetria NVIDIA está desabilitada (4 chaves zeradas + serviço desabilitado).
fn get_nvidia_telemetry_is_applied() -> Result<bool, String> {
    let v1 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1)?.unwrap_or(1);
    let v2 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2)?.unwrap_or(1);
    let v3 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3)?.unwrap_or(1);
    let v4 = read_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY)?.unwrap_or(1);

    let regs_disabled = v1 == 0 && v2 == 0 && v3 == 0 && v4 == 0;

    let svc_ok = match get_service_start_type(NVIDIA_TELEMETRY_SERVICE)? {
        None => true,
        Some(t) => t.eq_ignore_ascii_case("Disabled"),
    };

    Ok(regs_disabled && svc_ok)
}

/// Retorna informações do tweak NVIDIA Telemetry com estado atual.
///
/// Verifica 4 chaves de registro FTS/CP e o serviço `NvTelemetryContainer`.
/// Filtro de hardware: apenas GPUs NVIDIA.
#[tauri::command]
pub async fn get_nvidia_telemetry_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_nvidia_telemetry_is_applied().unwrap_or(false);
        let mut info = NVIDIA_TELEMETRY_META.build(is_applied);
        // hardware_filter precisa ser dinâmico (contém Strings alocadas)
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("nvidia".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita a telemetria NVIDIA: zera 4 chaves de registro e desabilita o serviço.
///
/// O estado original do serviço (Automatic/Manual/Disabled/inexistente) é preservado
/// no backup para reversão precisa.
#[tauri::command]
pub fn disable_nvidia_telemetry() -> Result<(), String> {
    if get_nvidia_telemetry_is_applied()? {
        return Err("Tweak 'disable_nvidia_telemetry' já está aplicado".to_string());
    }

    let orig_1 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1)?;
    let orig_2 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2)?;
    let orig_3 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3)?;
    let orig_4 = read_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY)?;
    let orig_svc = get_service_start_type(NVIDIA_TELEMETRY_SERVICE)?;

    let v1 = orig_1.map(|v| json!(v)).unwrap_or(Value::Null);
    let v2 = orig_2.map(|v| json!(v)).unwrap_or(Value::Null);
    let v3 = orig_3.map(|v| json!(v)).unwrap_or(Value::Null);
    let v4 = orig_4.map(|v| json!(v)).unwrap_or(Value::Null);
    let svc_val: Value = orig_svc.as_deref().map(|t| json!(t)).unwrap_or(Value::Null);

    backup_before_apply(
        "disable_nvidia_telemetry",
        TweakCategory::Registry,
        "NVIDIA Telemetria — 4 chaves FTS/CP + serviço NvTelemetryContainer",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "nvidia_telemetry_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_1,
                    "value": v1
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_2,
                    "value": v2
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_3,
                    "value": v3
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_CP_PATH,
                    "key": NVIDIA_CP_KEY,
                    "value": v4
                },
                {
                    "type": "service",
                    "name": NVIDIA_TELEMETRY_SERVICE,
                    "value": svc_val
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([0, 0, 0, 0, "Disabled"]),
    )?;

    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY, 0)?;

    if orig_svc.is_some() {
        let script = format!(
            "Set-Service -Name '{}' -StartupType Disabled -ErrorAction SilentlyContinue",
            NVIDIA_TELEMETRY_SERVICE
        );
        run_powershell(&script)?;
    }

    Ok(())
}

/// Restaura todas as chaves e o serviço NvTelemetryContainer para os estados originais.
#[tauri::command]
pub fn revert_nvidia_telemetry() -> Result<(), String> {
    let original = restore_from_backup("disable_nvidia_telemetry")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_nvidia_telemetry' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_nvidia_telemetry'")?;
    restore_multi_entries(arr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — NVIDIA PowerMizer (GPU P-State máximo)
//
// No subkey da GPU NVIDIA (encontrado via find_gpu_registry_subkey):
//   PerfLevelSrc (DWORD) = 0x2222
//   PowerMizerEnable (DWORD) = 1
//   PowerMizerLevelAC (DWORD) = 1
//
// Força a GPU a operar sempre no P-State máximo de performance (P0),
// eliminando downclock em idle e transições de frequência durante gaming.
// ═══════════════════════════════════════════════════════════════════════════════

const POWERMIZER_KEY_PERFLEVEL: &str = "PerfLevelSrc";
const POWERMIZER_KEY_ENABLE: &str = "PowerMizerEnable";
const POWERMIZER_KEY_LEVEL_AC: &str = "PowerMizerLevelAC";
const POWERMIZER_PERFLEVEL_VALUE: u32 = 0x2222;
const POWERMIZER_ENABLE_VALUE: u32 = 1;
const POWERMIZER_LEVEL_AC_VALUE: u32 = 1;

static POWERMIZER_META: TweakMeta = TweakMeta {
    id: "nvidia_power_mizer",
    name: "NVIDIA PowerMizer (P-State Máximo)",
    description: "Força a GPU NVIDIA a operar sempre no P-State máximo de performance (P0), \
        eliminando downclock em idle e transições de frequência durante gaming. Equivale a \
        \"Prefer Maximum Performance\" no NVIDIA Control Panel, mas aplicado diretamente \
        no registry do driver.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão NVIDIA: PowerMizer automático (driver gerencia P-States dinamicamente)",
    hardware_filter: None, // Será definido dinamicamente no get_info
};

/// Verifica se PowerMizer está configurado para P-State máximo.
fn get_powermizer_is_applied() -> Result<bool, String> {
    let subkey = match find_gpu_registry_subkey("NVIDIA") {
        Some(p) => p,
        None => return Ok(false),
    };
    let val = read_dword(Hive::LocalMachine, &subkey, POWERMIZER_KEY_ENABLE)?.unwrap_or(0);
    Ok(val == POWERMIZER_ENABLE_VALUE)
}

/// Retorna informações do tweak NVIDIA PowerMizer com estado atual.
#[tauri::command]
pub async fn get_nvidia_power_mizer_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_powermizer_is_applied().unwrap_or(false);
        let mut info = POWERMIZER_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("nvidia".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Aplica NVIDIA PowerMizer: força P-State máximo via 3 chaves de registro no subkey da GPU.
#[tauri::command]
pub fn enable_nvidia_power_mizer() -> Result<(), String> {
    let subkey = find_gpu_registry_subkey("NVIDIA")
        .ok_or("Nenhuma GPU NVIDIA detectada — PowerMizer não pode ser configurado")?;

    if get_powermizer_is_applied()? {
        return Err("Tweak 'nvidia_power_mizer' já está aplicado".to_string());
    }

    let orig_perf = read_dword(Hive::LocalMachine, &subkey, POWERMIZER_KEY_PERFLEVEL)?;
    let orig_enable = read_dword(Hive::LocalMachine, &subkey, POWERMIZER_KEY_ENABLE)?;
    let orig_level = read_dword(Hive::LocalMachine, &subkey, POWERMIZER_KEY_LEVEL_AC)?;

    let vp = orig_perf.map(|v| json!(v)).unwrap_or(Value::Null);
    let ve = orig_enable.map(|v| json!(v)).unwrap_or(Value::Null);
    let vl = orig_level.map(|v| json!(v)).unwrap_or(Value::Null);

    backup_before_apply(
        "nvidia_power_mizer",
        TweakCategory::Registry,
        "NVIDIA PowerMizer — PerfLevelSrc + PowerMizerEnable + PowerMizerLevelAC",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "nvidia_powermizer_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKLM",
                    "path": subkey,
                    "key": POWERMIZER_KEY_PERFLEVEL,
                    "value": vp
                },
                {
                    "hive": "HKLM",
                    "path": subkey,
                    "key": POWERMIZER_KEY_ENABLE,
                    "value": ve
                },
                {
                    "hive": "HKLM",
                    "path": subkey,
                    "key": POWERMIZER_KEY_LEVEL_AC,
                    "value": vl
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([POWERMIZER_PERFLEVEL_VALUE, POWERMIZER_ENABLE_VALUE, POWERMIZER_LEVEL_AC_VALUE]),
    )?;

    write_dword(
        Hive::LocalMachine,
        &subkey,
        POWERMIZER_KEY_PERFLEVEL,
        POWERMIZER_PERFLEVEL_VALUE,
    )?;
    write_dword(
        Hive::LocalMachine,
        &subkey,
        POWERMIZER_KEY_ENABLE,
        POWERMIZER_ENABLE_VALUE,
    )?;
    write_dword(
        Hive::LocalMachine,
        &subkey,
        POWERMIZER_KEY_LEVEL_AC,
        POWERMIZER_LEVEL_AC_VALUE,
    )?;
    Ok(())
}

/// Reverte NVIDIA PowerMizer: restaura os 3 valores originais do backup.
#[tauri::command]
pub fn revert_nvidia_power_mizer() -> Result<(), String> {
    let original = restore_from_backup("nvidia_power_mizer")?;
    let entries = original
        .value
        .ok_or("Backup de 'nvidia_power_mizer' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'nvidia_power_mizer'")?;
    restore_multi_entries(arr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — NVIDIA Telemetria off (IFEO + Scheduled Tasks)
//
// Parte 1 — IFEO: bloqueia NvTelemetryContainer.exe via Image File Execution Options
//   HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\
//       NvTelemetryContainer.exe -> Debugger = "%windir%\System32\taskkill.exe"
//
// Parte 2 — Scheduled tasks: desabilita tasks NvTmMon_* e NvTmRep_* via schtasks
//
// Reduz interferência de CPU/disco em background sem afetar o driver.
// ═══════════════════════════════════════════════════════════════════════════════

const NVTELEM_IFEO_PATH: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\NvTelemetryContainer.exe";
const NVTELEM_IFEO_KEY: &str = "Debugger";
const NVTELEM_IFEO_VALUE: &str = r"%windir%\System32\taskkill.exe";

static NVIDIA_TELEMETRY_OFF_META: TweakMeta = TweakMeta {
    id: "nvidia_telemetry_off",
    name: "NVIDIA Telemetria Off (IFEO + Tasks)",
    description: "Bloqueia o processo de telemetria da NVIDIA via Image File Execution Options \
        e desabilita as scheduled tasks de relatório. Reduz interferência de CPU/disco em \
        background sem afetar funcionalidade do driver ou do painel NVIDIA.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão NVIDIA: telemetria ativa (NvTelemetryContainer.exe executando normalmente)",
    hardware_filter: None,
};

/// Lista tasks agendadas da NVIDIA que começam com os prefixos fornecidos.
fn list_nvidia_telemetry_tasks() -> Vec<String> {
    let output = std::process::Command::new("schtasks")
        .args(["/Query", "/FO", "CSV", "/NH"])
        .output()
        .ok();

    let stdout = match output {
        Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        None => return Vec::new(),
    };

    stdout
        .lines()
        .filter_map(|line| {
            // CSV: "task_name","status",...
            let name = line.split(',').next()?.trim_matches('"');
            // Tasks de telemetria NVIDIA: NvTmMon_*, NvTmRep_*
            let basename = name.rsplit('\\').next().unwrap_or(name);
            if basename.starts_with("NvTmMon") || basename.starts_with("NvTmRep") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Verifica se a telemetria NVIDIA está bloqueada via IFEO.
fn get_nvidia_telemetry_off_is_applied() -> Result<bool, String> {
    let val = read_string(Hive::LocalMachine, NVTELEM_IFEO_PATH, NVTELEM_IFEO_KEY)?;
    Ok(val.is_some())
}

/// Retorna informações do tweak NVIDIA Telemetria Off com estado atual.
#[tauri::command]
pub async fn get_nvidia_telemetry_off_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_nvidia_telemetry_off_is_applied().unwrap_or(false);
        let mut info = NVIDIA_TELEMETRY_OFF_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("nvidia".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Aplica bloqueio de telemetria NVIDIA: IFEO + desabilita scheduled tasks.
#[tauri::command]
pub fn enable_nvidia_telemetry_off() -> Result<(), String> {
    if get_nvidia_telemetry_off_is_applied()? {
        return Err("Tweak 'nvidia_telemetry_off' já está aplicado".to_string());
    }

    // Buscar tasks antes de aplicar (para salvar no backup quais foram desabilitadas)
    let tasks = list_nvidia_telemetry_tasks();

    backup_before_apply(
        "nvidia_telemetry_off",
        TweakCategory::Registry,
        "NVIDIA Telemetria Off — IFEO NvTelemetryContainer + scheduled tasks NvTmMon/NvTmRep",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", NVTELEM_IFEO_PATH),
            key: NVTELEM_IFEO_KEY.to_string(),
            value: Some(json!({ "ifeo": true, "tasks": tasks })),
            value_type: "STRING".to_string(),
        },
        json!(NVTELEM_IFEO_VALUE),
    )?;

    // Parte 1: IFEO
    write_string(
        Hive::LocalMachine,
        NVTELEM_IFEO_PATH,
        NVTELEM_IFEO_KEY,
        NVTELEM_IFEO_VALUE,
    )?;

    // Parte 2: Desabilitar scheduled tasks
    for task in &tasks {
        let _ = std::process::Command::new("schtasks")
            .args(["/Change", "/TN", task, "/DISABLE"])
            .output();
    }

    Ok(())
}

/// Reverte bloqueio de telemetria NVIDIA: remove IFEO + reabilita tasks.
#[tauri::command]
pub fn revert_nvidia_telemetry_off() -> Result<(), String> {
    let original = restore_from_backup("nvidia_telemetry_off")?;

    // Remover a chave IFEO inteira (subkey NvTelemetryContainer.exe)
    delete_subkey_all(Hive::LocalMachine, NVTELEM_IFEO_PATH)?;

    // Reabilitar tasks salvas no backup
    if let Some(backup_val) = &original.value {
        if let Some(tasks) = backup_val.get("tasks").and_then(|t| t.as_array()) {
            for task in tasks {
                if let Some(name) = task.as_str() {
                    let _ = std::process::Command::new("schtasks")
                        .args(["/Change", "/TN", name, "/ENABLE"])
                        .output();
                }
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — NVIDIA Overlay Off (ShadowPlay / GeForce Experience)
//
// HKCU\Software\NVIDIA Corporation\ShadowPlay\ShadowPlayOnSystemStart
//   Enable (DWORD) = 0 para desabilitar
//   Enable (DWORD) = 1 para habilitar (padrão)
//
// O overlay faz DLL injection em todos os jogos, adicionando overhead de CPU.
// Desabilitar remove latência sem desinstalar o GFE.
// ═══════════════════════════════════════════════════════════════════════════════

const NVIDIA_OVERLAY_PATH: &str =
    r"Software\NVIDIA Corporation\ShadowPlay\ShadowPlayOnSystemStart";
const NVIDIA_OVERLAY_KEY: &str = "Enable";
const NVIDIA_OVERLAY_DISABLED: u32 = 0;
const NVIDIA_OVERLAY_ENABLED: u32 = 1;

static NVIDIA_OVERLAY_META: TweakMeta = TweakMeta {
    id: "nvidia_overlay_off",
    name: "NVIDIA Overlay Off (ShadowPlay / GFE)",
    description: "Desabilita o overlay in-game do GeForce Experience (ShadowPlay). O overlay \
        faz DLL injection em todos os jogos, adicionando overhead de CPU mensurável. \
        Desabilitar remove essa latência sem desinstalar o GFE.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão NVIDIA: overlay habilitado (ShadowPlayOnSystemStart\\Enable = 1)",
    hardware_filter: None,
};

/// Verifica se o overlay NVIDIA está desabilitado.
fn get_nvidia_overlay_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::CurrentUser, NVIDIA_OVERLAY_PATH, NVIDIA_OVERLAY_KEY)?.unwrap_or(1);
    Ok(val == NVIDIA_OVERLAY_DISABLED)
}

/// Retorna informações do tweak NVIDIA Overlay Off com estado atual.
#[tauri::command]
pub async fn get_nvidia_overlay_off_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_nvidia_overlay_is_applied().unwrap_or(false);
        let mut info = NVIDIA_OVERLAY_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("nvidia".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o overlay NVIDIA definindo `Enable = 0`.
#[tauri::command]
pub fn disable_nvidia_overlay() -> Result<(), String> {
    if get_nvidia_overlay_is_applied()? {
        return Err("Tweak 'nvidia_overlay_off' já está aplicado".to_string());
    }

    let original = read_dword(Hive::CurrentUser, NVIDIA_OVERLAY_PATH, NVIDIA_OVERLAY_KEY)?;

    backup_before_apply(
        "nvidia_overlay_off",
        TweakCategory::Registry,
        "NVIDIA Overlay — ShadowPlayOnSystemStart\\Enable",
        OriginalValue {
            path: format!("HKEY_CURRENT_USER\\{}", NVIDIA_OVERLAY_PATH),
            key: NVIDIA_OVERLAY_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(NVIDIA_OVERLAY_DISABLED),
    )?;

    write_dword(
        Hive::CurrentUser,
        NVIDIA_OVERLAY_PATH,
        NVIDIA_OVERLAY_KEY,
        NVIDIA_OVERLAY_DISABLED,
    )
}

/// Reabilita o overlay NVIDIA definindo `Enable = 1`.
#[tauri::command]
pub fn revert_nvidia_overlay() -> Result<(), String> {
    let original = restore_from_backup("nvidia_overlay_off")?;

    match original.value {
        None => write_dword(
            Hive::CurrentUser,
            NVIDIA_OVERLAY_PATH,
            NVIDIA_OVERLAY_KEY,
            NVIDIA_OVERLAY_ENABLED,
        ),
        Some(Value::Number(n)) => write_dword(
            Hive::CurrentUser,
            NVIDIA_OVERLAY_PATH,
            NVIDIA_OVERLAY_KEY,
            n.as_u64().unwrap_or(1) as u32,
        ),
        Some(other) => Err(format!(
            "Tipo inesperado no backup de 'nvidia_overlay_off': {:?}",
            other
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — AMD ULPS Disable (Ultra Low Power State)
//
// No subkey da GPU AMD (encontrado via find_gpu_registry_subkey):
//   EnableUlps (DWORD) = 0 para desabilitar
//   EnableUlps (DWORD) = 1 para habilitar (padrão)
//
// O ULPS coloca a GPU em deep sleep agressivo, causando delays de 100-500ms
// ao acordar, black screens em multi-monitor e stutters ao retornar do
// screensaver. Afeta especialmente configurações multi-GPU.
// ═══════════════════════════════════════════════════════════════════════════════

const AMD_ULPS_KEY: &str = "EnableUlps";

static AMD_ULPS_META: TweakMeta = TweakMeta {
    id: "amd_ulps_disable",
    name: "AMD ULPS Disable (Ultra Low Power State)",
    description: "Desabilita o Ultra Low Power State (ULPS) da GPU AMD. O ULPS coloca a GPU \
        em deep sleep agressivo, causando delays de 100-500ms ao acordar, black screens em \
        setups multi-monitor e stutters ao retornar do screensaver ou modo de economia. \
        Afeta especialmente configurações multi-GPU.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão AMD: ULPS habilitado (EnableUlps = 1)",
    hardware_filter: None,
};

/// Localiza o subkey da GPU AMD, tentando "AMD" e "Radeon" como fallback.
fn find_amd_gpu_subkey() -> Option<String> {
    find_gpu_registry_subkey("AMD").or_else(|| find_gpu_registry_subkey("Radeon"))
}

/// Verifica se ULPS está desabilitado na GPU AMD.
fn get_amd_ulps_is_applied() -> Result<bool, String> {
    let subkey = match find_amd_gpu_subkey() {
        Some(p) => p,
        None => return Ok(false),
    };
    let val = read_dword(Hive::LocalMachine, &subkey, AMD_ULPS_KEY)?.unwrap_or(1);
    Ok(val == 0)
}

/// Retorna informações do tweak AMD ULPS com estado atual.
#[tauri::command]
pub async fn get_amd_ulps_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_amd_ulps_is_applied().unwrap_or(false);
        let mut info = AMD_ULPS_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("amd".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita ULPS na GPU AMD definindo `EnableUlps = 0`.
#[tauri::command]
pub fn disable_amd_ulps() -> Result<(), String> {
    let subkey = find_amd_gpu_subkey()
        .ok_or("Nenhuma GPU AMD detectada — ULPS não pode ser configurado")?;

    if get_amd_ulps_is_applied()? {
        return Err("Tweak 'amd_ulps_disable' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, &subkey, AMD_ULPS_KEY)?;

    backup_before_apply(
        "amd_ulps_disable",
        TweakCategory::Registry,
        "AMD ULPS — EnableUlps no subkey da GPU AMD",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", subkey),
            key: AMD_ULPS_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(0),
    )?;

    write_dword(Hive::LocalMachine, &subkey, AMD_ULPS_KEY, 0)
}

/// Reabilita ULPS na GPU AMD restaurando o valor original do backup.
#[tauri::command]
pub fn revert_amd_ulps() -> Result<(), String> {
    let original = restore_from_backup("amd_ulps_disable")?;

    let full = &original.path;
    let reg_path = full
        .strip_prefix("HKEY_LOCAL_MACHINE\\")
        .ok_or_else(|| format!("Caminho de backup inválido para AMD ULPS: {}", full))?;

    match original.value {
        None => {
            // Valor não existia antes — restaurar para habilitado (padrão AMD)
            write_dword(Hive::LocalMachine, reg_path, AMD_ULPS_KEY, 1)
        }
        Some(Value::Number(n)) => write_dword(
            Hive::LocalMachine,
            reg_path,
            AMD_ULPS_KEY,
            n.as_u64().unwrap_or(1) as u32,
        ),
        Some(other) => Err(format!(
            "Tipo inesperado no backup de 'amd_ulps_disable': {:?}",
            other
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — AMD Shader Cache Forçado
//
// No subkey da GPU AMD, sub-chave `UMD`:
//   ShaderCache (REG_BINARY) = 32 00 (modo On)
//   ShaderCache (REG_BINARY) = 31 00 (modo Otimizado — padrão)
//
// Força o shader cache AMD para modo "On" permanente em vez do modo
// "Otimizado" padrão. Reduz stutters de compilação de shaders em jogos
// DirectX 12 e Vulkan.
// ═══════════════════════════════════════════════════════════════════════════════

const AMD_SHADER_CACHE_KEY: &str = "ShaderCache";
const AMD_SHADER_CACHE_ON: [u8; 2] = [0x32, 0x00];
const AMD_SHADER_CACHE_DEFAULT: [u8; 2] = [0x31, 0x00];

static AMD_SHADER_CACHE_META: TweakMeta = TweakMeta {
    id: "amd_shader_cache",
    name: "AMD Shader Cache Forçado (On)",
    description: "Força o shader cache AMD para modo \"On\" permanente em vez do modo \
        \"Otimizado\" padrão. Reduz stutters de compilação de shaders em jogos DirectX 12 \
        e Vulkan, especialmente notável em títulos como Elden Ring, STALKER 2 e outros \
        com compilação de shaders em tempo real.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description:
        "Padrão AMD: Shader Cache em modo \"Otimizado\" (ShaderCache = 31 00)",
    hardware_filter: None,
};

/// Retorna o path do subkey UMD da GPU AMD.
fn find_amd_umd_subkey() -> Option<String> {
    find_amd_gpu_subkey().map(|p| format!(r"{}\UMD", p))
}

/// Verifica se o shader cache AMD está em modo "On" (primeiro byte = 0x32).
fn get_amd_shader_cache_is_applied() -> Result<bool, String> {
    let subkey = match find_amd_umd_subkey() {
        Some(p) => p,
        None => return Ok(false),
    };
    let val = read_binary(Hive::LocalMachine, &subkey, AMD_SHADER_CACHE_KEY)?;
    Ok(val.as_ref().map(|b| b.first() == Some(&0x32)).unwrap_or(false))
}

/// Retorna informações do tweak AMD Shader Cache com estado atual.
#[tauri::command]
pub async fn get_amd_shader_cache_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_amd_shader_cache_is_applied().unwrap_or(false);
        let mut info = AMD_SHADER_CACHE_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: Some("amd".to_string()),
            cpu_vendor: None,
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Força shader cache AMD para modo "On" (ShaderCache = 32 00).
#[tauri::command]
pub fn enable_amd_shader_cache() -> Result<(), String> {
    let subkey = find_amd_umd_subkey()
        .ok_or("Nenhuma GPU AMD detectada — Shader Cache não pode ser configurado")?;

    if get_amd_shader_cache_is_applied()? {
        return Err("Tweak 'amd_shader_cache' já está aplicado".to_string());
    }

    let original = read_binary(Hive::LocalMachine, &subkey, AMD_SHADER_CACHE_KEY)?;

    backup_before_apply(
        "amd_shader_cache",
        TweakCategory::Registry,
        "AMD Shader Cache — ShaderCache (REG_BINARY) no subkey UMD da GPU AMD",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", subkey),
            key: AMD_SHADER_CACHE_KEY.to_string(),
            value: original.map(|b| json!(b)),
            value_type: "BINARY".to_string(),
        },
        json!(AMD_SHADER_CACHE_ON.to_vec()),
    )?;

    write_binary(
        Hive::LocalMachine,
        &subkey,
        AMD_SHADER_CACHE_KEY,
        &AMD_SHADER_CACHE_ON,
    )
}

/// Reverte shader cache AMD para modo "Otimizado" (ShaderCache = 31 00).
#[tauri::command]
pub fn revert_amd_shader_cache() -> Result<(), String> {
    let original = restore_from_backup("amd_shader_cache")?;

    let full = &original.path;
    let reg_path = full
        .strip_prefix("HKEY_LOCAL_MACHINE\\")
        .ok_or_else(|| format!("Caminho de backup inválido para AMD Shader Cache: {}", full))?;

    match original.value {
        None => {
            // Valor não existia antes — restaurar para padrão AMD
            write_binary(
                Hive::LocalMachine,
                reg_path,
                AMD_SHADER_CACHE_KEY,
                &AMD_SHADER_CACHE_DEFAULT,
            )
        }
        Some(Value::Array(arr)) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            write_binary(Hive::LocalMachine, reg_path, AMD_SHADER_CACHE_KEY, &bytes)
        }
        Some(other) => Err(format!(
            "Tipo inesperado no backup de 'amd_shader_cache': {:?}",
            other
        )),
    }
}
