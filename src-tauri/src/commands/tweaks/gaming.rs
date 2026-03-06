//! Tweaks de gaming: Game DVR, Xbox Overlay, Game Mode, VBS,
//! Timer Resolution, Mouse Acceleration, Fullscreen Optimizations.
//!
//! Cada tweak segue o padrão `TweakMeta` + `build()` para separar metadados
//! estáticos do estado dinâmico do sistema.

use serde_json::{json, Value};

use crate::commands::optimizations::{restore_multi_entries, EvidenceLevel, RiskLevel, TweakInfo};
use crate::utils::backup::{
    backup_before_apply, restore_from_backup, OriginalValue, TweakCategory,
};
use crate::utils::registry::{
    delete_value, key_exists, read_dword, read_string, write_dword, write_string, Hive,
};
use crate::utils::tweak_builder::TweakMeta;

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Windows Game Mode
//
// HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled
//   1 = habilitado (padrão)
//   0 = desabilitado
// ═══════════════════════════════════════════════════════════════════════════════

const GAME_MODE_REG_PATH: &str = r"Software\Microsoft\GameBar";
const GAME_MODE_REG_KEY: &str = "AutoGameModeEnabled";
const GAME_MODE_ENABLED: u32 = 1;
const GAME_MODE_DISABLED: u32 = 0;

static GAME_MODE_META: TweakMeta = TweakMeta {
    id: "enable_game_mode",
    name: "Windows Game Mode",
    description: "Prioriza recursos de CPU e GPU para o jogo em execução, reduzindo a \
        interferência de processos em segundo plano como atualizações do Windows. \
        Recomendado para melhor desempenho em jogos.",
    category: "gamer",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Unproven,
    default_value_description: "Padrão Windows: Game Mode ativo (AutoGameModeEnabled = 1)",
    hardware_filter: None,
};

/// Retorna informações do tweak Game Mode com estado atual do registro.
///
/// Lê `AutoGameModeEnabled` em `HKCU\Software\Microsoft\GameBar`.
/// Ausência da chave assume Game Mode ativo (padrão desde Windows 10 Creators Update).
#[tauri::command]
pub async fn get_game_mode_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_enabled = read_dword(Hive::CurrentUser, GAME_MODE_REG_PATH, GAME_MODE_REG_KEY)?
            .map(|v| v == GAME_MODE_ENABLED)
            .unwrap_or(true);

        Ok(GAME_MODE_META.build(is_enabled))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Habilita Game Mode definindo `AutoGameModeEnabled = 1` no registro.
#[tauri::command]
pub fn enable_game_mode() -> Result<(), String> {
    write_dword(
        Hive::CurrentUser,
        GAME_MODE_REG_PATH,
        GAME_MODE_REG_KEY,
        GAME_MODE_ENABLED,
    )
}

/// Desabilita Game Mode definindo `AutoGameModeEnabled = 0` no registro.
#[tauri::command]
pub fn disable_game_mode() -> Result<(), String> {
    write_dword(
        Hive::CurrentUser,
        GAME_MODE_REG_PATH,
        GAME_MODE_REG_KEY,
        GAME_MODE_DISABLED,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — VBS (Virtualization Based Security)
//
// HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\EnableVirtualizationBasedSecurity
//   1 = habilitado
//   0 = desabilitado
// ═══════════════════════════════════════════════════════════════════════════════

const VBS_REG_PATH: &str = r"SYSTEM\CurrentControlSet\Control\DeviceGuard";
const VBS_REG_KEY: &str = "EnableVirtualizationBasedSecurity";
const VBS_ENABLED: u32 = 1;
const VBS_DISABLED: u32 = 0;

static VBS_META: TweakMeta = TweakMeta {
    id: "disable_vbs",
    name: "Virtualização Baseada em Segurança (VBS)",
    description: "A VBS usa virtualização de hardware para isolar partes críticas do Windows, \
        mas pode reduzir o desempenho em jogos em até 10–15%. Desabilitar melhora FPS, \
        especialmente em CPUs sem hardware de virtualização otimizado.",
    category: "gamer",
    requires_restart: true,
    risk_level: RiskLevel::Medium,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows 11: VBS ativa (EnableVirtualizationBasedSecurity = 1)",
    hardware_filter: None,
};

/// Retorna informações do tweak VBS com estado atual do registro.
///
/// `is_applied = true` indica que a VBS está **desabilitada** — estado recomendado
/// para maximizar performance em jogos.
#[tauri::command]
pub async fn get_vbs_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let vbs_enabled = read_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY)?
            .map(|v| v == VBS_ENABLED)
            .unwrap_or(false);

        Ok(VBS_META.build(!vbs_enabled)) // tweak "aplicado" = VBS desabilitada
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita VBS definindo `EnableVirtualizationBasedSecurity = 0`.
/// Requer reinicialização para ter efeito.
#[tauri::command]
pub fn disable_vbs() -> Result<(), String> {
    write_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY, VBS_DISABLED)
}

/// Reabilita VBS definindo `EnableVirtualizationBasedSecurity = 1`.
/// Requer reinicialização para ter efeito.
#[tauri::command]
pub fn enable_vbs() -> Result<(), String> {
    write_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY, VBS_ENABLED)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Game DVR / Background Recording
//
// Quatro chaves de registro controlam o Game DVR:
//   1. HKCU\System\GameConfigStore -> GameDVR_Enabled (master switch)
//   2. HKLM\SOFTWARE\Policies\Microsoft\Windows\GameDVR -> AllowGameDVR (policy)
//   3. HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR -> AppCaptureEnabled
//   4. HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR -> HistoricalCaptureEnabled
//
// Ao aplicar: seta GameDVR_Enabled=0, AppCaptureEnabled=0, HistoricalCaptureEnabled=0.
// Ao reverter: seta GameDVR_Enabled=1, AppCaptureEnabled=1, HistoricalCaptureEnabled=0
// (manter background OFF — é o default seguro do Windows).
// ═══════════════════════════════════════════════════════════════════════════════

const GAME_DVR_PATH_GAMECONFIG: &str = r"System\GameConfigStore";
const GAME_DVR_KEY_ENABLED: &str = "GameDVR_Enabled";
const GAME_DVR_PATH_APPCAP: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR";
const GAME_DVR_KEY_APPCAP: &str = "AppCaptureEnabled";
const GAME_DVR_KEY_HISTORICAL: &str = "HistoricalCaptureEnabled";

static GAME_DVR_META: TweakMeta = TweakMeta {
    id: "disable_game_dvr",
    name: "Desabilitar Game DVR / Gravação em Segundo Plano",
    description: "Desabilita completamente o Game DVR (master switch, captura e gravação em \
        background). A gravação em segundo plano consome GPU (encoder) e CPU mesmo quando \
        você não está gravando. Ao reverter, restaura a feature sem gravação em background.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description: "Padrão Windows: Game DVR habilitado (GameDVR_Enabled = 1)",
    hardware_filter: None,
};

/// Verifica se o Game DVR está completamente desabilitado (3 chaves zeradas).
fn get_game_dvr_is_applied() -> Result<bool, String> {
    let dvr_enabled = read_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_GAMECONFIG,
        GAME_DVR_KEY_ENABLED,
    )?
    .unwrap_or(1);
    let app_capture =
        read_dword(Hive::CurrentUser, GAME_DVR_PATH_APPCAP, GAME_DVR_KEY_APPCAP)?.unwrap_or(1);
    let historical = read_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_HISTORICAL,
    )?
    .unwrap_or(0);
    Ok(dvr_enabled == 0 && app_capture == 0 && historical == 0)
}

/// Retorna informações do tweak Game DVR com estado atual.
///
/// Três estados possíveis:
/// - `disabled`: DVR desabilitado por política ou `GameDVR_Enabled=0`
/// - `available`: DVR ativo mas sem gravação em background (`HistoricalCaptureEnabled=0`)
/// - `recording`: DVR ativo COM gravação em background (`HistoricalCaptureEnabled=1`)
///
/// Somente `recording` impacta performance de forma mensurável.
#[tauri::command]
pub async fn get_game_dvr_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_game_dvr_is_applied()?;
        Ok(GAME_DVR_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o Game DVR zerando as chaves de registro que o controlam.
///
/// Persiste backup multi-chave em `backups.json` antes de qualquer modificação.
#[tauri::command]
pub fn disable_game_dvr() -> Result<(), String> {
    if get_game_dvr_is_applied()? {
        return Err("Tweak 'disable_game_dvr' já está aplicado".to_string());
    }

    let orig_1 = read_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_GAMECONFIG,
        GAME_DVR_KEY_ENABLED,
    )?;
    let orig_2 = read_dword(Hive::CurrentUser, GAME_DVR_PATH_APPCAP, GAME_DVR_KEY_APPCAP)?;
    let orig_3 = read_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_HISTORICAL,
    )?;

    let v1 = orig_1.map(|v| json!(v)).unwrap_or(Value::Null);
    let v2 = orig_2.map(|v| json!(v)).unwrap_or(Value::Null);
    let v3 = orig_3.map(|v| json!(v)).unwrap_or(Value::Null);

    backup_before_apply(
        "disable_game_dvr",
        TweakCategory::Registry,
        "Game DVR — GameDVR_Enabled + AppCaptureEnabled + HistoricalCaptureEnabled",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "game_dvr_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKCU",
                    "path": GAME_DVR_PATH_GAMECONFIG,
                    "key": GAME_DVR_KEY_ENABLED,
                    "value": v1
                },
                {
                    "hive": "HKCU",
                    "path": GAME_DVR_PATH_APPCAP,
                    "key": GAME_DVR_KEY_APPCAP,
                    "value": v2
                },
                {
                    "hive": "HKCU",
                    "path": GAME_DVR_PATH_APPCAP,
                    "key": GAME_DVR_KEY_HISTORICAL,
                    "value": v3
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([0, 0, 0]),
    )?;

    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_GAMECONFIG,
        GAME_DVR_KEY_ENABLED,
        0,
    )?;
    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_APPCAP,
        0,
    )?;
    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_HISTORICAL,
        0,
    )?;
    Ok(())
}

/// Reverte o Game DVR para estado funcional mas seguro:
/// `GameDVR_Enabled=1`, `AppCaptureEnabled=1`, `HistoricalCaptureEnabled=0`.
/// Mantém gravação em background desligada (é o default seguro do Windows).
#[tauri::command]
pub fn revert_game_dvr() -> Result<(), String> {
    let _original = restore_from_backup("disable_game_dvr")?;

    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_GAMECONFIG,
        GAME_DVR_KEY_ENABLED,
        1,
    )?;
    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_APPCAP,
        1,
    )?;
    write_dword(
        Hive::CurrentUser,
        GAME_DVR_PATH_APPCAP,
        GAME_DVR_KEY_HISTORICAL,
        0,
    )?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Xbox Game Bar Overlay
//
// Duas chaves em HKCU\SOFTWARE\Microsoft\GameBar controlam o overlay:
//   1. UseNexusForGameBarEnabled = 0
//   2. ShowStartupPanel = 0
//
// Remove o processo GameBarPresenceWriter.exe e impede ativação acidental com Win+G.
// ═══════════════════════════════════════════════════════════════════════════════

const XBOX_OVERLAY_PATH: &str = r"SOFTWARE\Microsoft\GameBar";
const XBOX_OVERLAY_KEY_NEXUS: &str = "UseNexusForGameBarEnabled";
const XBOX_OVERLAY_KEY_PANEL: &str = "ShowStartupPanel";

static XBOX_OVERLAY_META: TweakMeta = TweakMeta {
    id: "disable_xbox_overlay",
    name: "Desabilitar Xbox Game Bar Overlay",
    description: "Remove o overlay da Xbox Game Bar que pode ser ativado acidentalmente \
        durante jogos (Win+G). Impacto em recursos é mínimo, mas elimina o processo \
        GameBarPresenceWriter.exe.",
    category: "gpu_display",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: Xbox Game Bar habilitado (UseNexusForGameBarEnabled = 1)",
    hardware_filter: None,
};

/// Verifica se o Xbox Overlay está desabilitado (ambas as chaves zeradas).
fn get_xbox_overlay_is_applied() -> Result<bool, String> {
    let v1 = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_NEXUS)?.unwrap_or(1);
    let v2 = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_PANEL)?.unwrap_or(1);
    Ok(v1 == 0 && v2 == 0)
}

/// Retorna informações do tweak Xbox Overlay com estado atual do registro.
#[tauri::command]
pub async fn get_xbox_overlay_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_xbox_overlay_is_applied()?;
        Ok(XBOX_OVERLAY_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita o Xbox Game Bar zerando as duas chaves de controle do overlay.
#[tauri::command]
pub fn disable_xbox_overlay() -> Result<(), String> {
    if get_xbox_overlay_is_applied()? {
        return Err("Tweak 'disable_xbox_overlay' já está aplicado".to_string());
    }

    let orig_nexus = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_NEXUS)?;
    let orig_panel = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_PANEL)?;

    let vn = orig_nexus.map(|v| json!(v)).unwrap_or(Value::Null);
    let vp = orig_panel.map(|v| json!(v)).unwrap_or(Value::Null);

    backup_before_apply(
        "disable_xbox_overlay",
        TweakCategory::Registry,
        "Xbox Game Bar — UseNexusForGameBarEnabled + ShowStartupPanel",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "xbox_overlay_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKCU",
                    "path": XBOX_OVERLAY_PATH,
                    "key": XBOX_OVERLAY_KEY_NEXUS,
                    "value": vn
                },
                {
                    "hive": "HKCU",
                    "path": XBOX_OVERLAY_PATH,
                    "key": XBOX_OVERLAY_KEY_PANEL,
                    "value": vp
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([0, 0]),
    )?;

    write_dword(
        Hive::CurrentUser,
        XBOX_OVERLAY_PATH,
        XBOX_OVERLAY_KEY_NEXUS,
        0,
    )?;
    write_dword(
        Hive::CurrentUser,
        XBOX_OVERLAY_PATH,
        XBOX_OVERLAY_KEY_PANEL,
        0,
    )?;
    Ok(())
}

/// Restaura as configurações do Xbox Game Bar para os valores originais.
#[tauri::command]
pub fn revert_xbox_overlay() -> Result<(), String> {
    let original = restore_from_backup("disable_xbox_overlay")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_xbox_overlay' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_xbox_overlay'")?;
    restore_multi_entries(arr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Timer de Alta Resolução (GlobalTimerResolutionRequests)
//
// HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\kernel
//   -> GlobalTimerResolutionRequests = 1 (DWORD)
//
// Permite que aplicações solicitem timer resolution global de 1ms em vez do
// padrão 15,6ms. Específico do Windows 11. Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

const TIMER_RES_PATH: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel";
const TIMER_RES_KEY: &str = "GlobalTimerResolutionRequests";

static TIMER_RES_META: TweakMeta = TweakMeta {
    id: "enable_timer_resolution",
    name: "Timer de Alta Resolução (GlobalTimerResolutionRequests)",
    description: "Permite que aplicações solicitem timer resolution global de 1ms em vez \
        do padrão 15,6ms. Melhora frame pacing e reduz input lag, especialmente em \
        monitores 144Hz+. Específico do Windows 11.",
    category: "gaming",
    requires_restart: true,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: GlobalTimerResolutionRequests ausente (timer 15,6ms)",
    hardware_filter: None,
};

/// Verifica se o timer de alta resolução está habilitado.
fn get_timer_resolution_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY)?.unwrap_or(0);
    Ok(val == 1)
}

/// Retorna informações do tweak Timer Resolution com estado atual do registro.
///
/// Lê `GlobalTimerResolutionRequests` em `HKLM\SYSTEM\...\kernel`.
#[tauri::command]
pub async fn get_timer_resolution_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_timer_resolution_is_applied()?;
        Ok(TIMER_RES_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Habilita requisições globais de timer de alta resolução no kernel do Windows 11.
#[tauri::command]
pub fn enable_timer_resolution() -> Result<(), String> {
    if get_timer_resolution_is_applied()? {
        return Err("Tweak 'enable_timer_resolution' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY)?;

    backup_before_apply(
        "enable_timer_resolution",
        TweakCategory::Registry,
        "Timer Resolution — GlobalTimerResolutionRequests no kernel session manager",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", TIMER_RES_PATH),
            key: TIMER_RES_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(1),
    )?;

    write_dword(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY, 1)
}

/// Reverte o timer resolution para o estado original (remove a chave ou restaura o valor).
#[tauri::command]
pub fn disable_timer_resolution() -> Result<(), String> {
    let original = restore_from_backup("enable_timer_resolution")?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY)? {
                delete_value(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                TIMER_RES_PATH,
                TIMER_RES_KEY,
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'enable_timer_resolution': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Aceleração do Mouse
//
// Três chaves REG_SZ em HKCU\Control Panel\Mouse:
//   MouseSpeed = "0"  (padrão Windows: "1")
//   MouseThreshold1 = "0"  (padrão Windows: "6")
//   MouseThreshold2 = "0"  (padrão Windows: "10")
//
// Remove a curva Enhanced Pointer Precision → movimento 1:1 com o mouse físico.
// ═══════════════════════════════════════════════════════════════════════════════

const MOUSE_ACC_PATH: &str = r"Control Panel\Mouse";
const MOUSE_SPEED_KEY: &str = "MouseSpeed";
const MOUSE_THRESHOLD1_KEY: &str = "MouseThreshold1";
const MOUSE_THRESHOLD2_KEY: &str = "MouseThreshold2";

static MOUSE_ACC_META: TweakMeta = TweakMeta {
    id: "disable_mouse_acceleration",
    name: "Desabilitar Aceleração do Mouse",
    description: "Remove a curva não-linear de resposta do mouse do Windows. Essencial \
        para mira consistente em jogos FPS. O movimento do cursor passa a ser 1:1 com \
        o movimento físico do mouse.",
    category: "gaming",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Proven,
    default_value_description:
        "Padrão Windows: MouseSpeed = \"1\", Threshold1 = \"6\", Threshold2 = \"10\"",
    hardware_filter: None,
};

/// Verifica se a aceleração do mouse está desabilitada (3 chaves em "0").
fn get_mouse_acc_is_applied() -> Result<bool, String> {
    let speed =
        read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_SPEED_KEY)?.unwrap_or_default();
    let thr1 =
        read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD1_KEY)?.unwrap_or_default();
    let thr2 =
        read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD2_KEY)?.unwrap_or_default();
    Ok(speed == "0" && thr1 == "0" && thr2 == "0")
}

/// Retorna informações do tweak Aceleração do Mouse com estado atual.
///
/// Lê `MouseSpeed`, `MouseThreshold1`, `MouseThreshold2` em `HKCU\Control Panel\Mouse`.
#[tauri::command]
pub async fn get_mouse_acceleration_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_mouse_acc_is_applied()?;
        Ok(MOUSE_ACC_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita a aceleração do mouse zerando as três chaves REG_SZ.
///
/// Os valores originais são preservados no backup como strings JSON para
/// restauração exata — incluindo valores não-padrão configurados pelo usuário.
#[tauri::command]
pub fn disable_mouse_acceleration() -> Result<(), String> {
    if get_mouse_acc_is_applied()? {
        return Err("Tweak 'disable_mouse_acceleration' já está aplicado".to_string());
    }

    let orig_speed = read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_SPEED_KEY)?;
    let orig_thr1 = read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD1_KEY)?;
    let orig_thr2 = read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD2_KEY)?;

    let vs = orig_speed.map(|v| json!(v)).unwrap_or(Value::Null);
    let vt1 = orig_thr1.map(|v| json!(v)).unwrap_or(Value::Null);
    let vt2 = orig_thr2.map(|v| json!(v)).unwrap_or(Value::Null);

    backup_before_apply(
        "disable_mouse_acceleration",
        TweakCategory::Registry,
        "Aceleração do Mouse — MouseSpeed + MouseThreshold1 + MouseThreshold2 (REG_SZ)",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "mouse_acceleration_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKCU",
                    "path": MOUSE_ACC_PATH,
                    "key": MOUSE_SPEED_KEY,
                    "value": vs
                },
                {
                    "hive": "HKCU",
                    "path": MOUSE_ACC_PATH,
                    "key": MOUSE_THRESHOLD1_KEY,
                    "value": vt1
                },
                {
                    "hive": "HKCU",
                    "path": MOUSE_ACC_PATH,
                    "key": MOUSE_THRESHOLD2_KEY,
                    "value": vt2
                }
            ])),
            value_type: "MULTI_STRING".to_string(),
        },
        json!(["0", "0", "0"]),
    )?;

    write_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_SPEED_KEY, "0")?;
    write_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD1_KEY, "0")?;
    write_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD2_KEY, "0")?;
    Ok(())
}

/// Restaura os valores originais de MouseSpeed e MouseThreshold (REG_SZ) a partir do backup.
#[tauri::command]
pub fn revert_mouse_acceleration() -> Result<(), String> {
    let original = restore_from_backup("disable_mouse_acceleration")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_mouse_acceleration' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_mouse_acceleration'")?;
    restore_multi_entries(arr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Fullscreen Optimizations (global)
//
// Cinco chaves DWORD em HKCU\System\GameConfigStore:
//   GameDVR_FSEBehaviorMode = 2
//   GameDVR_HonorUserFSEBehaviorMode = 1
//   GameDVR_FSEBehavior = 2
//   GameDVR_DXGIHonorFSEWindowsCompatible = 1
//   GameDVR_EFSEFeatureFlags = 0
//
// Benefício real principalmente em jogos DX9/DX11. No Windows 11 com jogos
// DX12/Vulkan modernos, o impacto é negligível ou inexistente.
// ═══════════════════════════════════════════════════════════════════════════════

const FSO_KEY_FSE_BEHAVIOR_MODE: &str = "GameDVR_FSEBehaviorMode";
const FSO_KEY_HONOR_USER_FSE: &str = "GameDVR_HonorUserFSEBehaviorMode";
const FSO_KEY_FSE_BEHAVIOR: &str = "GameDVR_FSEBehavior";
const FSO_KEY_DXGI_HONOR: &str = "GameDVR_DXGIHonorFSEWindowsCompatible";
const FSO_KEY_EFSE_FLAGS: &str = "GameDVR_EFSEFeatureFlags";

/// Pares (chave, valor-alvo) para verificação e aplicação do tweak de FSO.
/// Usa unwrap_or(99) no check — 99 nunca coincide com os valores alvo (0, 1, 2).
const FSO_TARGET: [(&str, u32); 5] = [
    (FSO_KEY_FSE_BEHAVIOR_MODE, 2),
    (FSO_KEY_HONOR_USER_FSE, 1),
    (FSO_KEY_FSE_BEHAVIOR, 2),
    (FSO_KEY_DXGI_HONOR, 1),
    (FSO_KEY_EFSE_FLAGS, 0),
];

static FSO_META: TweakMeta = TweakMeta {
    id: "disable_fullscreen_optimizations",
    name: "Desabilitar Fullscreen Optimizations (global)",
    description: "Força jogos a usar fullscreen exclusivo em vez do modo otimizado do \
        Windows. Era relevante no Windows 10, mas no Windows 11 o sistema de FSO foi \
        significativamente melhorado. Pode beneficiar jogos DX9/DX11 mais antigos. Para \
        jogos DX12/Vulkan modernos, o impacto é negligível ou inexistente.",
    category: "gaming",
    requires_restart: false,
    risk_level: RiskLevel::Low,
    evidence_level: EvidenceLevel::Unproven,
    default_value_description:
        "Padrão Windows: Fullscreen Optimizations habilitado (chaves FSE ausentes)",
    hardware_filter: None,
};

/// Verifica se todas as 5 chaves FSO estão nos valores-alvo.
fn get_fso_is_applied() -> Result<bool, String> {
    for (key, target) in &FSO_TARGET {
        let val = read_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Retorna informações do tweak Fullscreen Optimizations com estado atual.
///
/// Lê 5 chaves `GameDVR_FSE*` em `HKCU\System\GameConfigStore`.
#[tauri::command]
pub async fn get_fullscreen_optimizations_info() -> Result<TweakInfo, String> {
    tokio::task::spawn_blocking(|| {
        let is_applied = get_fso_is_applied()?;
        Ok(FSO_META.build(is_applied))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Aplica as 5 chaves FSO de uma vez, preservando cada valor original no backup.
#[tauri::command]
pub fn disable_fullscreen_optimizations() -> Result<(), String> {
    if get_fso_is_applied()? {
        return Err("Tweak 'disable_fullscreen_optimizations' já está aplicado".to_string());
    }

    let orig_vals: Vec<Value> = FSO_TARGET
        .iter()
        .map(|(key, _)| {
            read_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, key)
                .map(|opt| opt.map(|v| json!(v)).unwrap_or(Value::Null))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let backup_entries: Vec<Value> = FSO_TARGET
        .iter()
        .zip(orig_vals.iter())
        .map(|((key, _), orig)| {
            json!({
                "hive": "HKCU",
                "path": GAME_DVR_PATH_GAMECONFIG,
                "key": key,
                "value": orig
            })
        })
        .collect();

    let applied_vals: Vec<Value> = FSO_TARGET.iter().map(|(_, v)| json!(v)).collect();

    backup_before_apply(
        "disable_fullscreen_optimizations",
        TweakCategory::Registry,
        "Fullscreen Optimizations — 5 chaves GameDVR_FSE em GameConfigStore",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "fso_keys".to_string(),
            value: Some(Value::Array(backup_entries)),
            value_type: "MULTI_DWORD".to_string(),
        },
        Value::Array(applied_vals),
    )?;

    for (key, target) in &FSO_TARGET {
        write_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, key, *target)?;
    }
    Ok(())
}

/// Restaura os valores originais das 5 chaves de Fullscreen Optimizations.
#[tauri::command]
pub fn revert_fullscreen_optimizations() -> Result<(), String> {
    let original = restore_from_backup("disable_fullscreen_optimizations")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_fullscreen_optimizations' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_fullscreen_optimizations'")?;
    restore_multi_entries(arr)
}
