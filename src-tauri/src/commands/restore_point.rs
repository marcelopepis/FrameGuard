//! Comando Tauri para criação de pontos de restauração do Windows.
//!
//! Expõe `create_restore_point` ao frontend para que tweaks e planos possam
//! criar um safety net antes de aplicar alterações ao sistema.

use serde::Serialize;

use crate::utils::restore_point::{self, RestorePointResult};

/// Resultado retornado ao frontend após tentativa de criar restore point.
#[derive(Debug, Serialize)]
pub struct RestorePointResponse {
    /// `"created"` | `"skipped"` | `"disabled"` | `"failed"`
    pub status: String,
    /// Mensagem descritiva para exibição em toast/notificação
    pub message: String,
}

/// Cria um ponto de restauração do Windows.
///
/// Chamado pelo frontend antes de aplicar tweaks ou executar planos.
/// Executa via `spawn_blocking` porque PowerShell pode levar vários segundos.
///
/// # Parâmetros
/// - `description`: Texto descritivo (ex: "Antes de aplicar: Game DVR")
///
/// # Retorna
/// `RestorePointResponse` com status e mensagem — **nunca retorna Err**
/// para não impedir a execução do tweak/plano.
#[tauri::command]
pub async fn create_restore_point(description: String) -> Result<RestorePointResponse, String> {
    let result =
        tokio::task::spawn_blocking(move || restore_point::create_restore_point(&description))
            .await
            .map_err(|e| format!("Falha no spawn_blocking: {}", e))?;

    Ok(match result {
        RestorePointResult::Created => RestorePointResponse {
            status: "created".to_string(),
            message: "Ponto de restauração criado com sucesso".to_string(),
        },
        RestorePointResult::Skipped => RestorePointResponse {
            status: "skipped".to_string(),
            message: "Ponto de restauração recente já existe (< 24h)".to_string(),
        },
        RestorePointResult::Disabled(msg) => RestorePointResponse {
            status: "disabled".to_string(),
            message: msg,
        },
        RestorePointResult::Failed(msg) => RestorePointResponse {
            status: "failed".to_string(),
            message: msg,
        },
    })
}
