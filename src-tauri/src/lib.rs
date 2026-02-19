// Registro de comandos e setup do FrameGuard
mod commands;
mod utils;

use commands::{cleanup, export_import, health_check, optimizations, plans, system_info};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Informações do sistema
            system_info::get_system_info,
            system_info::get_system_usage,
            system_info::get_system_summary,
            // Otimizações — Compressão de Wallpaper
            optimizations::get_wallpaper_compression_status,
            optimizations::get_wallpaper_compression_info,
            optimizations::disable_wallpaper_compression,
            optimizations::revert_wallpaper_compression,
            // Otimizações — Armazenamento Reservado
            optimizations::get_reserved_storage_status,
            optimizations::get_reserved_storage_info,
            optimizations::disable_reserved_storage,
            optimizations::enable_reserved_storage,
            // Otimizações — Otimização de Entrega
            optimizations::get_delivery_optimization_status,
            optimizations::get_delivery_optimization_info,
            optimizations::disable_delivery_optimization,
            optimizations::revert_delivery_optimization,
            // Limpeza
            cleanup::analyze_cleanup,
            cleanup::run_cleanup,
            // Saúde do Sistema — DISM
            health_check::run_dism_cleanup,
            health_check::run_dism_checkhealth,
            health_check::run_dism_scanhealth,
            health_check::run_dism_restorehealth,
            // Saúde do Sistema — Verificações
            health_check::run_sfc,
            health_check::run_chkdsk,
            health_check::run_ssd_trim,
            // Saúde do Sistema — Manutenção
            health_check::flush_dns,
            health_check::run_temp_cleanup,
            // Planos de Execução
            plans::create_plan,
            plans::update_plan,
            plans::delete_plan,
            plans::get_plan,
            plans::get_all_plans,
            plans::execute_plan,
            // Export / Import
            export_import::export_config,
            export_import::import_config,
            export_import::validate_fg_file,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o FrameGuard");
}
