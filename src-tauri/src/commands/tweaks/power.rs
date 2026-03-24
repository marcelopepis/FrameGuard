//! Tweaks de energia: Ultimate Performance, Power Throttling, Hibernation.

use serde_json::{json, Value};

use crate::commands::optimizations::{EvidenceLevel, HardwareFilter, RiskLevel, TweakInfo};
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

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — AMD Ryzen Power Plan
//
// Ativa o plano de energia otimizado para processadores AMD Ryzen.
// Se o plano AMD Ryzen Balanced (instalado via drivers de chipset) existir,
// ativa-o diretamente. Caso contrário, aplica parâmetros otimizados sobre o
// plano ativo atual via powercfg.
//
// Revert: restaura o plano Balanceado padrão do Windows.
// ═══════════════════════════════════════════════════════════════════════════════

const AMD_RYZEN_PLAN_GUID: &str = "9897998c-92de-4669-853f-b7cd3ecb2790";
const BALANCED_PLAN_GUID: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

static AMD_RYZEN_META: TweakMeta = TweakMeta {
    id: "amd_ryzen_power_plan",
    name: "AMD Ryzen Power Plan",
    description: "Ativa o plano de energia otimizado para processadores AMD Ryzen. \
        Melhora responsividade e frequências de boost comparado ao plano Balanceado \
        padrão do Windows.",
    category: "energy_cpu",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: plano Balanceado ativo",
    hardware_filter: None, // definido dinamicamente em get_info (contém String)
};

/// Verifica se o plano AMD Ryzen está ativo ou se os parâmetros manuais foram aplicados.
fn get_amd_ryzen_plan_is_applied() -> Result<bool, String> {
    let output = run_command("powercfg.exe", &["/getactivescheme"])?;
    let lower = output.stdout.to_lowercase();

    // Plano AMD Ryzen Balanced ativo diretamente
    if lower.contains(AMD_RYZEN_PLAN_GUID) {
        return Ok(true);
    }

    // Verifica se temos backup Applied (parâmetros manuais foram aplicados)
    if let Ok(backups) = get_all_backups() {
        if let Some(entry) = backups.get("amd_ryzen_power_plan") {
            if entry.status == BackupStatus::Applied {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Verifica se o plano AMD Ryzen Balanced existe no sistema.
fn amd_ryzen_plan_exists() -> Result<bool, String> {
    let output = run_command("powercfg.exe", &["/list"])?;
    Ok(output.stdout.to_lowercase().contains(AMD_RYZEN_PLAN_GUID))
}

/// Retorna informações do tweak AMD Ryzen Power Plan com estado atual.
#[tauri::command]
pub async fn get_amd_ryzen_power_plan_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_amd_ryzen_plan_is_applied().unwrap_or(false);
        let mut info = AMD_RYZEN_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: None,
            cpu_vendor: Some("amd".to_string()),
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ativa o plano AMD Ryzen Power Plan.
///
/// Se o plano AMD Ryzen Balanced (GUID 9897998c-...) estiver disponível no
/// sistema (drivers de chipset instalados), ativa-o diretamente.
/// Caso contrário, aplica parâmetros otimizados sobre o plano ativo atual.
#[tauri::command]
pub async fn enable_amd_ryzen_power_plan() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        if get_amd_ryzen_plan_is_applied()? {
            return Err("Tweak 'amd_ryzen_power_plan' já está aplicado".to_string());
        }

        let original_guid = get_active_power_scheme_guid()?;
        let plan_exists = amd_ryzen_plan_exists()?;

        // Backup do GUID ativo original + modo de aplicação
        backup_before_apply(
            "amd_ryzen_power_plan",
            TweakCategory::Powershell,
            "AMD Ryzen Power Plan — GUID do plano ativo antes da troca",
            OriginalValue {
                path: "powercfg".to_string(),
                key: "active_scheme_guid".to_string(),
                value: Some(json!(original_guid)),
                value_type: "STRING".to_string(),
            },
            json!(if plan_exists { "ryzen_plan" } else { "manual_params" }),
        )?;

        if plan_exists {
            // Plano AMD existe — ativar diretamente
            let result = run_command("powercfg.exe", &["/setactive", AMD_RYZEN_PLAN_GUID])?;
            if !result.success {
                return Err(format!(
                    "Falha ao ativar plano AMD Ryzen Balanced: {}",
                    result.stderr
                ));
            }
        } else {
            // Plano AMD não existe — aplicar parâmetros manualmente sobre o plano ativo
            let params = [
                &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PERFINCTHRESHOLD", "25"],
                &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PERFDECTHRESHOLD", "10"],
                &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PERFCHECK", "15"],
                &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PROCTHROTTLEMIN", "90"],
            ];

            for args in &params {
                let result = run_command("powercfg.exe", *args)?;
                if !result.success {
                    return Err(format!(
                        "Falha ao aplicar parâmetro powercfg {:?}: {}",
                        args, result.stderr
                    ));
                }
            }

            // Aplicar as alterações no plano ativo
            let result = run_command("powercfg.exe", &["/setactive", "scheme_current"])?;
            if !result.success {
                return Err(format!(
                    "Falha ao aplicar plano ativo: {}",
                    result.stderr
                ));
            }
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reverte o tweak AMD Ryzen Power Plan: restaura o plano Balanceado padrão do Windows.
#[tauri::command]
pub async fn revert_amd_ryzen_power_plan() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        restore_from_backup("amd_ryzen_power_plan")?;

        let result = run_command("powercfg.exe", &["/setactive", BALANCED_PLAN_GUID])?;
        if !result.success {
            return Err(format!(
                "Falha ao restaurar plano Balanceado ({}): {}",
                BALANCED_PLAN_GUID, result.stderr
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Intel Power Throttling Off
//
// HKLM\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling
//   -> PowerThrottlingOff = 1 (DWORD)
// Intel-specific: aparece apenas em CPUs Intel via hardware_filter.
// ═══════════════════════════════════════════════════════════════════════════════

static INTEL_POWER_THROTTLE_META: TweakMeta = TweakMeta {
    id: "intel_power_throttling_off",
    name: "Intel Power Throttling Off",
    description: "Desabilita o Power Throttling do Windows para CPUs Intel. Impede que o \
        sistema reduza a performance de processos em background durante gaming. Mais impacto \
        em laptops.",
    category: "energy_cpu",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description: "Padrão Windows: Power Throttling habilitado (PowerThrottlingOff = 0)",
    hardware_filter: None, // definido dinamicamente em get_info (contém String)
};

/// Verifica se o Intel Power Throttling Off está aplicado (PowerThrottlingOff = 1).
fn get_intel_power_throttling_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?.unwrap_or(0);
    Ok(val == 1)
}

/// Retorna informações do tweak Intel Power Throttling Off com estado atual.
#[tauri::command]
pub async fn get_intel_power_throttling_off_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_intel_power_throttling_is_applied()?;
        let mut info = INTEL_POWER_THROTTLE_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: None,
            cpu_vendor: Some("intel".to_string()),
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o Power Throttling para Intel escrevendo `PowerThrottlingOff = 1`.
#[tauri::command]
pub async fn enable_intel_power_throttling_off() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        if get_intel_power_throttling_is_applied()? {
            return Err("Tweak 'intel_power_throttling_off' já está aplicado".to_string());
        }

        let original = read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?;

        backup_before_apply(
            "intel_power_throttling_off",
            TweakCategory::Registry,
            "Intel Power Throttling Off — PowerThrottlingOff em HKLM\\...\\Power\\PowerThrottling",
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
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reverte o Intel Power Throttling: restaura PowerThrottlingOff = 0.
#[tauri::command]
pub async fn revert_intel_power_throttling_off() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        restore_from_backup("intel_power_throttling_off")?;

        write_dword(
            Hive::LocalMachine,
            POWER_THROTTLE_PATH,
            POWER_THROTTLE_KEY,
            0,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Intel Turbo Boost Agressivo
//
// Expõe o controle oculto de Processor Performance Boost Mode e seta para
// Aggressive (valor 2) no plano ativo.
//
// Passo 1: HKLM\...\PowerSettings\{54533251-...}\{be337238-...}\Attributes = 2
// Passo 2: powercfg /setacvalueindex scheme_current SUB_PROCESSOR PERFBOOSTMODE 2
//
// Revert: Attributes = 1, PERFBOOSTMODE = 1 (Enabled padrão)
// ═══════════════════════════════════════════════════════════════════════════════

const BOOST_MODE_SETTINGS_PATH: &str =
    r"SYSTEM\CurrentControlSet\Control\Power\PowerSettings\54533251-82be-4824-96c1-47b60b740d00\be337238-0d82-4146-a960-4f3749d470c7";
const BOOST_MODE_ATTR_KEY: &str = "Attributes";

static INTEL_TURBO_BOOST_META: TweakMeta = TweakMeta {
    id: "intel_turbo_boost_aggressive",
    name: "Intel Turbo Boost Agressivo",
    description: "Expõe e ativa o modo de Turbo Boost Agressivo nas opções de energia do \
        Windows. Permite que o processador Intel mantenha frequências mais altas por períodos \
        maiores.",
    category: "energy_cpu",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description:
        "Padrão Windows: Boost Mode = Enabled (1), controle oculto (Attributes = 1)",
    hardware_filter: None, // definido dinamicamente em get_info (contém String)
};

/// Verifica se o Turbo Boost Agressivo está aplicado:
/// Attributes = 2 (exposto) e backup Applied.
fn get_intel_turbo_boost_is_applied() -> Result<bool, String> {
    let attr = read_dword(Hive::LocalMachine, BOOST_MODE_SETTINGS_PATH, BOOST_MODE_ATTR_KEY)?
        .unwrap_or(1);

    if attr != 2 {
        return Ok(false);
    }

    // Confirma via backup que o FrameGuard aplicou
    if let Ok(backups) = get_all_backups() {
        if let Some(entry) = backups.get("intel_turbo_boost_aggressive") {
            if entry.status == BackupStatus::Applied {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Retorna informações do tweak Intel Turbo Boost Agressivo com estado atual.
#[tauri::command]
pub async fn get_intel_turbo_boost_aggressive_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_intel_turbo_boost_is_applied().unwrap_or(false);
        let mut info = INTEL_TURBO_BOOST_META.build(is_applied);
        info.hardware_filter = Some(HardwareFilter {
            gpu_vendor: None,
            cpu_vendor: Some("intel".to_string()),
        });
        Ok(info)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ativa o Turbo Boost Agressivo: expõe o controle e seta PERFBOOSTMODE = 2.
#[tauri::command]
pub async fn enable_intel_turbo_boost_aggressive() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        if get_intel_turbo_boost_is_applied()? {
            return Err("Tweak 'intel_turbo_boost_aggressive' já está aplicado".to_string());
        }

        // Salva valor original de Attributes
        let original_attr = read_dword(
            Hive::LocalMachine,
            BOOST_MODE_SETTINGS_PATH,
            BOOST_MODE_ATTR_KEY,
        )?;

        backup_before_apply(
            "intel_turbo_boost_aggressive",
            TweakCategory::Registry,
            "Intel Turbo Boost Agressivo — Attributes + PERFBOOSTMODE",
            OriginalValue {
                path: format!("HKEY_LOCAL_MACHINE\\{}", BOOST_MODE_SETTINGS_PATH),
                key: BOOST_MODE_ATTR_KEY.to_string(),
                value: original_attr.map(|v| json!(v)),
                value_type: "DWORD".to_string(),
            },
            json!(2),
        )?;

        // Passo 1: expor o controle oculto (Attributes = 2)
        write_dword(
            Hive::LocalMachine,
            BOOST_MODE_SETTINGS_PATH,
            BOOST_MODE_ATTR_KEY,
            2,
        )?;

        // Passo 2: setar PERFBOOSTMODE = 2 (Aggressive) no plano ativo
        let result = run_command(
            "powercfg.exe",
            &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PERFBOOSTMODE", "2"],
        )?;
        if !result.success {
            return Err(format!(
                "Falha ao setar PERFBOOSTMODE = 2: {}",
                result.stderr
            ));
        }

        // Aplicar alterações no plano ativo
        let result = run_command("powercfg.exe", &["/setactive", "scheme_current"])?;
        if !result.success {
            return Err(format!(
                "Falha ao aplicar plano ativo: {}",
                result.stderr
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reverte o Turbo Boost Agressivo: esconde o controle e restaura PERFBOOSTMODE = 1.
#[tauri::command]
pub async fn revert_intel_turbo_boost_aggressive() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        restore_from_backup("intel_turbo_boost_aggressive")?;

        // Restaurar Attributes = 1 (esconde o controle)
        write_dword(
            Hive::LocalMachine,
            BOOST_MODE_SETTINGS_PATH,
            BOOST_MODE_ATTR_KEY,
            1,
        )?;

        // Restaurar PERFBOOSTMODE = 1 (Enabled padrão)
        let result = run_command(
            "powercfg.exe",
            &["/setacvalueindex", "scheme_current", "SUB_PROCESSOR", "PERFBOOSTMODE", "1"],
        )?;
        if !result.success {
            return Err(format!(
                "Falha ao restaurar PERFBOOSTMODE = 1: {}",
                result.stderr
            ));
        }

        let result = run_command("powercfg.exe", &["/setactive", "scheme_current"])?;
        if !result.success {
            return Err(format!(
                "Falha ao aplicar plano ativo: {}",
                result.stderr
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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
