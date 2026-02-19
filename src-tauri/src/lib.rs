// Registro de comandos e setup do FrameGuard
mod commands;
mod utils;

use commands::{cleanup, health_check, optimizations, system_info};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Informações do sistema
            system_info::get_system_info,
            system_info::get_system_usage,
            system_info::get_system_summary,
            // Otimizações
            optimizations::get_optimizations,
            optimizations::toggle_optimization,
            // Limpeza
            cleanup::analyze_cleanup,
            cleanup::run_cleanup,
            // Verificação de saúde
            health_check::run_health_check,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o FrameGuard");
}
