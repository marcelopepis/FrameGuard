//! Tweaks visuais e UX: Wallpaper Compression, Sticky Keys, Bing Search, Classic Right Click.

use serde_json::{json, Value};

use crate::commands::optimizations::{backup_info, EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::registry::{
    delete_subkey_all, delete_value, key_exists, read_dword, read_string, subkey_exists,
    write_dword, write_string, Hive,
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

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Classic Right Click Menu (Windows 11)
//
// HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32
//   Default value = "" (string vazia) → restaura menu completo do Win10
// Somente Windows 11 (build ≥ 22000)
// ═══════════════════════════════════════════════════════════════════════════════

/// CLSID pai que ativa o menu clássico quando InprocServer32 existe com valor default vazio
const CLASSIC_CTX_CLSID_PATH: &str =
    r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}";
const CLASSIC_CTX_INPROC_PATH: &str =
    r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32";

/// Verifica o CurrentBuildNumber para determinar se estamos em Windows 11.
fn is_windows_11() -> bool {
    let build_str: String = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|k| k.get_value("CurrentBuildNumber"))
        .unwrap_or_default();
    let build: u32 = build_str.trim().parse().unwrap_or(0);
    build >= 22000
}

/// Verifica se o tweak está aplicado (a subchave InprocServer32 existe).
fn get_classic_right_click_is_applied() -> Result<bool, String> {
    subkey_exists(Hive::CurrentUser, CLASSIC_CTX_INPROC_PATH)
}

#[tauri::command]
pub async fn get_classic_right_click_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_classic_right_click_is_applied().unwrap_or(false);
        let (has_backup, last_applied) = backup_info("classic_right_click");

        Ok(TweakInfo {
            id: "classic_right_click".to_string(),
            name: "Menu de Contexto Clássico (Win11)".to_string(),
            description: "Restaura o menu de contexto completo do Windows 10 no Windows 11. \
                Por padrão, o Win11 exibe um menu reduzido com \"Mostrar mais opções\" — \
                este tweak restaura o menu completo diretamente. Reinicia o Explorer \
                automaticamente ao aplicar e ao reverter."
                .to_string(),
            category: "visual".to_string(),
            is_applied,
            requires_restart: false,
            last_applied,
            has_backup,
            risk_level: RiskLevel::Low,
            evidence_level: EvidenceLevel::Proven,
            default_value_description:
                "Padrão Windows 11: menu de contexto moderno com \"Mostrar mais opções\""
                    .to_string(),
            hardware_filter: None,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reinicia o Explorer (taskkill + spawn) para aplicar a mudança imediatamente.
fn restart_explorer() {
    std::process::Command::new("taskkill")
        .args(["/F", "/IM", "explorer.exe"])
        .output()
        .ok();
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::process::Command::new("explorer.exe").spawn().ok();
}

/// Aplica o menu de contexto clássico criando a chave InprocServer32 com valor default vazio.
#[tauri::command]
pub fn apply_classic_right_click() -> Result<(), String> {
    if !is_windows_11() {
        return Err("Este tweak é exclusivo para Windows 11 (build ≥ 22000)".to_string());
    }

    if get_classic_right_click_is_applied()? {
        return Err("Tweak 'classic_right_click' já está aplicado".to_string());
    }

    // O backup registra se a chave existia (sempre false neste caso, pois já checamos)
    backup_before_apply(
        "classic_right_click",
        TweakCategory::Registry,
        "Classic Right Click: CLSID InprocServer32 override",
        OriginalValue {
            path: format!("HKEY_CURRENT_USER\\{}", CLASSIC_CTX_CLSID_PATH),
            key: "InprocServer32".to_string(),
            value: None, // Chave não existia antes
            value_type: "SUBKEY".to_string(),
        },
        json!("created"),
    )?;

    // Criar a subchave InprocServer32 e definir o valor padrão (Default) como string vazia
    write_string(Hive::CurrentUser, CLASSIC_CTX_INPROC_PATH, "", "")?;

    restart_explorer();

    Ok(())
}

/// Reverte o menu de contexto para o moderno do Win11 deletando a chave CLSID inteira.
#[tauri::command]
pub fn revert_classic_right_click() -> Result<(), String> {
    let _original = restore_from_backup("classic_right_click")?;

    // Deletar a chave CLSID inteira (e suas subchaves)
    delete_subkey_all(Hive::CurrentUser, CLASSIC_CTX_CLSID_PATH)?;

    restart_explorer();

    Ok(())
}
