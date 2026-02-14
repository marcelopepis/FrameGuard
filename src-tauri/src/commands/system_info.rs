// Informações do sistema (CPU, GPU, RAM, OS)
use serde::Serialize;

/// Informações gerais do sistema
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub ram_total_gb: f64,
    pub ram_used_gb: f64,
    pub gpu_name: String,
}

/// Retorna as informações do sistema para o dashboard
#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    // TODO: implementar coleta real via sysinfo + WMI
    Ok(SystemInfo {
        os_name: "Windows 11".to_string(),
        os_version: "placeholder".to_string(),
        cpu_name: "placeholder".to_string(),
        cpu_cores: 0,
        ram_total_gb: 0.0,
        ram_used_gb: 0.0,
        gpu_name: "placeholder".to_string(),
    })
}

/// Informações de uso atual de CPU e RAM
#[derive(Debug, Serialize)]
pub struct SystemUsage {
    pub cpu_usage_percent: f32,
    pub ram_usage_percent: f32,
}

/// Retorna o uso atual de CPU e RAM
#[tauri::command]
pub fn get_system_usage() -> Result<SystemUsage, String> {
    // TODO: implementar coleta em tempo real via sysinfo
    Ok(SystemUsage {
        cpu_usage_percent: 0.0,
        ram_usage_percent: 0.0,
    })
}
