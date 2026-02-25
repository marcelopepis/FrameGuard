//! Comandos Tauri para o log de atividade recente.

use crate::utils::activity_log::{self, ActivityEntry};

/// Registra uma atividade de tweak individual no log.
///
/// Chamado pelo frontend após aplicar ou reverter um tweak com sucesso/falha.
///
/// # Parâmetros
/// - `name`: nome legível do tweak (ex: "Desabilitar VBS")
/// - `applied`: `true` para aplicação, `false` para reversão
/// - `success`: `true` se a operação foi bem-sucedida
#[tauri::command]
pub fn log_tweak_activity(name: String, applied: bool, success: bool) -> Result<(), String> {
    let entry = activity_log::tweak_entry(&name, applied, success);
    activity_log::log_activity(entry)
}

/// Retorna as atividades mais recentes para exibição no Dashboard.
///
/// # Parâmetros
/// - `limit`: número máximo de entradas a retornar
#[tauri::command]
pub fn get_recent_activity(limit: u32) -> Result<Vec<ActivityEntry>, String> {
    activity_log::get_recent(limit)
}
