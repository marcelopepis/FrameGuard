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

// ─── Resumo do sistema ────────────────────────────────────

/// Dados essenciais do sistema para prova de conceito frontend↔backend
#[derive(Debug, Serialize)]
pub struct SystemSummary {
    pub os_version: String,
    pub hostname: String,
    pub is_elevated: bool,
}

/// Retorna resumo do sistema: versão do Windows, hostname e status de elevação
#[tauri::command]
pub fn get_system_summary() -> Result<SystemSummary, String> {
    let os_version = get_windows_version()
        // Fallback via sysinfo se o registro falhar
        .unwrap_or_else(|_| {
            sysinfo::System::long_os_version()
                .unwrap_or_else(|| "Windows 11".to_string())
        });

    let hostname = sysinfo::System::host_name()
        // Fallback via variável de ambiente do Windows
        .unwrap_or_else(|| {
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Desconhecido".to_string())
        });

    let is_elevated = crate::utils::elevated::is_elevated();

    Ok(SystemSummary {
        os_version,
        hostname,
        is_elevated,
    })
}

/// Lê a versão do Windows pelo registro.
/// Retorna ex: "Windows 11 Pro 23H2"
fn get_windows_version() -> Result<String, String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .map_err(|e| e.to_string())?;

    let product: String = key
        .get_value("ProductName")
        .unwrap_or_else(|_| "Windows".to_string());

    // DisplayVersion contém o canal de lançamento (ex: "23H2")
    let display_ver: String = key.get_value("DisplayVersion").unwrap_or_default();

    Ok(if display_ver.is_empty() {
        product
    } else {
        format!("{} {}", product, display_ver)
    })
}
