//! Tweaks visuais e UX: Wallpaper Compression, Sticky Keys, Bing Search.

use serde_json::{json, Value};

use crate::commands::optimizations::{EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::registry::{
    delete_value, key_exists, read_dword, read_string, write_dword, write_string, Hive,
};
use crate::utils::tweak_builder::TweakMeta;

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Compressão de Wallpaper
//
// HKCU\Control Panel\Desktop -> JPEGImportQuality
//   100 = sem compressão (tweak aplicado)
//   ausente = Windows aplica 85% internamente
// ═══════════════════════════════════════════════════════════════════════════════

const WALLPAPER_REG_PATH: &str = r"Control Panel\Desktop";
const WALLPAPER_REG_KEY: &str = "JPEGImportQuality";
const WALLPAPER_QUALITY_MAX: u32 = 100;

static WALLPAPER_META: TweakMeta = TweakMeta {
    id: "disable_wallpaper_compression",
    name: "Desabilitar Compressão de Wallpaper",
    description: "Desabilita a compressão automática de imagens JPEG usadas como papel de \
        parede. O Windows reduz a qualidade para 85% por padrão. Este tweak mantém a \
        qualidade original da imagem (100%).",
    category: "optimization",
    requires_restart: true,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Unproven,
    default_value_description: "Padrão Windows: compressão JPEG em 85% (JPEGImportQuality ausente)",
    hardware_filter: None,
};

/// Verifica se a compressão de wallpaper está desabilitada (qualidade = 100).
fn check_wallpaper_compression_disabled() -> Result<bool, String> {
    let current_value =
        read_dword(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?.unwrap_or(0);
    Ok(current_value == WALLPAPER_QUALITY_MAX)
}

/// Retorna informações do tweak Compressão de Wallpaper com estado atual.
#[tauri::command]
pub async fn get_wallpaper_compression_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = check_wallpaper_compression_disabled()?;
        Ok(WALLPAPER_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita a compressão de wallpaper definindo `JPEGImportQuality = 100`.
#[tauri::command]
pub fn disable_wallpaper_compression() -> Result<(), String> {
    if check_wallpaper_compression_disabled()? {
        return Err(
            "Tweak 'disable_wallpaper_compression' já está aplicado (qualidade = 100)".to_string(),
        );
    }

    let original_dword = read_dword(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
    let original_json: Option<Value> = original_dword.map(|v| json!(v));

    backup_before_apply(
        "disable_wallpaper_compression",
        TweakCategory::Registry,
        "JPEGImportQuality em HKCU\\Control Panel\\Desktop — qualidade de wallpaper JPEG",
        OriginalValue {
            path: format!("HKEY_CURRENT_USER\\{}", WALLPAPER_REG_PATH),
            key: WALLPAPER_REG_KEY.to_string(),
            value: original_json,
            value_type: "DWORD".to_string(),
        },
        json!(WALLPAPER_QUALITY_MAX),
    )?;

    write_dword(
        Hive::CurrentUser,
        WALLPAPER_REG_PATH,
        WALLPAPER_REG_KEY,
        WALLPAPER_QUALITY_MAX,
    )
}

/// Reverte a compressão de wallpaper para o estado original salvo no backup.
#[tauri::command]
pub fn revert_wallpaper_compression() -> Result<(), String> {
    let original = restore_from_backup("disable_wallpaper_compression")?;

    match original.value {
        None => {
            if key_exists(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)? {
                delete_value(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            let v = n.as_u64().unwrap_or(85) as u32;
            write_dword(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY, v)?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_wallpaper_compression': {:?}",
                other
            ));
        }
    }

    Ok(())
}

/// Remove a chave `JPEGImportQuality`, restaurando a compressão padrão do Windows (85%).
#[tauri::command]
pub fn restore_wallpaper_default() -> Result<(), String> {
    if key_exists(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)? {
        delete_value(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Sticky Keys (Teclas de Aderência)
// ═══════════════════════════════════════════════════════════════════════════════

const STICKY_KEYS_PATH: &str = r"Control Panel\Accessibility\StickyKeys";
const STICKY_KEYS_FLAGS_KEY: &str = "Flags";
const STICKY_KEYS_APPLIED_FLAGS: &str = "506";

static STICKY_KEYS_META: TweakMeta = TweakMeta {
    id: "disable_sticky_keys",
    name: "Desabilitar Teclas de Aderência (Sticky Keys)",
    description: "Desabilita o atalho de ativação do Sticky Keys (5x Shift), prevenindo \
        interrupções acidentais durante sessões de jogo.",
    category: "visual",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: atalho de Sticky Keys habilitado (Flags = 510)",
    hardware_filter: None,
};

/// Verifica se o atalho de Sticky Keys está desabilitado (Flags = "506").
fn get_sticky_keys_applied() -> Result<bool, String> {
    let flags = read_string(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
    Ok(flags.as_deref() == Some(STICKY_KEYS_APPLIED_FLAGS))
}

/// Retorna informações do tweak Sticky Keys com estado atual.
#[tauri::command]
pub async fn get_sticky_keys_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_sticky_keys_applied().unwrap_or(false);
        Ok(STICKY_KEYS_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Define `Flags = "506"` para desabilitar o atalho de ativação por 5x Shift.
#[tauri::command]
pub fn disable_sticky_keys() -> Result<(), String> {
    if get_sticky_keys_applied()? {
        return Err("Tweak 'disable_sticky_keys' já está aplicado (Flags = 506)".to_string());
    }

    let original_flags = read_string(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
    let original_json = original_flags.as_ref().map(|v| json!(v));

    backup_before_apply(
        "disable_sticky_keys",
        TweakCategory::Registry,
        "Flags em HKCU\\Control Panel\\Accessibility\\StickyKeys — controla atalho 5x Shift",
        OriginalValue {
            path: format!("HKEY_CURRENT_USER\\{}", STICKY_KEYS_PATH),
            key: STICKY_KEYS_FLAGS_KEY.to_string(),
            value: original_json,
            value_type: "STRING".to_string(),
        },
        json!(STICKY_KEYS_APPLIED_FLAGS),
    )?;

    write_string(
        Hive::CurrentUser,
        STICKY_KEYS_PATH,
        STICKY_KEYS_FLAGS_KEY,
        STICKY_KEYS_APPLIED_FLAGS,
    )
}

/// Reverte as Flags do Sticky Keys para o valor original salvo no backup.
#[tauri::command]
pub fn revert_sticky_keys() -> Result<(), String> {
    let original = restore_from_backup("disable_sticky_keys")?;

    match original.value {
        None => {
            if key_exists(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)? {
                delete_value(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
            }
        }
        Some(Value::String(s)) => {
            write_string(
                Hive::CurrentUser,
                STICKY_KEYS_PATH,
                STICKY_KEYS_FLAGS_KEY,
                &s,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_sticky_keys': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Busca Bing no Menu Iniciar
// ═══════════════════════════════════════════════════════════════════════════════

const BING_SEARCH_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
const BING_SEARCH_KEY: &str = "BingSearchEnabled";

static BING_SEARCH_META: TweakMeta = TweakMeta {
    id: "disable_bing_search",
    name: "Desabilitar Busca Bing no Menu Iniciar",
    description: "Remove a integração do Bing no menu Iniciar do Windows. Buscas ficam \
        apenas locais, mais rápidas e sem envio de dados para a Microsoft.",
    category: "visual",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: busca Bing habilitada no Menu Iniciar (chave ausente ou = 1)",
    hardware_filter: None,
};

/// Verifica se o Bing está desabilitado no Menu Iniciar (BingSearchEnabled = 0).
fn get_bing_search_applied() -> Result<bool, String> {
    let val = read_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)?;
    Ok(val == Some(0))
}

/// Retorna informações do tweak Bing Search com estado atual.
#[tauri::command]
pub async fn get_bing_search_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_bing_search_applied().unwrap_or(false);
        Ok(BING_SEARCH_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Define `BingSearchEnabled = 0` para remover integração Bing.
#[tauri::command]
pub fn disable_bing_search() -> Result<(), String> {
    if get_bing_search_applied()? {
        return Err(
            "Tweak 'disable_bing_search' já está aplicado (BingSearchEnabled = 0)".to_string(),
        );
    }

    let original_val = read_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)?;
    let original_json = original_val.map(|v| json!(v));

    backup_before_apply(
        "disable_bing_search",
        TweakCategory::Registry,
        "BingSearchEnabled em HKCU\\...\\Search — controla integração Bing no Menu Iniciar",
        OriginalValue {
            path: format!("HKEY_CURRENT_USER\\{}", BING_SEARCH_PATH),
            key: BING_SEARCH_KEY.to_string(),
            value: original_json,
            value_type: "DWORD".to_string(),
        },
        json!(0u32),
    )?;

    write_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY, 0)
}

/// Reverte o Bing no Menu Iniciar para o estado original salvo no backup.
#[tauri::command]
pub fn revert_bing_search() -> Result<(), String> {
    let original = restore_from_backup("disable_bing_search")?;

    match original.value {
        None => {
            if key_exists(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)? {
                delete_value(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            let v = n.as_u64().unwrap_or(1) as u32;
            write_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY, v)?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_bing_search': {:?}",
                other
            ));
        }
    }

    Ok(())
}
