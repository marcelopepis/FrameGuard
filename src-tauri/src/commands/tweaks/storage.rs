//! Tweaks de armazenamento: Reserved Storage, NTFS Last Access.

use serde_json::{json, Value};

use crate::commands::optimizations::{EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::command_runner::{run_command, run_command_with_progress};
use crate::utils::tweak_builder::TweakMeta;

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Armazenamento Reservado (DISM)
// ═══════════════════════════════════════════════════════════════════════════════

static RESERVED_STORAGE_META: TweakMeta = TweakMeta {
    id: "disable_reserved_storage",
    name: "Recuperar Armazenamento Reservado",
    description: "Recupera o espaço de armazenamento reservado pelo Windows para \
        atualizações. O Windows reserva cerca de 7GB do disco para garantir que updates \
        possam ser instalados. Se você prefere gerenciar isso manualmente, pode desabilitar \
        e recuperar este espaço.",
    category: "optimization",
    requires_restart: false,
    risk_level: RiskLevel::Medium,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: Armazenamento Reservado habilitado (~7 GB)",
    hardware_filter: None,
};

/// Verifica se o armazenamento reservado está habilitado via DISM.
fn check_reserved_storage_enabled() -> Result<bool, String> {
    let output = run_command("dism.exe", &["/Online", "/Get-ReservedStorageState"])?;
    let stdout_lower = output.stdout.to_lowercase();
    Ok(stdout_lower.contains("enabled") && !stdout_lower.contains("disabled"))
}

/// Retorna informações do tweak Armazenamento Reservado com estado atual.
///
/// `is_applied = true` quando o armazenamento reservado está **desabilitado**.
#[tauri::command]
pub async fn get_reserved_storage_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let enabled = check_reserved_storage_enabled()?;
        Ok(RESERVED_STORAGE_META.build(!enabled))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o armazenamento reservado via DISM com streaming de progresso.
#[tauri::command]
pub fn disable_reserved_storage(app_handle: tauri::AppHandle) -> Result<(), String> {
    let enabled = check_reserved_storage_enabled()?;
    if !enabled {
        return Err("Armazenamento reservado já está desabilitado — tweak já aplicado".to_string());
    }

    backup_before_apply(
        "disable_reserved_storage",
        TweakCategory::Dism,
        "Estado do armazenamento reservado — DISM /Online /Get-ReservedStorageState",
        OriginalValue {
            path: "DISM /Online".to_string(),
            key: "ReservedStorageState".to_string(),
            value: Some(json!("Enabled")),
            value_type: "STATE".to_string(),
        },
        json!("Disabled"),
    )?;

    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Disabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Disabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

/// Reabilita o armazenamento reservado do Windows via DISM.
#[tauri::command]
pub fn enable_reserved_storage(app_handle: tauri::AppHandle) -> Result<(), String> {
    restore_from_backup("disable_reserved_storage")?;

    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Enabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Enabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

/// Reabilita o Armazenamento Reservado via DISM sem precisar de backup.
#[tauri::command]
pub fn restore_reserved_storage_default(app_handle: tauri::AppHandle) -> Result<(), String> {
    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Enabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Enabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — NTFS Last Access Timestamp
// ═══════════════════════════════════════════════════════════════════════════════

static NTFS_LAST_ACCESS_META: TweakMeta = TweakMeta {
    id: "disable_ntfs_last_access",
    name: "Desabilitar Timestamp de Último Acesso NTFS",
    description: "Impede o NTFS de atualizar o timestamp de último acesso em cada leitura \
        de arquivo. Reduz operações de escrita no disco. No Windows 11, volumes >128GB já \
        têm isso desabilitado por padrão, mas este tweak garante a configuração \
        independente do tamanho.",
    category: "storage",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Plausible,
    default_value_description:
        "Padrão Windows: timestamps habilitados (0) ou desabilitados pelo sistema (2) em volumes grandes",
    hardware_filter: None,
};

/// Consulta o valor atual de `disablelastaccess` via `fsutil behavior query`.
fn query_ntfs_last_access() -> Result<u32, String> {
    let result = run_command("fsutil.exe", &["behavior", "query", "disablelastaccess"])?;

    if !result.success {
        return Err(format!(
            "fsutil behavior query falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    let output = result.stdout.trim().to_string();
    let mut iter = output.splitn(2, '=');
    let _ = iter.next();
    if let Some(rhs) = iter.next() {
        if let Some(tok) = rhs.split_whitespace().next() {
            if let Ok(n) = tok.parse::<u32>() {
                return Ok(n);
            }
        }
    }

    Err(format!(
        "Não foi possível parsear saída do fsutil: '{}'",
        output
    ))
}

/// Retorna informações do tweak NTFS Last Access com estado atual.
#[tauri::command]
pub async fn get_ntfs_last_access_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = query_ntfs_last_access().map(|v| v == 1).unwrap_or(false);
        Ok(NTFS_LAST_ACCESS_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita os timestamps de último acesso NTFS via `fsutil behavior set disablelastaccess 1`.
#[tauri::command]
pub fn disable_ntfs_last_access() -> Result<(), String> {
    let current_val = query_ntfs_last_access()?;
    if current_val == 1 {
        return Err(
            "Tweak 'disable_ntfs_last_access' já está aplicado (disablelastaccess = 1)".to_string(),
        );
    }

    backup_before_apply(
        "disable_ntfs_last_access",
        TweakCategory::Powershell,
        "NtfsDisableLastAccessUpdate — controla atualização de timestamps de leitura NTFS",
        OriginalValue {
            path: "fsutil behavior".to_string(),
            key: "disablelastaccess".to_string(),
            value: Some(json!(current_val)),
            value_type: "FSUTIL_STATE".to_string(),
        },
        json!(1u32),
    )?;

    let result = run_command("fsutil.exe", &["behavior", "set", "disablelastaccess", "1"])?;
    if !result.success {
        return Err(format!(
            "fsutil behavior set falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

/// Reverte o timestamp de último acesso para o valor original salvo no backup.
#[tauri::command]
pub fn revert_ntfs_last_access() -> Result<(), String> {
    let original = restore_from_backup("disable_ntfs_last_access")?;

    let original_val = match original.value {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0) as u32,
        _ => 0,
    };

    let val_str = original_val.to_string();
    let result = run_command(
        "fsutil.exe",
        &["behavior", "set", "disablelastaccess", val_str.as_str()],
    )?;
    if !result.success {
        return Err(format!(
            "fsutil behavior set (reversão) falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}
