//! Tweak de Edge Debloat: desabilita comportamentos agressivos do Microsoft Edge
//! via políticas de grupo (registry), sem desinstalar o browser.

use serde_json::{json, Value};

use crate::commands::optimizations::{backup_info, EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Edge Debloat
//
// HKLM\SOFTWARE\Policies\Microsoft\Edge -> múltiplas policies
// Desabilita startup boost, background mode, sidebar, shopping, telemetria etc.
// ═══════════════════════════════════════════════════════════════════════════════

const EDGE_POLICY_PATH: &str = r"SOFTWARE\Policies\Microsoft\Edge";

/// Chaves de policy do Edge: (nome, valor_aplicado)
/// Todas em HKLM\SOFTWARE\Policies\Microsoft\Edge como DWORD
const EDGE_KEYS: [(&str, u32); 12] = [
    // Performance
    ("StartupBoostEnabled", 0),
    ("BackgroundModeEnabled", 0),
    ("NewTabPagePrerenderEnabled", 0),
    // Bloatware/UX
    ("HubsSidebarEnabled", 0),
    ("EdgeShoppingAssistantEnabled", 0),
    ("EdgeCollectionsEnabled", 0),
    ("ShowRecommendationsEnabled", 0),
    ("DefaultBrowserSettingsCampaignEnabled", 0),
    ("NewTabPageBingChatEnabled", 0),
    // Telemetria
    ("DiagnosticData", 0),
    ("PersonalizationReportingEnabled", 0),
    ("UserFeedbackAllowed", 0),
];

/// Detect: verifica se pelo menos StartupBoostEnabled e BackgroundModeEnabled estão com valor 0
fn get_edge_debloat_is_applied() -> Result<bool, String> {
    let startup = read_dword(Hive::LocalMachine, EDGE_POLICY_PATH, "StartupBoostEnabled")?;
    let background = read_dword(Hive::LocalMachine, EDGE_POLICY_PATH, "BackgroundModeEnabled")?;

    Ok(startup == Some(0) && background == Some(0))
}

#[tauri::command]
pub async fn get_edge_debloat_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_edge_debloat_is_applied().unwrap_or(false);
        let (has_backup, last_applied) = backup_info("edge_debloat");

        Ok(TweakInfo {
            id: "edge_debloat".to_string(),
            name: "Edge Debloat".to_string(),
            description: "Desabilita comportamentos agressivos do Microsoft Edge sem desinstalar \
                o browser. Remove startup boost, execução em background, sidebar com Copilot, \
                shopping assistant e telemetria. O Edge continua funcionando normalmente para \
                navegação. Nota: o Edge pode exibir o aviso \"Gerenciado pela sua organização\" \
                — isso é esperado com políticas aplicadas via registry."
                .to_string(),
            category: "privacy".to_string(),
            is_applied,
            requires_restart: false,
            last_applied,
            has_backup,
            risk_level: RiskLevel::Low,
            evidence_level: EvidenceLevel::Proven,
            default_value_description:
                "Padrão Windows: Edge com startup boost, background mode, sidebar, shopping e telemetria habilitados"
                    .to_string(),
            hardware_filter: None,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn apply_edge_debloat() -> Result<(), String> {
    if get_edge_debloat_is_applied()? {
        return Err("Tweak 'edge_debloat' já está aplicado".to_string());
    }

    // Lê todos os valores originais ANTES de qualquer modificação
    let mut backup_entries: Vec<Value> = Vec::new();
    let mut applied_vals: Vec<Value> = Vec::new();

    for (key, target) in &EDGE_KEYS {
        let orig = read_dword(Hive::LocalMachine, EDGE_POLICY_PATH, key)?
            .map(|v| json!(v))
            .unwrap_or(Value::Null);
        backup_entries.push(json!({
            "hive": "LocalMachine",
            "path": EDGE_POLICY_PATH,
            "key": key,
            "value": orig
        }));
        applied_vals.push(json!(target));
    }

    backup_before_apply(
        "edge_debloat",
        TweakCategory::Registry,
        "Edge Debloat: 12 policies (startup, background, sidebar, shopping, telemetria)",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "edge_debloat_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    // Aplica todas as policies (write_dword cria a chave pai se não existir)
    for (key, target) in &EDGE_KEYS {
        write_dword(Hive::LocalMachine, EDGE_POLICY_PATH, key, *target)?;
    }

    Ok(())
}

#[tauri::command]
pub fn revert_edge_debloat() -> Result<(), String> {
    let original = restore_from_backup("edge_debloat")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de Edge Debloat inválido".to_string()),
    };

    // Revert: para cada entrada, se o valor original era null (não existia),
    // deletamos a chave individual. Caso contrário, restauramos o valor original.
    for entry in &entries {
        let path = entry["path"]
            .as_str()
            .ok_or("Backup EdgeDebloat: campo 'path' ausente")?;
        let key = entry["key"]
            .as_str()
            .ok_or("Backup EdgeDebloat: campo 'key' ausente")?;

        match &entry["value"] {
            Value::Null => {
                // Chave não existia antes — deletar o valor (não a chave pai)
                if key_exists(Hive::LocalMachine, path, key)? {
                    delete_value(Hive::LocalMachine, path, key)?;
                }
            }
            Value::Number(n) => {
                let v = n.as_u64().unwrap_or(0) as u32;
                write_dword(Hive::LocalMachine, path, key, v)?;
            }
            other => {
                return Err(format!(
                    "Tipo inesperado no backup de EdgeDebloat para '{}': {:?}",
                    key, other
                ));
            }
        }
    }

    Ok(())
}
