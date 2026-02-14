// Leitura e escrita no registro do Windows
use winreg::enums::*;
use winreg::{HKEY, RegKey};

/// Lê um valor string de uma chave do registro
pub fn read_string(hkey: HKEY, subkey: &str, value_name: &str) -> Result<String, String> {
    let key = RegKey::predef(hkey)
        .open_subkey(subkey)
        .map_err(|e| format!("Erro ao abrir chave do registro: {}", e))?;

    key.get_value::<String, _>(value_name)
        .map_err(|e| format!("Erro ao ler valor '{}': {}", value_name, e))
}

/// Lê um valor DWORD de uma chave do registro
pub fn read_dword(hkey: HKEY, subkey: &str, value_name: &str) -> Result<u32, String> {
    let key = RegKey::predef(hkey)
        .open_subkey(subkey)
        .map_err(|e| format!("Erro ao abrir chave do registro: {}", e))?;

    key.get_value::<u32, _>(value_name)
        .map_err(|e| format!("Erro ao ler valor '{}': {}", value_name, e))
}

/// Escreve um valor DWORD em uma chave do registro
pub fn write_dword(hkey: HKEY, subkey: &str, value_name: &str, data: u32) -> Result<(), String> {
    let key = RegKey::predef(hkey)
        .open_subkey_with_flags(subkey, KEY_SET_VALUE)
        .map_err(|e| format!("Erro ao abrir chave do registro para escrita: {}", e))?;

    key.set_value(value_name, &data)
        .map_err(|e| format!("Erro ao escrever valor '{}': {}", value_name, e))
}
