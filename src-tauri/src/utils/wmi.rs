//! Consultas WMI (Windows Management Instrumentation) para informações de hardware.
//!
//! Fornece wrappers simplificados sobre a crate `wmi` para inicializar uma conexão
//! COM e executar consultas WQL tipadas. Usado internamente por `system_info` para
//! obter dados de CPU, RAM e outros componentes quando o acesso direto ao registro
//! não é viável.

use serde::Deserialize;
use wmi::{COMLibrary, WMIConnection};

/// Inicializa a biblioteca COM e cria uma conexão WMI com o namespace padrão
/// (`ROOT\CIMV2`).
///
/// Deve ser chamada uma vez por thread — a `COMLibrary` inicializa COM via
/// `CoInitializeEx` e a conexão é vinculada à thread atual.
///
/// # Erros
/// Retorna `Err` se a inicialização COM falhar (ex: já inicializada com
/// modelo de threading incompatível) ou se a conexão WMI não puder ser
/// estabelecida.
pub fn create_connection() -> Result<WMIConnection, String> {
    let com = COMLibrary::new()
        .map_err(|e| format!("Erro ao inicializar COM: {}", e))?;

    WMIConnection::new(com)
        .map_err(|e| format!("Erro ao conectar ao WMI: {}", e))
}

/// Executa uma consulta WQL genérica e retorna os resultados deserializados.
///
/// O tipo `T` determina a classe WMI consultada e os campos retornados —
/// a crate `wmi` infere a query `SELECT * FROM <classe>` a partir dos campos
/// anotados com `#[serde(rename = "...")]` na struct de destino.
///
/// # Erros
/// Retorna `Err` se a consulta WQL falhar (ex: classe inexistente, permissão
/// negada, timeout de conexão).
pub fn query<T>(connection: &WMIConnection) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    connection
        .query()
        .map_err(|e| format!("Erro na consulta WMI: {}", e))
}
