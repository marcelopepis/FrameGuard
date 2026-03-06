//! Tweaks de GPU: HAGS, MSI Mode, MPO, NVIDIA Telemetry.
//!
//! Cada tweak segue o padrão `TweakMeta` + `build()` para separar metadados
//! estáticos do estado dinâmico do sistema.

use serde_json::{json, Value};

use crate::commands::optimizations::{
    restore_multi_entries, EvidenceLevel, HardwareFilter, RiskLevel, TweakInfo,
};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::command_runner::run_powershell;
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};
use crate::utils::tweak_builder::TweakMeta;

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
// TWEAK — Habilitar MSI Mode para GPU NVIDIA
//
// Detecta o InstanceId da GPU NVIDIA via Get-PnpDevice e configura:
//   HKLM\SYSTEM\CurrentControlSet\Enum\{InstanceId}\Device Parameters\
//       Interrupt Management\MessageSignaledInterruptProperties -> MSISupported = 1
//
// GPUs RTX 40+ já usam MSI por padrão. Benefício principal em RTX 30 e anteriores.
// O caminho dinâmico é salvo integralmente no backup para reversão segura.
// Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

static MSI_MODE_META: TweakMeta = TweakMeta {
    id: "enable_msi_mode_gpu",
    name: "Habilitar MSI Mode para GPU",
    description: "Habilita Message Signaled Interrupts para a GPU, reduzindo latência de \
        DPC. GPUs RTX 40+ já usam MSI por padrão. Benefício principal em GPUs RTX 30 e \
        anteriores.",
    category: "gpu_display",
    requires_restart: true,
    risk_level: RiskLevel::Medium,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: MSI Mode desabilitado para GPU (MSISupported ausente ou 0)",
    hardware_filter: None,
};

/// Busca o InstanceId da primeira GPU NVIDIA de Display encontrada via Get-PnpDevice.
/// Retorna `None` se nenhuma GPU NVIDIA estiver instalada.
fn get_nvidia_instance_id() -> Result<Option<String>, String> {
    let output = run_powershell(
        "(Get-PnpDevice | Where-Object { \
            $_.FriendlyName -like '*NVIDIA*' -and $_.Class -eq 'Display' \
        } | Select-Object -First 1).InstanceId",
    )?;
    let id = output.stdout.trim().to_string();
    if id.is_empty() {
        Ok(None)
    } else {
        Ok(Some(id))
    }
}

/// Monta o caminho de registro MSI Interrupt Properties para o InstanceId fornecido.
fn msi_reg_path(instance_id: &str) -> String {
    format!(
        r"SYSTEM\CurrentControlSet\Enum\{}\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties",
        instance_id
    )
}

/// Verifica se MSI Mode está habilitado para a GPU com o InstanceId fornecido.
fn get_msi_mode_is_applied(instance_id: &str) -> Result<bool, String> {
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
/// Detecta automaticamente a GPU NVIDIA via PowerShell e verifica `MSISupported`.
/// Se nenhuma GPU NVIDIA estiver presente, `is_applied` retorna `false`.
#[tauri::command]
pub async fn get_msi_mode_gpu_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = match get_nvidia_instance_id()? {
            Some(id) => get_msi_mode_is_applied(&id).unwrap_or(false),
            None => false,
        };

        Ok(MSI_MODE_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Habilita MSI Mode na GPU NVIDIA detectada automaticamente.
///
/// O caminho de registro dinâmico (contendo o InstanceId do dispositivo) é salvo
/// integralmente no backup para garantir reversão correta mesmo após updates de driver.
#[tauri::command]
pub fn enable_msi_mode_gpu() -> Result<(), String> {
    let instance_id = get_nvidia_instance_id()?
        .ok_or("Nenhuma GPU NVIDIA detectada — MSI Mode não pode ser configurado")?;

    if get_msi_mode_is_applied(&instance_id)? {
        return Err("Tweak 'enable_msi_mode_gpu' já está aplicado".to_string());
    }

    let reg_path = msi_reg_path(&instance_id);
    let original = read_dword(Hive::LocalMachine, &reg_path, "MSISupported")?;

    backup_before_apply(
        "enable_msi_mode_gpu",
        TweakCategory::Registry,
        "MSI Mode GPU — MSISupported no caminho do dispositivo NVIDIA",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", reg_path),
            key: "MSISupported".to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(1),
    )?;

    write_dword(Hive::LocalMachine, &reg_path, "MSISupported", 1)
}

/// Reverte MSI Mode para o estado original; usa o caminho salvo no backup para
/// acessar o mesmo dispositivo que foi configurado durante o apply.
#[tauri::command]
pub fn disable_msi_mode_gpu() -> Result<(), String> {
    let original = restore_from_backup("enable_msi_mode_gpu")?;

    let full_path = &original.path;
    let reg_path = full_path
        .strip_prefix("HKEY_LOCAL_MACHINE\\")
        .ok_or_else(|| format!("Caminho de backup inválido para MSI Mode: {}", full_path))?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, reg_path, "MSISupported")? {
                delete_value(Hive::LocalMachine, reg_path, "MSISupported")?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                reg_path,
                "MSISupported",
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'enable_msi_mode_gpu': {:?}",
                other
            ));
        }
    }

    Ok(())
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
