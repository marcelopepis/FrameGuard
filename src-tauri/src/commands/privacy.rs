//! Comandos Tauri para tweaks de privacidade e debloat do FrameGuard.
//!
//! Cada tweak segue o fluxo padrão:
//!   1. Verificar estado atual no sistema (`get_X_info`)
//!   2. Salvar backup dos valores originais via `utils::backup`
//!   3. Aplicar as modificações (`disable_X`)
//!   4. Reverter a partir do backup (`revert_X`)

use serde_json::{json, Value};

use crate::commands::optimizations::{backup_info, EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};

// ═══════════════════════════════════════════════════════════════════════════════
// Privacidade — Desabilitar Telemetria do Windows
// ═══════════════════════════════════════════════════════════════════════════════

const TELEMETRY_DATA_COLLECTION_PATH: &str =
    r"SOFTWARE\Policies\Microsoft\Windows\DataCollection";
const TELEMETRY_DATA_COLLECTION_KEY: &str = "AllowTelemetry";

const TELEMETRY_PRIVACY_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Privacy";
const TELEMETRY_TAILORED_KEY: &str = "TailoredExperiencesWithDiagnosticDataEnabled";

const TELEMETRY_ADVERTISING_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo";
const TELEMETRY_ADVERTISING_KEY: &str = "Enabled";

/// Chaves de telemetria: (Hive, path, key, valor_aplicado)
const TELEMETRY_KEYS: [(Hive, &str, &str, u32); 3] = [
    (Hive::LocalMachine, TELEMETRY_DATA_COLLECTION_PATH, TELEMETRY_DATA_COLLECTION_KEY, 0),
    (Hive::CurrentUser,  TELEMETRY_PRIVACY_PATH,         TELEMETRY_TAILORED_KEY,         0),
    (Hive::CurrentUser,  TELEMETRY_ADVERTISING_PATH,     TELEMETRY_ADVERTISING_KEY,      0),
];

fn get_telemetry_is_applied() -> Result<bool, String> {
    for (hive, path, key, target) in &TELEMETRY_KEYS {
        let val = read_dword(*hive, path, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_telemetry_registry_info() -> Result<TweakInfo, String> {
    let is_applied = get_telemetry_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_telemetry_registry");

    Ok(TweakInfo {
        id: "disable_telemetry_registry".to_string(),
        name: "Desabilitar Telemetria do Windows".to_string(),
        description: "Reduz a coleta de dados de diagnóstico e telemetria enviados à Microsoft. \
            Define AllowTelemetry = 0 (nível mínimo), desabilita experiências personalizadas e \
            remove o ID de publicidade usado para rastreamento entre apps."
            .to_string(),
        category: "privacy".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: telemetria em nível Completo, experiências personalizadas e ID de publicidade habilitados"
                .to_string(),
    
        hardware_filter: None,
    })
}

#[tauri::command]
pub fn disable_telemetry_registry() -> Result<(), String> {
    if get_telemetry_is_applied()? {
        return Err(
            "Tweak 'disable_telemetry_registry' já está aplicado".to_string(),
        );
    }

    // Lê todos os originais ANTES de qualquer modificação
    let orig_vals: Vec<Value> = TELEMETRY_KEYS
        .iter()
        .map(|(hive, path, key, _)| {
            read_dword(*hive, path, key)
                .map(|opt| opt.map(|v| json!(v)).unwrap_or(Value::Null))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let backup_entries: Vec<Value> = TELEMETRY_KEYS
        .iter()
        .zip(orig_vals.iter())
        .map(|((hive, path, key, _), orig)| {
            json!({
                "hive": format!("{:?}", hive),
                "path": path,
                "key": key,
                "value": orig
            })
        })
        .collect();

    let applied_vals: Vec<Value> = TELEMETRY_KEYS.iter().map(|(_, _, _, v)| json!(v)).collect();

    backup_before_apply(
        "disable_telemetry_registry",
        TweakCategory::Registry,
        "Telemetria: AllowTelemetry, TailoredExperiences, AdvertisingId",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "telemetry_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    for (hive, path, key, target) in &TELEMETRY_KEYS {
        write_dword(*hive, path, key, *target)?;
    }

    Ok(())
}

#[tauri::command]
pub fn revert_telemetry_registry() -> Result<(), String> {
    let original = restore_from_backup("disable_telemetry_registry")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de telemetria inválido".to_string()),
    };

    for entry in &entries {
        let hive_str = entry["hive"].as_str().unwrap_or("CurrentUser");
        let path = entry["path"].as_str()
            .ok_or("Backup telemetria: campo 'path' ausente")?;
        let key = entry["key"].as_str()
            .ok_or("Backup telemetria: campo 'key' ausente")?;
        let hive = if hive_str == "LocalMachine" { Hive::LocalMachine } else { Hive::CurrentUser };

        match &entry["value"] {
            Value::Null => {
                if key_exists(hive, path, key)? {
                    delete_value(hive, path, key)?;
                }
            }
            Value::Number(n) => {
                let v = n.as_u64().unwrap_or(0) as u32;
                write_dword(hive, path, key, v)?;
            }
            other => {
                return Err(format!(
                    "Tipo inesperado no backup de telemetria para '{}': {:?}",
                    key, other
                ));
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacidade — Desabilitar Copilot e Cortana
// ═══════════════════════════════════════════════════════════════════════════════

const COPILOT_POLICY_PATH: &str =
    r"Software\Policies\Microsoft\Windows\WindowsCopilot";
const COPILOT_POLICY_KEY: &str = "TurnOffWindowsCopilot";

const COPILOT_BUTTON_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const COPILOT_BUTTON_KEY: &str = "ShowCopilotButton";

const CORTANA_POLICY_PATH: &str =
    r"SOFTWARE\Policies\Microsoft\Windows\Windows Search";
const CORTANA_POLICY_KEY: &str = "AllowCortana";

/// Chaves Copilot/Cortana: (Hive, path, key, valor_aplicado)
const COPILOT_KEYS: [(Hive, &str, &str, u32); 3] = [
    (Hive::CurrentUser,  COPILOT_POLICY_PATH, COPILOT_POLICY_KEY, 1),
    (Hive::CurrentUser,  COPILOT_BUTTON_PATH, COPILOT_BUTTON_KEY, 0),
    (Hive::LocalMachine, CORTANA_POLICY_PATH, CORTANA_POLICY_KEY, 0),
];

fn get_copilot_is_applied() -> Result<bool, String> {
    for (hive, path, key, target) in &COPILOT_KEYS {
        let val = read_dword(*hive, path, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_copilot_info() -> Result<TweakInfo, String> {
    let is_applied = get_copilot_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_copilot");

    Ok(TweakInfo {
        id: "disable_copilot".to_string(),
        name: "Desabilitar Copilot e Cortana".to_string(),
        description: "Desabilita o Windows Copilot (assistente IA), oculta o botão da barra \
            de tarefas e bloqueia o Cortana via política de grupo. Impede envio de consultas \
            e contexto do sistema para servidores da Microsoft."
            .to_string(),
        category: "privacy".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: Copilot habilitado, botão visível, Cortana permitida".to_string(),
    
        hardware_filter: None,
    })
}

#[tauri::command]
pub fn disable_copilot() -> Result<(), String> {
    if get_copilot_is_applied()? {
        return Err("Tweak 'disable_copilot' já está aplicado".to_string());
    }

    let orig_vals: Vec<Value> = COPILOT_KEYS
        .iter()
        .map(|(hive, path, key, _)| {
            read_dword(*hive, path, key)
                .map(|opt| opt.map(|v| json!(v)).unwrap_or(Value::Null))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let backup_entries: Vec<Value> = COPILOT_KEYS
        .iter()
        .zip(orig_vals.iter())
        .map(|((hive, path, key, _), orig)| {
            json!({
                "hive": format!("{:?}", hive),
                "path": path,
                "key": key,
                "value": orig
            })
        })
        .collect();

    let applied_vals: Vec<Value> = COPILOT_KEYS.iter().map(|(_, _, _, v)| json!(v)).collect();

    backup_before_apply(
        "disable_copilot",
        TweakCategory::Registry,
        "Copilot/Cortana: TurnOffWindowsCopilot, ShowCopilotButton, AllowCortana",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "copilot_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    for (hive, path, key, target) in &COPILOT_KEYS {
        write_dword(*hive, path, key, *target)?;
    }

    Ok(())
}

#[tauri::command]
pub fn revert_copilot() -> Result<(), String> {
    let original = restore_from_backup("disable_copilot")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de Copilot inválido".to_string()),
    };

    revert_multi_dword_entries(&entries, "Copilot")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacidade — Desabilitar Content Delivery Manager
// ═══════════════════════════════════════════════════════════════════════════════

const CDM_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";

/// Chaves do ContentDeliveryManager que devem ser 0 quando aplicado
const CDM_KEYS: [&str; 14] = [
    "ContentDeliveryAllowed",
    "OemPreInstalledAppsEnabled",
    "PreInstalledAppsEnabled",
    "PreInstalledAppsEverEnabled",
    "SilentInstalledAppsEnabled",
    "SoftLandingEnabled",
    "SubscribedContentEnabled",
    "SubscribedContent-310093Enabled",
    "SubscribedContent-338388Enabled",
    "SubscribedContent-338389Enabled",
    "SubscribedContent-338393Enabled",
    "SubscribedContent-353694Enabled",
    "SubscribedContent-353696Enabled",
    "SystemPaneSuggestionsEnabled",
];

const CDM_CLOUD_PATH: &str = r"SOFTWARE\Policies\Microsoft\Windows\CloudContent";
const CDM_PUSH_PATH: &str = r"SOFTWARE\Policies\Microsoft\PushToInstall";

/// Chaves adicionais (HKLM): (path, key, valor_aplicado)
const CDM_EXTRA_KEYS: [(&str, &str, u32); 3] = [
    (CDM_CLOUD_PATH, "DisableWindowsConsumerFeatures",    1),
    (CDM_CLOUD_PATH, "DisableConsumerAccountStateContent", 1),
    (CDM_PUSH_PATH,  "DisablePushToInstall",               1),
];

fn get_content_delivery_is_applied() -> Result<bool, String> {
    // Verifica as 14 chaves HKCU do ContentDeliveryManager (todas devem ser 0)
    for key in &CDM_KEYS {
        let val = read_dword(Hive::CurrentUser, CDM_PATH, key)?.unwrap_or(99);
        if val != 0 {
            return Ok(false);
        }
    }
    // Verifica as 3 chaves HKLM adicionais
    for (path, key, target) in &CDM_EXTRA_KEYS {
        let val = read_dword(Hive::LocalMachine, path, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_content_delivery_info() -> Result<TweakInfo, String> {
    let is_applied = get_content_delivery_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_content_delivery");

    Ok(TweakInfo {
        id: "disable_content_delivery".to_string(),
        name: "Desabilitar Content Delivery Manager".to_string(),
        description: "Impede o Windows de instalar silenciosamente apps sugeridos (bloatware), \
            exibir dicas, sugestões e propagandas no Menu Iniciar e tela de bloqueio. \
            Também bloqueia recursos de consumidor via política de grupo."
            .to_string(),
        category: "privacy".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: instalação automática de apps sugeridos e sugestões habilitadas"
                .to_string(),
    
        hardware_filter: None,
    })
}

#[tauri::command]
pub fn disable_content_delivery() -> Result<(), String> {
    if get_content_delivery_is_applied()? {
        return Err(
            "Tweak 'disable_content_delivery' já está aplicado".to_string(),
        );
    }

    // Monta array com todas as entradas (14 HKCU + 3 HKLM)
    let mut backup_entries: Vec<Value> = Vec::new();
    let mut applied_vals: Vec<Value> = Vec::new();

    // 14 chaves HKCU do ContentDeliveryManager
    for key in &CDM_KEYS {
        let orig = read_dword(Hive::CurrentUser, CDM_PATH, key)?
            .map(|v| json!(v))
            .unwrap_or(Value::Null);
        backup_entries.push(json!({
            "hive": "CurrentUser",
            "path": CDM_PATH,
            "key": key,
            "value": orig
        }));
        applied_vals.push(json!(0));
    }

    // 3 chaves HKLM adicionais
    for (path, key, target) in &CDM_EXTRA_KEYS {
        let orig = read_dword(Hive::LocalMachine, path, key)?
            .map(|v| json!(v))
            .unwrap_or(Value::Null);
        backup_entries.push(json!({
            "hive": "LocalMachine",
            "path": path,
            "key": key,
            "value": orig
        }));
        applied_vals.push(json!(target));
    }

    backup_before_apply(
        "disable_content_delivery",
        TweakCategory::Registry,
        "ContentDeliveryManager (14 chaves HKCU) + CloudContent/PushToInstall (3 chaves HKLM)",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "content_delivery_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    // Aplica as 14 chaves HKCU
    for key in &CDM_KEYS {
        write_dword(Hive::CurrentUser, CDM_PATH, key, 0)?;
    }
    // Aplica as 3 chaves HKLM
    for (path, key, target) in &CDM_EXTRA_KEYS {
        write_dword(Hive::LocalMachine, path, key, *target)?;
    }

    Ok(())
}

#[tauri::command]
pub fn revert_content_delivery() -> Result<(), String> {
    let original = restore_from_backup("disable_content_delivery")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de ContentDelivery inválido".to_string()),
    };

    revert_multi_dword_entries(&entries, "ContentDelivery")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacidade — Desabilitar Background Apps (global)
// ═══════════════════════════════════════════════════════════════════════════════

const BG_APPS_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications";
const BG_APPS_KEY: &str = "GlobalUserDisabled";

const BG_APPS_SEARCH_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Search";
const BG_APPS_SEARCH_KEY: &str = "BackgroundAppGlobalToggle";

/// Chaves de background apps: (Hive, path, key, valor_aplicado)
const BG_APPS_KEYS: [(Hive, &str, &str, u32); 2] = [
    (Hive::CurrentUser, BG_APPS_PATH,        BG_APPS_KEY,        1),
    (Hive::CurrentUser, BG_APPS_SEARCH_PATH,  BG_APPS_SEARCH_KEY, 0),
];

fn get_background_apps_is_applied() -> Result<bool, String> {
    for (hive, path, key, target) in &BG_APPS_KEYS {
        let val = read_dword(*hive, path, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_background_apps_info() -> Result<TweakInfo, String> {
    let is_applied = get_background_apps_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_background_apps");

    Ok(TweakInfo {
        id: "disable_background_apps".to_string(),
        name: "Desabilitar Apps em Segundo Plano".to_string(),
        description: "Desabilita globalmente a execução de apps UWP em segundo plano. \
            Reduz consumo de CPU, RAM e rede por apps que verificam notificações e \
            atualizam conteúdo mesmo quando não estão em uso."
            .to_string(),
        category: "privacy".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description:
            "Padrão Windows: apps UWP podem executar em segundo plano".to_string(),
    
        hardware_filter: None,
    })
}

#[tauri::command]
pub fn disable_background_apps() -> Result<(), String> {
    if get_background_apps_is_applied()? {
        return Err(
            "Tweak 'disable_background_apps' já está aplicado".to_string(),
        );
    }

    let orig_vals: Vec<Value> = BG_APPS_KEYS
        .iter()
        .map(|(hive, path, key, _)| {
            read_dword(*hive, path, key)
                .map(|opt| opt.map(|v| json!(v)).unwrap_or(Value::Null))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let backup_entries: Vec<Value> = BG_APPS_KEYS
        .iter()
        .zip(orig_vals.iter())
        .map(|((hive, path, key, _), orig)| {
            json!({
                "hive": format!("{:?}", hive),
                "path": path,
                "key": key,
                "value": orig
            })
        })
        .collect();

    let applied_vals: Vec<Value> = BG_APPS_KEYS.iter().map(|(_, _, _, v)| json!(v)).collect();

    backup_before_apply(
        "disable_background_apps",
        TweakCategory::Registry,
        "Background apps: GlobalUserDisabled + BackgroundAppGlobalToggle",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "background_apps_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    for (hive, path, key, target) in &BG_APPS_KEYS {
        write_dword(*hive, path, key, *target)?;
    }

    Ok(())
}

#[tauri::command]
pub fn revert_background_apps() -> Result<(), String> {
    let original = restore_from_backup("disable_background_apps")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de background apps inválido".to_string()),
    };

    revert_multi_dword_entries(&entries, "BackgroundApps")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utilitário interno — Revert MULTI_DWORD genérico
// ═══════════════════════════════════════════════════════════════════════════════

/// Reverte um array de entradas MULTI_DWORD do backup para o registro.
///
/// Cada entrada deve conter `hive`, `path`, `key` e `value`.
/// Se `value` é `null`, a chave é removida (não existia antes).
fn revert_multi_dword_entries(entries: &[Value], label: &str) -> Result<(), String> {
    for entry in entries {
        let hive_str = entry["hive"].as_str().unwrap_or("CurrentUser");
        let path = entry["path"]
            .as_str()
            .ok_or(format!("Backup {}: campo 'path' ausente", label))?;
        let key = entry["key"]
            .as_str()
            .ok_or(format!("Backup {}: campo 'key' ausente", label))?;
        let hive = if hive_str == "LocalMachine" {
            Hive::LocalMachine
        } else {
            Hive::CurrentUser
        };

        match &entry["value"] {
            Value::Null => {
                if key_exists(hive, path, key)? {
                    delete_value(hive, path, key)?;
                }
            }
            Value::Number(n) => {
                let v = n.as_u64().unwrap_or(0) as u32;
                write_dword(hive, path, key, v)?;
            }
            other => {
                return Err(format!(
                    "Tipo inesperado no backup de {} para '{}': {:?}",
                    label, key, other
                ));
            }
        }
    }

    Ok(())
}
