// Registro de comandos e setup do FrameGuard
mod commands;
mod utils;

use commands::{cleanup, export_import, health_check, optimizations, plans, privacy, services, system_info};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Informações do sistema
            system_info::get_static_hw_info,
            system_info::get_system_status,
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
            // Otimizações — HAGS
            optimizations::get_hags_info,
            optimizations::enable_hags,
            optimizations::disable_hags,
            // Otimizações — Game Mode
            optimizations::get_game_mode_info,
            optimizations::enable_game_mode,
            optimizations::disable_game_mode,
            // Otimizações — VBS
            optimizations::get_vbs_info,
            optimizations::disable_vbs,
            optimizations::enable_vbs,
            // Otimizações — Restaurar Padrão Windows (sem backup)
            optimizations::restore_wallpaper_default,
            optimizations::restore_delivery_optimization_default,
            optimizations::restore_reserved_storage_default,
            // Otimizações — GPU e Display (placeholders)
            optimizations::get_game_dvr_info,
            optimizations::disable_game_dvr,
            optimizations::revert_game_dvr,
            optimizations::get_xbox_overlay_info,
            optimizations::disable_xbox_overlay,
            optimizations::revert_xbox_overlay,
            optimizations::get_msi_mode_gpu_info,
            optimizations::enable_msi_mode_gpu,
            optimizations::disable_msi_mode_gpu,
            optimizations::get_mpo_info,
            optimizations::disable_mpo,
            optimizations::revert_mpo,
            optimizations::get_nvidia_telemetry_info,
            optimizations::disable_nvidia_telemetry,
            optimizations::revert_nvidia_telemetry,
            // Otimizações — Gaming (placeholders)
            optimizations::get_timer_resolution_info,
            optimizations::enable_timer_resolution,
            optimizations::disable_timer_resolution,
            optimizations::get_mouse_acceleration_info,
            optimizations::disable_mouse_acceleration,
            optimizations::revert_mouse_acceleration,
            optimizations::get_fullscreen_optimizations_info,
            optimizations::disable_fullscreen_optimizations,
            optimizations::revert_fullscreen_optimizations,
            // Otimizações — Energia e CPU (placeholders)
            optimizations::get_ultimate_performance_info,
            optimizations::enable_ultimate_performance,
            optimizations::revert_ultimate_performance,
            optimizations::get_power_throttling_info,
            optimizations::disable_power_throttling,
            optimizations::revert_power_throttling,
            // Otimizações — Armazenamento (placeholders)
            optimizations::get_hibernation_info,
            optimizations::disable_hibernation,
            optimizations::enable_hibernation,
            optimizations::get_ntfs_last_access_info,
            optimizations::disable_ntfs_last_access,
            optimizations::revert_ntfs_last_access,
            // Otimizações — Rede (placeholders)
            optimizations::get_nagle_info,
            optimizations::disable_nagle,
            optimizations::revert_nagle,
            // Otimizações — Visual e Experiência (placeholders)
            optimizations::get_sticky_keys_info,
            optimizations::disable_sticky_keys,
            optimizations::revert_sticky_keys,
            optimizations::get_bing_search_info,
            optimizations::disable_bing_search,
            optimizations::revert_bing_search,
            // Privacidade — Telemetria
            privacy::get_telemetry_registry_info,
            privacy::disable_telemetry_registry,
            privacy::revert_telemetry_registry,
            // Privacidade — Copilot e Cortana
            privacy::get_copilot_info,
            privacy::disable_copilot,
            privacy::revert_copilot,
            // Privacidade — Content Delivery Manager
            privacy::get_content_delivery_info,
            privacy::disable_content_delivery,
            privacy::revert_content_delivery,
            // Privacidade — Background Apps
            privacy::get_background_apps_info,
            privacy::disable_background_apps,
            privacy::revert_background_apps,
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
            // Serviços e Tarefas Agendadas
            services::get_services_status,
            services::disable_services,
            services::restore_services,
            services::get_tasks_status,
            services::disable_tasks,
            services::restore_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o FrameGuard");
}
