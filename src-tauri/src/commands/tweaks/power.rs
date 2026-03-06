//! Tweaks de energia: Ultimate Performance, Power Throttling, Hibernation.

use serde_json::{json, Value};

use crate::commands::optimizations::{EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, get_all_backups, restore_from_backup, BackupStatus, OriginalValue,
    TweakCategory,
};
use crate::utils::command_runner::{run_command, run_powershell};
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};
use crate::utils::tweak_builder::TweakMeta;

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Ultimate Performance Power Plan
//
// Duplica o plano template (GUID e9a42b02-...) com powercfg -duplicatescheme
// e o ativa. O GUID do plano ativo original é salvo no backup para reversão.
//
// Se o sistema usa Modern Standby (bloqueio à duplicação), escreve
// PlatformAoAcOverride = 0 em HKLM\SYSTEM\...\Power antes de tentar de novo.
// ═══════════════════════════════════════════════════════════════════════════════

const ULTIMATE_PERF_GUID: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
const MODERN_STANDBY_PATH: &str = r"SYSTEM\CurrentControlSet\Control\Power";
const MODERN_STANDBY_KEY: &str = "PlatformAoAcOverride";

static ULTIMATE_PERF_META: TweakMeta = TweakMeta {
    id: "enable_ultimate_performance",
    name: "Plano de Energia: Ultimate Performance",
    description: "Ativa o plano de energia Ultimate Performance, que mantém o processador \
        em frequência máxima constantemente. Elimina latência de boost de CPU. Escondido \
        por padrão no Windows 11.",
    category: "energy_cpu",
    requires_restart: false,
    risk_level: RiskLevel::Medium,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: plano Balanceado ou Alto Desempenho ativo",
    hardware_filter: None,
};

/// Extrai um GUID UUID (8-4-4-4-12) da linha de saída do powercfg.
/// Funciona tanto em Windows PT-BR quanto EN, ignorando texto ao redor.
fn extract_guid_from_powercfg(s: &str) -> Result<String, String> {
    for word in s.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
        if clean.len() == 36 {
            let b = clean.as_bytes();
            if b[8] == b'-'
                && b[13] == b'-'
                && b[18] == b'-'
                && b[23] == b'-'
                && clean
                    .replace('-', "")
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
            {
                return Ok(clean.to_string());
            }
        }
    }
    Err(format!(
        "GUID não encontrado na saída do powercfg: {}",
        s.trim()
    ))
}

/// Retorna o GUID do plano de energia atualmente ativo.
fn get_active_power_scheme_guid() -> Result<String, String> {
    let output = run_powershell("powercfg /getactivescheme")?;
    extract_guid_from_powercfg(&output.stdout)
}

/// Verifica se o plano Ultimate Performance está ativo.
fn get_ultimate_performance_is_applied() -> Result<bool, String> {
    let output = run_powershell("powercfg /getactivescheme")?;
    let lower = output.stdout.to_lowercase();

    // 1. Template GUID ativo diretamente
    if lower.contains(ULTIMATE_PERF_GUID) {
        return Ok(true);
    }

    // 2. Plano duplicado pelo FrameGuard — compara GUID ativo com o armazenado no backup
    if let Ok(active_guid) = extract_guid_from_powercfg(&output.stdout) {
        if let Ok(backups) = get_all_backups() {
            if let Some(entry) = backups.get("enable_ultimate_performance") {
                if entry.status == BackupStatus::Applied {
                    if let Some(stored_guid) = entry.applied_value.as_str() {
                        if stored_guid.eq_ignore_ascii_case(&active_guid) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Retorna informações do tweak Ultimate Performance com estado atual.
#[tauri::command]
pub async fn get_ultimate_performance_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_ultimate_performance_is_applied().unwrap_or(false);
        Ok(ULTIMATE_PERF_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ativa o plano Ultimate Performance duplicando-o a partir do GUID template.
///
/// Fluxo:
/// 1. Captura GUID ativo original (para backup de reversão)
/// 2. Executa `powercfg -duplicatescheme` — obtém novo GUID
/// 3. Se bloqueado por Modern Standby, escreve PlatformAoAcOverride=0 e tenta de novo
/// 4. Salva backup com GUID original antes de ativar
/// 5. Executa `powercfg -setactive [novo GUID]`
#[tauri::command]
pub fn enable_ultimate_performance() -> Result<(), String> {
    if get_ultimate_performance_is_applied()? {
        return Err("Tweak 'enable_ultimate_performance' já está aplicado".to_string());
    }

    let original_guid = get_active_power_scheme_guid()?;

    let dup_cmd = format!("powercfg -duplicatescheme {}", ULTIMATE_PERF_GUID);
    let dup_output = match run_powershell(&dup_cmd) {
        Ok(out) if out.success && !out.stdout.trim().is_empty() => out,
        _ => {
            write_dword(
                Hive::LocalMachine,
                MODERN_STANDBY_PATH,
                MODERN_STANDBY_KEY,
                0,
            )?;
            let retry = run_powershell(&dup_cmd)?;
            if !retry.success || retry.stdout.trim().is_empty() {
                return Err(format!(
                    "Falha ao duplicar plano Ultimate Performance (Modern Standby): {}",
                    retry.stderr
                ));
            }
            retry
        }
    };

    let new_guid = extract_guid_from_powercfg(&dup_output.stdout)?;

    backup_before_apply(
        "enable_ultimate_performance",
        TweakCategory::Powershell,
        "Ultimate Performance — GUID do plano de energia ativo antes da troca",
        OriginalValue {
            path: "powercfg".to_string(),
            key: "active_scheme_guid".to_string(),
            value: Some(json!(original_guid)),
            value_type: "STRING".to_string(),
        },
        json!(new_guid),
    )?;

    let activate = run_powershell(&format!("powercfg -setactive {}", new_guid))?;
    if !activate.success {
        return Err(format!(
            "Falha ao ativar plano Ultimate Performance ({}): {}",
            new_guid, activate.stderr
        ));
    }

    Ok(())
}

/// Restaura o plano de energia que estava ativo antes da aplicação do tweak.
#[tauri::command]
pub fn revert_ultimate_performance() -> Result<(), String> {
    let original = restore_from_backup("enable_ultimate_performance")?;

    let original_guid = original
        .value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("GUID original não encontrado no backup de 'enable_ultimate_performance'")?;

    let output = run_powershell(&format!("powercfg -setactive {}", original_guid))?;
    if !output.success {
        return Err(format!(
            "Falha ao restaurar plano de energia ({}): {}",
            original_guid, output.stderr
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Power Throttling
//
// HKLM\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling
//   -> PowerThrottlingOff = 1 (DWORD)
// ═══════════════════════════════════════════════════════════════════════════════

const POWER_THROTTLE_PATH: &str = r"SYSTEM\CurrentControlSet\Control\Power\PowerThrottling";
const POWER_THROTTLE_KEY: &str = "PowerThrottlingOff";

static POWER_THROTTLE_META: TweakMeta = TweakMeta {
    id: "disable_power_throttling",
    name: "Desabilitar Power Throttling",
    description: "Impede que o Windows reduza a frequência de CPU para processos em \
        segundo plano. Útil para garantir que nenhum processo relacionado ao jogo seja \
        limitado.",
    category: "energy_cpu",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description:
        "Padrão Windows: Power Throttling habilitado (PowerThrottlingOff ausente)",
    hardware_filter: None,
};

/// Verifica se o Power Throttling está desabilitado (PowerThrottlingOff = 1).
fn get_power_throttling_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?.unwrap_or(0);
    Ok(val == 1)
}

/// Retorna informações do tweak Power Throttling com estado atual.
#[tauri::command]
pub async fn get_power_throttling_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_power_throttling_is_applied()?;
        Ok(POWER_THROTTLE_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o Power Throttling escrevendo `PowerThrottlingOff = 1`.
/// Cria o caminho de registro automaticamente se não existir.
#[tauri::command]
pub fn disable_power_throttling() -> Result<(), String> {
    if get_power_throttling_is_applied()? {
        return Err("Tweak 'disable_power_throttling' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?;

    backup_before_apply(
        "disable_power_throttling",
        TweakCategory::Registry,
        "Power Throttling — PowerThrottlingOff em HKLM\\...\\Power\\PowerThrottling",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", POWER_THROTTLE_PATH),
            key: POWER_THROTTLE_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(1),
    )?;

    write_dword(
        Hive::LocalMachine,
        POWER_THROTTLE_PATH,
        POWER_THROTTLE_KEY,
        1,
    )
}

/// Reverte o Power Throttling: remove a chave (se ausente antes) ou restaura o valor original.
#[tauri::command]
pub fn revert_power_throttling() -> Result<(), String> {
    let original = restore_from_backup("disable_power_throttling")?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)? {
                delete_value(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                POWER_THROTTLE_PATH,
                POWER_THROTTLE_KEY,
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_power_throttling': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Hibernação
// ═══════════════════════════════════════════════════════════════════════════════

static HIBERNATION_META: TweakMeta = TweakMeta {
    id: "disable_hibernation",
    name: "Desabilitar Hibernação",
    description: "Desabilita a hibernação e remove o arquivo hiberfil.sys, liberando \
        8-16 GB de espaço no disco do sistema. Também desabilita o Fast Startup, que \
        pode causar problemas de driver e estado do sistema.",
    category: "storage",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: hibernação habilitada (hiberfil.sys presente)",
    hardware_filter: None,
};

/// Verifica se a hibernação está habilitada checando a existência de `hiberfil.sys`.
fn get_hibernation_enabled() -> bool {
    std::path::Path::new(r"C:\hiberfil.sys").exists()
}

/// Retorna informações do tweak Hibernação com estado atual.
#[tauri::command]
pub async fn get_hibernation_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = !get_hibernation_enabled();
        Ok(HIBERNATION_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita a hibernação via `powercfg /h off`.
///
/// Remove `hiberfil.sys` e desabilita o Fast Startup automaticamente.
#[tauri::command]
pub fn disable_hibernation() -> Result<(), String> {
    if !get_hibernation_enabled() {
        return Err(
            "Tweak 'disable_hibernation' já está aplicado (hibernação já desabilitada)".to_string(),
        );
    }

    backup_before_apply(
        "disable_hibernation",
        TweakCategory::Powershell,
        "Estado da hibernação do Windows — hiberfil.sys e Fast Startup",
        OriginalValue {
            path: "powercfg".to_string(),
            key: "hibernate_state".to_string(),
            value: Some(json!("on")),
            value_type: "STATE".to_string(),
        },
        json!("off"),
    )?;

    let result = run_command("powercfg.exe", &["/h", "off"])?;
    if !result.success {
        return Err(format!(
            "powercfg /h off falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

/// Reverte a hibernação para o estado original (`powercfg /h on`).
#[tauri::command]
pub fn enable_hibernation() -> Result<(), String> {
    restore_from_backup("disable_hibernation")?;

    let result = run_command("powercfg.exe", &["/h", "on"])?;
    if !result.success {
        return Err(format!(
            "powercfg /h on falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_guid_from_english_output() {
        let output =
            "Power Scheme GUID: e9a42b02-d5df-448d-aa00-03f14749eb61  (Ultimate Performance)";
        let guid = extract_guid_from_powercfg(output).unwrap();
        assert_eq!(guid, "e9a42b02-d5df-448d-aa00-03f14749eb61");
    }

    #[test]
    fn extracts_guid_from_ptbr_output() {
        let output =
            "GUID do Esquema de Energia: 381b4222-f694-41f0-9685-ff5bb260df2e  (Equilibrado)";
        let guid = extract_guid_from_powercfg(output).unwrap();
        assert_eq!(guid, "381b4222-f694-41f0-9685-ff5bb260df2e");
    }

    #[test]
    fn extracts_guid_from_duplicate_output() {
        let output =
            "Power Scheme GUID: a1b2c3d4-e5f6-7890-abcd-ef1234567890  (Ultimate Performance)";
        let guid = extract_guid_from_powercfg(output).unwrap();
        assert_eq!(guid, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn fails_on_no_guid() {
        assert!(extract_guid_from_powercfg("No power scheme found").is_err());
    }

    #[test]
    fn fails_on_empty_string() {
        assert!(extract_guid_from_powercfg("").is_err());
    }

    #[test]
    fn ignores_non_hex_guid_format() {
        let output = "GUID: zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz";
        assert!(extract_guid_from_powercfg(output).is_err());
    }
}
