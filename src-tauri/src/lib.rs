// Registro de comandos e setup do FrameGuard
mod commands;
mod utils;

use commands::{
    about, activity, bloatware, cleanup, export_import, health, plans, privacy, restore_point,
    services, system_info, tweaks, window,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|_app| {
            // Pre-warm de TODOS os caches estáticos em background.
            // Roda em paralelo — a janela abre imediatamente enquanto
            // os dados são coletados. Dashboard mostra skeletons.
            system_info::pre_warm_all_caches();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Informações do sistema
            system_info::get_static_hw_info,
            system_info::get_gpu_info,
            system_info::get_detected_vendors,
            system_info::get_system_status,
            system_info::get_system_usage,
            system_info::get_system_summary,
            // Otimizações — Compressão de Wallpaper (tweaks/visual)
            tweaks::get_wallpaper_compression_info,
            tweaks::disable_wallpaper_compression,
            tweaks::revert_wallpaper_compression,
            // Otimizações — Armazenamento Reservado (tweaks/storage)
            tweaks::get_reserved_storage_info,
            tweaks::disable_reserved_storage,
            tweaks::enable_reserved_storage,
            // Otimizações — Otimização de Entrega (tweaks/network)
            tweaks::get_delivery_optimization_info,
            tweaks::disable_delivery_optimization,
            tweaks::revert_delivery_optimization,
            // Otimizações — HAGS (tweaks/gpu)
            tweaks::get_hags_info,
            tweaks::enable_hags,
            tweaks::disable_hags,
            // Otimizações — Game Mode (tweaks/gaming)
            tweaks::get_game_mode_info,
            tweaks::enable_game_mode,
            tweaks::disable_game_mode,
            // Otimizações — VBS (tweaks/gaming)
            tweaks::get_vbs_info,
            tweaks::disable_vbs,
            tweaks::enable_vbs,
            // Otimizações — Restaurar Padrão Windows (sem backup)
            tweaks::restore_wallpaper_default,
            tweaks::restore_delivery_optimization_default,
            tweaks::restore_reserved_storage_default,
            // Otimizações — Game DVR e Xbox Overlay (tweaks/gaming)
            tweaks::get_game_dvr_info,
            tweaks::disable_game_dvr,
            tweaks::revert_game_dvr,
            tweaks::get_xbox_overlay_info,
            tweaks::disable_xbox_overlay,
            tweaks::revert_xbox_overlay,
            tweaks::get_msi_mode_gpu_info,
            tweaks::enable_msi_mode_gpu,
            tweaks::disable_msi_mode_gpu,
            tweaks::get_mpo_info,
            tweaks::disable_mpo,
            tweaks::revert_mpo,
            tweaks::get_nvidia_telemetry_info,
            tweaks::disable_nvidia_telemetry,
            tweaks::revert_nvidia_telemetry,
            // Otimizações — Gaming (tweaks/gaming)
            tweaks::get_timer_resolution_info,
            tweaks::enable_timer_resolution,
            tweaks::disable_timer_resolution,
            tweaks::get_mouse_acceleration_info,
            tweaks::disable_mouse_acceleration,
            tweaks::revert_mouse_acceleration,
            tweaks::get_fullscreen_optimizations_info,
            tweaks::disable_fullscreen_optimizations,
            tweaks::revert_fullscreen_optimizations,
            // Otimizações — Energia e CPU (tweaks/power)
            tweaks::get_ultimate_performance_info,
            tweaks::enable_ultimate_performance,
            tweaks::revert_ultimate_performance,
            tweaks::get_power_throttling_info,
            tweaks::disable_power_throttling,
            tweaks::revert_power_throttling,
            // Otimizações — Armazenamento (tweaks/power + tweaks/storage)
            tweaks::get_hibernation_info,
            tweaks::disable_hibernation,
            tweaks::enable_hibernation,
            tweaks::get_ntfs_last_access_info,
            tweaks::disable_ntfs_last_access,
            tweaks::revert_ntfs_last_access,
            // Otimizações — Rede (tweaks/network)
            tweaks::get_nagle_info,
            tweaks::disable_nagle,
            tweaks::revert_nagle,
            // Otimizações — Visual e Experiência (tweaks/visual)
            tweaks::get_sticky_keys_info,
            tweaks::disable_sticky_keys,
            tweaks::revert_sticky_keys,
            tweaks::get_bing_search_info,
            tweaks::disable_bing_search,
            tweaks::revert_bing_search,
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
            cleanup::scan_cleanup,
            cleanup::execute_cleanup,
            // Saúde do Sistema — DISM
            health::run_dism_cleanup,
            health::run_dism_checkhealth,
            health::run_dism_scanhealth,
            health::run_dism_restorehealth,
            // Saúde do Sistema — Verificações
            health::run_sfc,
            health::run_chkdsk,
            health::run_ssd_trim,
            // Saúde do Sistema — Manutenção
            health::flush_dns,
            health::run_temp_cleanup,
            health::kill_process,
            // Planos de Execução
            plans::create_plan,
            plans::update_plan,
            plans::delete_plan,
            plans::duplicate_plan,
            plans::get_plan,
            plans::get_all_plans,
            plans::execute_plan,
            // Export / Import
            export_import::export_config,
            export_import::import_config,
            export_import::validate_fg_file,
            // Atividade Recente
            activity::log_tweak_activity,
            activity::get_recent_activity,
            // Serviços e Tarefas Agendadas
            services::get_services_status,
            services::disable_services,
            services::restore_services,
            services::get_tasks_status,
            services::disable_tasks,
            services::restore_tasks,
            // Ponto de Restauração
            restore_point::create_restore_point,
            // Remoção de Bloatware UWP
            bloatware::get_installed_uwp_apps,
            bloatware::remove_uwp_apps,
            // Sobre / Atualizações
            about::check_for_updates,
            // Controle de Janela (titlebar customizada)
            window::minimize_window,
            window::maximize_window,
            window::close_window,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o FrameGuard");
}
