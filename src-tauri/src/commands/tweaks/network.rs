//! Tweaks de rede: Delivery Optimization, Nagle.

use serde_json::{json, Value};

use crate::commands::optimizations::{EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::command_runner::run_powershell;
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};
use crate::utils::tweak_builder::TweakMeta;

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Otimização de Entrega (Delivery Optimization)
//
// HKLM\...\DeliveryOptimization\Config -> DODownloadMode
//   0 = HTTP only (sem P2P)
//   1 = HTTP + P2P rede local (padrão)
// ═══════════════════════════════════════════════════════════════════════════════

const DELIVERY_OPT_REG_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config";
const DELIVERY_OPT_REG_KEY: &str = "DODownloadMode";
const DELIVERY_OPT_DISABLED: u32 = 0;
const DELIVERY_OPT_DEFAULT: u32 = 1;

static DELIVERY_OPT_META: TweakMeta = TweakMeta {
    id: "disable_delivery_optimization",
    name: "Desabilitar Otimização de Entrega",
    description: "Desabilita o compartilhamento P2P de atualizações do Windows. Por padrão, \
        o Windows usa sua conexão de internet para enviar partes de updates para outros PCs. \
        Desabilitar libera banda de rede e pode melhorar latência em jogos online.",
    category: "optimization",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description: "Padrão Windows: P2P habilitado (DODownloadMode = 1)",
    hardware_filter: None,
};

/// Verifica se a Otimização de Entrega está desabilitada (DODownloadMode = 0).
fn check_delivery_optimization_disabled() -> Result<bool, String> {
    let current_value = read_dword(
        Hive::LocalMachine,
        DELIVERY_OPT_REG_PATH,
        DELIVERY_OPT_REG_KEY,
    )?
    .unwrap_or(DELIVERY_OPT_DEFAULT);
    Ok(current_value == DELIVERY_OPT_DISABLED)
}

/// Retorna informações do tweak Delivery Optimization com estado atual.
#[tauri::command]
pub async fn get_delivery_optimization_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = check_delivery_optimization_disabled()?;
        Ok(DELIVERY_OPT_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita a Otimização de Entrega definindo `DODownloadMode = 0`.
#[tauri::command]
pub fn disable_delivery_optimization() -> Result<(), String> {
    if check_delivery_optimization_disabled()? {
        return Err(
            "Tweak 'disable_delivery_optimization' já está aplicado (DODownloadMode = 0)"
                .to_string(),
        );
    }

    let original_dword = read_dword(
        Hive::LocalMachine,
        DELIVERY_OPT_REG_PATH,
        DELIVERY_OPT_REG_KEY,
    )?;
    let original_json: Option<Value> = original_dword.map(|v| json!(v));

    backup_before_apply(
        "disable_delivery_optimization",
        TweakCategory::Registry,
        "DODownloadMode em HKLM\\...\\DeliveryOptimization\\Config — modo P2P de updates",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", DELIVERY_OPT_REG_PATH),
            key: DELIVERY_OPT_REG_KEY.to_string(),
            value: original_json,
            value_type: "DWORD".to_string(),
        },
        json!(DELIVERY_OPT_DISABLED),
    )?;

    write_dword(
        Hive::LocalMachine,
        DELIVERY_OPT_REG_PATH,
        DELIVERY_OPT_REG_KEY,
        DELIVERY_OPT_DISABLED,
    )
}

/// Reverte a Otimização de Entrega para o estado original salvo no backup.
#[tauri::command]
pub fn revert_delivery_optimization() -> Result<(), String> {
    let original = restore_from_backup("disable_delivery_optimization")?;

    match original.value {
        None => {
            if key_exists(
                Hive::LocalMachine,
                DELIVERY_OPT_REG_PATH,
                DELIVERY_OPT_REG_KEY,
            )? {
                delete_value(
                    Hive::LocalMachine,
                    DELIVERY_OPT_REG_PATH,
                    DELIVERY_OPT_REG_KEY,
                )?;
            }
        }
        Some(Value::Number(n)) => {
            let v = n.as_u64().unwrap_or(DELIVERY_OPT_DEFAULT as u64) as u32;
            write_dword(
                Hive::LocalMachine,
                DELIVERY_OPT_REG_PATH,
                DELIVERY_OPT_REG_KEY,
                v,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_delivery_optimization': {:?}",
                other
            ));
        }
    }

    Ok(())
}

/// Remove a chave `DODownloadMode`, restaurando o modo P2P padrão do Windows Update.
#[tauri::command]
pub fn restore_delivery_optimization_default() -> Result<(), String> {
    if key_exists(
        Hive::LocalMachine,
        DELIVERY_OPT_REG_PATH,
        DELIVERY_OPT_REG_KEY,
    )? {
        delete_value(
            Hive::LocalMachine,
            DELIVERY_OPT_REG_PATH,
            DELIVERY_OPT_REG_KEY,
        )?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Algoritmo de Nagle
// ═══════════════════════════════════════════════════════════════════════════════

const NAGLE_INTERFACES_BASE: &str =
    r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
const NAGLE_ACK_FREQ_KEY: &str = "TcpAckFrequency";
const NAGLE_NO_DELAY_KEY: &str = "TCPNoDelay";

static NAGLE_META: TweakMeta = TweakMeta {
    id: "disable_nagle",
    name: "Desabilitar Algoritmo de Nagle",
    description: "Desabilita o algoritmo de Nagle e força ACK imediato em conexões TCP. \
        Pode reduzir latência em 10-20ms para jogos que usam TCP (alguns MMOs, League of \
        Legends). A maioria dos jogos modernos usa UDP, onde este tweak não tem efeito.",
    category: "network",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description: "Padrão Windows: algoritmo de Nagle habilitado",
    hardware_filter: None,
};

struct NagleStatus {
    is_applied: bool,
    guid: Option<String>,
}

/// Obtém o GUID da NIC ativa principal via PowerShell.
fn get_active_nic_guid() -> Result<String, String> {
    let result = run_powershell(
        "(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | Select-Object -First 1).InterfaceGuid",
    )?;

    let guid = result.stdout.trim().to_string();
    if guid.is_empty() {
        return Err("Nenhum adaptador de rede ativo encontrado".to_string());
    }

    Ok(guid)
}

/// Constrói o caminho de registro para as interfaces TCP da NIC informada.
fn nagle_reg_path(guid: &str) -> String {
    format!(r"{}\{}", NAGLE_INTERFACES_BASE, guid)
}

/// Verifica o estado atual do tweak de Nagle na NIC ativa.
fn get_nagle_status() -> NagleStatus {
    let guid = match get_active_nic_guid() {
        Ok(g) => g,
        Err(_) => {
            return NagleStatus {
                is_applied: false,
                guid: None,
            }
        }
    };

    let path = nagle_reg_path(&guid);
    let ack_freq = read_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY)
        .unwrap_or(None)
        .unwrap_or(0);
    let no_delay = read_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY)
        .unwrap_or(None)
        .unwrap_or(0);

    NagleStatus {
        is_applied: ack_freq == 1 && no_delay == 1,
        guid: Some(guid),
    }
}

/// Retorna informações do tweak Nagle com estado atual.
#[tauri::command]
pub async fn get_nagle_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let status = get_nagle_status();
        Ok(NAGLE_META.build(status.is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o algoritmo de Nagle na NIC ativa escrevendo `TcpAckFrequency = 1`
/// e `TCPNoDelay = 1` no caminho de registro da interface detectada dinamicamente.
#[tauri::command]
pub fn disable_nagle() -> Result<(), String> {
    let status = get_nagle_status();

    if status.is_applied {
        return Err("Tweak 'disable_nagle' já está aplicado".to_string());
    }

    let guid = status
        .guid
        .ok_or("Nenhum adaptador de rede ativo encontrado para aplicar o tweak")?;
    let path = nagle_reg_path(&guid);
    let hklm_path = format!(r"HKEY_LOCAL_MACHINE\{}", path);

    let orig_ack = read_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY)?;
    let orig_delay = read_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY)?;

    let orig_ack_json = orig_ack.map(|v| json!(v)).unwrap_or(Value::Null);
    let orig_delay_json = orig_delay.map(|v| json!(v)).unwrap_or(Value::Null);

    let backup_entries = json!([
        {
            "hive": "HKLM",
            "path": hklm_path,
            "key": NAGLE_ACK_FREQ_KEY,
            "value": orig_ack_json
        },
        {
            "hive": "HKLM",
            "path": hklm_path,
            "key": NAGLE_NO_DELAY_KEY,
            "value": orig_delay_json
        }
    ]);

    backup_before_apply(
        "disable_nagle",
        TweakCategory::Registry,
        "TcpAckFrequency e TCPNoDelay na NIC ativa — desabilita algoritmo de Nagle",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "nagle_keys".to_string(),
            value: Some(backup_entries),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([1, 1]),
    )?;

    write_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY, 1)?;
    write_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY, 1)?;

    Ok(())
}

/// Reverte o algoritmo de Nagle restaurando os valores originais das duas chaves de registro.
#[tauri::command]
pub fn revert_nagle() -> Result<(), String> {
    let original = restore_from_backup("disable_nagle")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de Nagle inválido — esperado array MULTI".to_string()),
    };

    for entry in &entries {
        let path_full = entry["path"]
            .as_str()
            .ok_or("Backup Nagle: campo 'path' ausente ou inválido")?;
        let key = entry["key"]
            .as_str()
            .ok_or("Backup Nagle: campo 'key' ausente ou inválido")?;

        let reg_path = path_full
            .strip_prefix(r"HKEY_LOCAL_MACHINE\")
            .unwrap_or(path_full);

        match &entry["value"] {
            Value::Null => {
                if key_exists(Hive::LocalMachine, reg_path, key)? {
                    delete_value(Hive::LocalMachine, reg_path, key)?;
                }
            }
            Value::Number(n) => {
                let v = n.as_u64().unwrap_or(0) as u32;
                write_dword(Hive::LocalMachine, reg_path, key, v)?;
            }
            other => {
                return Err(format!(
                    "Tipo inesperado no backup de Nagle para chave '{}': {:?}",
                    key, other
                ));
            }
        }
    }

    Ok(())
}
