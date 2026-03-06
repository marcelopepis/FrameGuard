//! Manipulação do registro do Windows.
//!
//! Funções de leitura, escrita e remoção de valores em hives do registro,
//! com tratamento uniforme de erros e criação automática de subchaves.

use std::io;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE};
use winreg::RegKey;

// ─── Hive ──────────────────────────────────────────────────────────────────────

/// Identifica a hive raiz do registro a ser acessada.
#[derive(Debug, Clone, Copy)]
pub enum Hive {
    /// `HKEY_CURRENT_USER` — configurações do usuário atual; não requer elevação.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE` — configurações do sistema; requer admin para escrita.
    LocalMachine,
}

impl Hive {
    /// Retorna um `RegKey` para a hive raiz correspondente.
    fn as_regkey(self) -> RegKey {
        RegKey::predef(match self {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        })
    }

    /// Retorna o nome legível da hive para uso em mensagens de erro.
    fn display_name(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKEY_CURRENT_USER",
            Hive::LocalMachine => "HKEY_LOCAL_MACHINE",
        }
    }
}

// ─── Helpers internos ─────────────────────────────────────────────────────────

/// Retorna `true` se o erro de IO indica chave ou valor não encontrado.
/// Cobre tanto `ERROR_FILE_NOT_FOUND` quanto `ERROR_PATH_NOT_FOUND` do Windows.
fn is_not_found(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound
}

/// Monta o caminho completo hive + subchave para uso em mensagens de erro.
fn full_path(hive: Hive, path: &str) -> String {
    format!("{}\\{}", hive.display_name(), path)
}

// ─── API pública ──────────────────────────────────────────────────────────────

/// Lê um valor DWORD (u32) do registro.
///
/// Retorna `Ok(None)` se a subchave ou o valor não existirem — ausência
/// de valor não é tratada como falha. Retorna `Err` apenas para problemas
/// de acesso ou dados corrompidos.
///
/// # Exemplo
/// ```ignore
/// let val = read_dword(Hive::CurrentUser, r"Control Panel\Desktop", "JPEGImportQuality")?;
/// // Ok(Some(100)) se existir, Ok(None) se não existir
/// ```
pub fn read_dword(hive: Hive, path: &str, key: &str) -> Result<Option<u32>, String> {
    let subkey = match hive.as_regkey().open_subkey(path) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("Erro ao abrir '{}': {}", full_path(hive, path), e)),
    };

    match subkey.get_value::<u32, _>(key) {
        Ok(v) => Ok(Some(v)),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(format!(
            "Erro ao ler DWORD '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )),
    }
}

/// Escreve um valor DWORD (u32) no registro.
///
/// Cria a subchave automaticamente se ela não existir (equivalente a
/// `RegCreateKeyEx`). Para `HKEY_LOCAL_MACHINE`, o processo deve estar
/// rodando como administrador.
pub fn write_dword(hive: Hive, path: &str, key: &str, value: u32) -> Result<(), String> {
    let (subkey, _) = hive.as_regkey().create_subkey(path).map_err(|e| {
        format!(
            "Erro ao criar/abrir '{}' para escrita: {}",
            full_path(hive, path),
            e
        )
    })?;

    subkey.set_value(key, &value).map_err(|e| {
        format!(
            "Erro ao escrever DWORD '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )
    })
}

/// Lê um valor String (`REG_SZ`) do registro.
///
/// Retorna `Ok(None)` se a subchave ou o valor não existirem.
/// Retorna `Err` para problemas de acesso ou se o tipo armazenado
/// não for compatível com String.
#[allow(dead_code)]
pub fn read_string(hive: Hive, path: &str, key: &str) -> Result<Option<String>, String> {
    let subkey = match hive.as_regkey().open_subkey(path) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("Erro ao abrir '{}': {}", full_path(hive, path), e)),
    };

    match subkey.get_value::<String, _>(key) {
        Ok(v) => Ok(Some(v)),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(format!(
            "Erro ao ler String '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )),
    }
}

/// Escreve um valor String (`REG_SZ`) no registro.
///
/// Cria a subchave automaticamente se ela não existir.
#[allow(dead_code)]
pub fn write_string(hive: Hive, path: &str, key: &str, value: &str) -> Result<(), String> {
    let (subkey, _) = hive.as_regkey().create_subkey(path).map_err(|e| {
        format!(
            "Erro ao criar/abrir '{}' para escrita: {}",
            full_path(hive, path),
            e
        )
    })?;

    subkey.set_value(key, &value).map_err(|e| {
        format!(
            "Erro ao escrever String '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )
    })
}

/// Remove um valor do registro.
///
/// Abre a subchave com permissão de escrita (`KEY_SET_VALUE`) e remove
/// o valor especificado. Retorna `Err` se a subchave não existir ou o
/// valor não for encontrado.
///
/// Para remover o valor de forma idempotente (sem erro em caso de ausência),
/// chame [`key_exists`] antes e só prossiga se retornar `true`.
pub fn delete_value(hive: Hive, path: &str, key: &str) -> Result<(), String> {
    let subkey = hive
        .as_regkey()
        .open_subkey_with_flags(path, KEY_SET_VALUE)
        .map_err(|e| {
            format!(
                "Erro ao abrir '{}' para remoção: {}",
                full_path(hive, path),
                e
            )
        })?;

    subkey.delete_value(key).map_err(|e| {
        format!(
            "Erro ao remover valor '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )
    })
}

/// Verifica se um valor específico existe no registro, independente do tipo.
///
/// Retorna `Ok(false)` — sem erro — se a subchave não existir ou o valor
/// não estiver presente. Usa `get_raw_value` para não depender do tipo do dado.
/// Retorna `Err` apenas para erros de acesso ou permissão.
pub fn key_exists(hive: Hive, path: &str, key: &str) -> Result<bool, String> {
    let subkey = match hive.as_regkey().open_subkey(path) {
        Ok(k) => k,
        Err(e) if is_not_found(&e) => return Ok(false),
        Err(e) => {
            return Err(format!(
                "Erro ao verificar existência em '{}': {}",
                full_path(hive, path),
                e
            ))
        }
    };

    match subkey.get_raw_value(key) {
        Ok(_) => Ok(true),
        Err(e) if is_not_found(&e) => Ok(false),
        Err(e) => Err(format!(
            "Erro ao verificar valor '{}' em '{}': {}",
            key,
            full_path(hive, path),
            e
        )),
    }
}
