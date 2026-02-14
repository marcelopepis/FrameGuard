// Consultas WMI para informações de hardware
use serde::Deserialize;
use wmi::{COMLibrary, WMIConnection};

/// Inicializa a conexão WMI
pub fn create_connection() -> Result<WMIConnection, String> {
    let com = COMLibrary::new()
        .map_err(|e| format!("Erro ao inicializar COM: {}", e))?;

    WMIConnection::new(com)
        .map_err(|e| format!("Erro ao conectar ao WMI: {}", e))
}

/// Executa uma consulta WMI genérica e retorna os resultados
pub fn query<T>(connection: &WMIConnection) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    connection
        .query()
        .map_err(|e| format!("Erro na consulta WMI: {}", e))
}
