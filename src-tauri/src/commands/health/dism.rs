//! Comandos DISM para manutenção do Component Store do Windows.
//!
//! Quatro operações progressivas:
//! - **Cleanup** — limpa versões antigas de componentes
//! - **CheckHealth** — verificação rápida de integridade
//! - **ScanHealth** — varredura profunda (2-15 min)
//! - **RestoreHealth** — reparo via Windows Update

use std::time::Instant;

use crate::utils::command_runner::run_command_with_progress;

use super::{now_utc, ps_utf8_dism, CheckStatus, HealthCheckResult};

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /StartComponentCleanup
// ═══════════════════════════════════════════════════════════════════════════════

/// Limpa componentes antigos de atualizações via DISM StartComponentCleanup.
#[tauri::command]
pub async fn run_dism_cleanup(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) =
            ps_utf8_dism("/Online /Cleanup-Image /StartComponentCleanup");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_cleanup_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if result.success
            && (stdout_lower.contains("operation completed successfully")
                || stdout_lower.contains("concluída com êxito")
                || stdout_lower.contains("foi concluída"))
        {
            (
                CheckStatus::Success,
                "Component Store limpo com sucesso. Espaço de versões antigas liberado."
                    .to_string(),
            )
        } else if result.success {
            (
                CheckStatus::Warning,
                "DISM concluiu mas não foi possível confirmar o resultado.".to_string(),
            )
        } else {
            (
                CheckStatus::Error,
                format!(
                    "DISM falhou com código {}. Verifique se há atualizações pendentes.",
                    result.exit_code
                ),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_cleanup".to_string(),
            name: "DISM Cleanup".to_string(),
            status,
            message,
            details: result.stdout,
            duration_seconds: duration,
            space_freed_mb: None,
            timestamp: ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /CheckHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Verificação rápida de integridade do Component Store via DISM CheckHealth.
#[tauri::command]
pub async fn run_dism_checkhealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /CheckHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_checkhealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        // Detecta os três estados possíveis em inglês (EN) e PT-BR
        let (status, message) = if !result.success {
            (
                CheckStatus::Error,
                format!("DISM falhou com código {}.", result.exit_code),
            )
        } else if stdout_lower.contains("no component store corruption detected")
            || stdout_lower.contains("não está danificado")
        {
            (
                CheckStatus::Success,
                "Nenhuma corrupção detectada no Component Store.".to_string(),
            )
        } else if stdout_lower.contains("repairable") || stdout_lower.contains("reparável") {
            (
                CheckStatus::Warning,
                "Corrupção detectada no Component Store. Execute DISM RestoreHealth para reparar."
                    .to_string(),
            )
        } else if stdout_lower.contains("corrupted")
            || stdout_lower.contains("corrompido")
            || stdout_lower.contains("danificado")
        {
            (
                CheckStatus::Error,
                "Component Store corrompido. Execute DISM RestoreHealth imediatamente.".to_string(),
            )
        } else {
            (
                CheckStatus::Warning,
                "Não foi possível determinar o estado do Component Store. Verifique os detalhes."
                    .to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_checkhealth".to_string(),
            name: "DISM CheckHealth".to_string(),
            status,
            message,
            details: result.stdout,
            duration_seconds: duration,
            space_freed_mb: None,
            timestamp: ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /ScanHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Varredura profunda de integridade do Component Store via DISM ScanHealth.
#[tauri::command]
pub async fn run_dism_scanhealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /ScanHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_scanhealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        // ScanHealth usa as mesmas mensagens de diagnóstico que o CheckHealth
        let (status, message) = if !result.success {
            (
                CheckStatus::Error,
                format!("DISM ScanHealth falhou com código {}.", result.exit_code),
            )
        } else if stdout_lower.contains("no component store corruption detected")
            || stdout_lower.contains("não está danificado")
        {
            (
                CheckStatus::Success,
                "Nenhuma corrupção encontrada na varredura completa do Component Store."
                    .to_string(),
            )
        } else if stdout_lower.contains("repairable") || stdout_lower.contains("reparável") {
            (
                CheckStatus::Warning,
                "Corrupção reparável encontrada. Use DISM RestoreHealth para corrigir.".to_string(),
            )
        } else if stdout_lower.contains("corrupted")
            || stdout_lower.contains("corrompido")
            || stdout_lower.contains("danificado")
        {
            (
                CheckStatus::Error,
                "Corrupção grave detectada no Component Store. Reparo necessário.".to_string(),
            )
        } else {
            (
                CheckStatus::Warning,
                "Varredura concluída. Resultado indeterminado — verifique os detalhes.".to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_scanhealth".to_string(),
            name: "DISM ScanHealth".to_string(),
            status,
            message,
            details: result.stdout,
            duration_seconds: duration,
            space_freed_mb: None,
            timestamp: ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /RestoreHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Repara o Component Store via DISM RestoreHealth usando o Windows Update.
#[tauri::command]
pub async fn run_dism_restorehealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /RestoreHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_restorehealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if result.success
            && (stdout_lower.contains("operation completed successfully")
                || stdout_lower.contains("concluída com êxito")
                || stdout_lower.contains("foi concluída"))
        {
            (
                CheckStatus::Success,
                "Component Store reparado com sucesso via Windows Update.".to_string(),
            )
        } else if !result.success
            && (stdout_lower.contains("source files could not be found")
                || stdout_lower.contains("arquivos de origem não foram encontrados")
                || stdout_lower.contains("ficheiros de origem não foram encontrados"))
        {
            (
                CheckStatus::Error,
                "Arquivos de origem não encontrados. Verifique a conexão com a internet."
                    .to_string(),
            )
        } else if !result.success {
            (
                CheckStatus::Error,
                format!(
                    "DISM RestoreHealth falhou com código {}. Verifique a conexão.",
                    result.exit_code
                ),
            )
        } else {
            (
                CheckStatus::Warning,
                "DISM concluiu mas não foi possível confirmar a conclusão do reparo.".to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_restorehealth".to_string(),
            name: "DISM RestoreHealth".to_string(),
            status,
            message,
            details: result.stdout,
            duration_seconds: duration,
            space_freed_mb: None,
            timestamp: ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}
