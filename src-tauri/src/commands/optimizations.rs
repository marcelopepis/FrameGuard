//! Comandos Tauri para tweaks de otimização do FrameGuard.
//!
//! Cada tweak segue o fluxo padrão:
//!   1. Verificar estado atual no sistema (comando `get_X_status`)
//!   2. Salvar backup do valor original via `utils::backup` antes de qualquer mudança
//!   3. Aplicar a modificação (comando `disable_X`)
//!   4. Retornar resultado com status de sucesso/falha
//!
//! Reversão é feita pelo comando `revert_X` / `enable_X`, que restaura o valor
//! original a partir do backup e marca a entrada como `Reverted`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::utils::backup::{
    backup_before_apply, get_all_backups, restore_from_backup, BackupStatus, OriginalValue,
    TweakCategory,
};
use crate::utils::command_runner::{run_command, run_command_with_progress, run_powershell};
use crate::utils::registry::{
    delete_value, key_exists, read_dword, read_string, write_dword, write_string, Hive,
};

// ─── Constantes de registro ───────────────────────────────────────────────────

/// Caminho no HKCU onde o Windows armazena configurações de exibição do Desktop
const WALLPAPER_REG_PATH: &str = r"Control Panel\Desktop";
/// Chave que controla a qualidade de importação de wallpapers JPEG (0-100)
const WALLPAPER_REG_KEY: &str = "JPEGImportQuality";
/// Qualidade máxima sem perdas — valor que este tweak escreve
const WALLPAPER_QUALITY_MAX: u32 = 100;

/// Caminho no HKLM onde a Otimização de Entrega guarda sua configuração
const DELIVERY_OPT_REG_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Config";
/// Chave que define o modo de download P2P do Windows Update
const DELIVERY_OPT_REG_KEY: &str = "DODownloadMode";
/// Valor que desabilita completamente o P2P (somente HTTP direto aos servidores Microsoft)
const DELIVERY_OPT_DISABLED: u32 = 0;
/// Valor padrão do Windows: P2P habilitado entre PCs da rede local e da internet
const DELIVERY_OPT_DEFAULT: u32 = 1;

// ─── Constantes — Game DVR ────────────────────────────────────────────────────

/// HKCU: chave principal que habilita/desabilita o Game DVR globalmente
const GAME_DVR_PATH_GAMECONFIG: &str = r"System\GameConfigStore";
const GAME_DVR_KEY_ENABLED: &str = "GameDVR_Enabled";
/// HKLM: política de grupo que bloqueia o Game DVR para todos os usuários
const GAME_DVR_PATH_POLICIES: &str = r"SOFTWARE\Policies\Microsoft\Windows\GameDVR";
const GAME_DVR_KEY_ALLOW: &str = "AllowGameDVR";
/// HKCU: controla a captura de tela via Game DVR
const GAME_DVR_PATH_APPCAP: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR";
const GAME_DVR_KEY_APPCAP: &str = "AppCaptureEnabled";

// ─── Constantes — Xbox Overlay ────────────────────────────────────────────────

/// Chave de registro do Xbox Game Bar no perfil do usuário
const XBOX_OVERLAY_PATH: &str = r"SOFTWARE\Microsoft\GameBar";
/// Habilita o GameBarPresenceWriter (overlay Win+G)
const XBOX_OVERLAY_KEY_NEXUS: &str = "UseNexusForGameBarEnabled";
/// Exibe painel de boas-vindas na primeira abertura
const XBOX_OVERLAY_KEY_PANEL: &str = "ShowStartupPanel";

// ─── Constantes — MPO (Multiplane Overlay) ────────────────────────────────────

/// Caminho do DWM onde a flag de teste de overlay está
const MPO_PATH: &str = r"SOFTWARE\Microsoft\Windows\Dwm";
/// Chave que define o modo de overlay; valor 5 = MPO desabilitado
const MPO_KEY: &str = "OverlayTestMode";
/// Valor que desabilita o MPO no DWM
const MPO_DISABLED_VALUE: u32 = 5;

// ─── Constantes — NVIDIA Telemetria ───────────────────────────────────────────

/// Caminho das chaves de Feature Telemetry do driver NVIDIA
const NVIDIA_FTS_PATH: &str = r"SOFTWARE\NVIDIA Corporation\Global\FTS";
const NVIDIA_FTS_KEY_1: &str = "EnableRID44231";
const NVIDIA_FTS_KEY_2: &str = "EnableRID64640";
const NVIDIA_FTS_KEY_3: &str = "EnableRID66610";
/// Caminho da preferência de opt-in no Painel de Controle NVIDIA
const NVIDIA_CP_PATH: &str = r"SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client";
const NVIDIA_CP_KEY: &str = "OptInOrOutPreference";
/// Nome do serviço Windows de telemetria do driver NVIDIA
const NVIDIA_TELEMETRY_SERVICE: &str = "NvTelemetryContainer";

// ─── Constantes — Timer Resolution ───────────────────────────────────────────

/// Caminho do kernel session manager onde a flag de timer resolution fica
const TIMER_RES_PATH: &str =
    r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel";
/// Chave que habilita requisições globais de alta resolução (1ms) — Windows 11+
const TIMER_RES_KEY: &str = "GlobalTimerResolutionRequests";

// ─── Constantes — Mouse Acceleration ─────────────────────────────────────────

/// Caminho das configurações de mouse no perfil do usuário (valores REG_SZ)
const MOUSE_ACC_PATH: &str = r"Control Panel\Mouse";
/// Multiplica velocidade do cursor — "0" = sem aceleração, "1" = aceleração habilitada
const MOUSE_SPEED_KEY: &str = "MouseSpeed";
/// Limiar 1 da aceleração; "0" desabilita o patamar
const MOUSE_THRESHOLD1_KEY: &str = "MouseThreshold1";
/// Limiar 2 da aceleração; "0" desabilita o patamar
const MOUSE_THRESHOLD2_KEY: &str = "MouseThreshold2";

// ─── Constantes — Fullscreen Optimizations ────────────────────────────────────
// Nota: reutiliza GAME_DVR_PATH_GAMECONFIG (r"System\GameConfigStore")

/// Modo de comportamento FSE — 2 = forçar fullscreen exclusivo
const FSO_KEY_FSE_BEHAVIOR_MODE: &str = "GameDVR_FSEBehaviorMode";
/// Honra preferência do usuário por FSE — 1 = habilitado
const FSO_KEY_HONOR_USER_FSE: &str = "GameDVR_HonorUserFSEBehaviorMode";
/// Comportamento FSE — 2 = fullscreen exclusivo
const FSO_KEY_FSE_BEHAVIOR: &str = "GameDVR_FSEBehavior";
/// DXGI honra janelas compatíveis com FSE — 1 = habilitado
const FSO_KEY_DXGI_HONOR: &str = "GameDVR_DXGIHonorFSEWindowsCompatible";
/// Flags EFSE — 0 = sem flags extras
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

// ─── Constantes — Ultimate Performance Power Plan ─────────────────────────────

/// GUID template do plano Ultimate Performance — não pode ser ativado diretamente
const ULTIMATE_PERF_GUID: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
/// Caminho e chave para desabilitar bloqueio Modern Standby na duplicação do plano
const MODERN_STANDBY_PATH: &str = r"SYSTEM\CurrentControlSet\Control\Power";
const MODERN_STANDBY_KEY: &str = "PlatformAoAcOverride";

// ─── Constantes — Power Throttling ────────────────────────────────────────────

/// Caminho onde o PowerThrottlingOff é configurado (criado se não existir)
const POWER_THROTTLE_PATH: &str =
    r"SYSTEM\CurrentControlSet\Control\Power\PowerThrottling";
/// Valor 1 = Power Throttling desabilitado globalmente
const POWER_THROTTLE_KEY: &str = "PowerThrottlingOff";

// ─── Tipos compartilhados ─────────────────────────────────────────────────────

/// Nível de risco associado à aplicação de um tweak.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Alteração cosmética ou de preferência; facilmente reversível sem impacto sistêmico
    Low,
    /// Modifica comportamento do sistema; reversível, mas pode exigir reinicialização
    Medium,
    /// Impacto sistêmico significativo; pode afetar estabilidade ou compatibilidade
    High,
}

/// Nível de evidência técnica/científica que sustenta o benefício de um tweak.
///
/// Permite ao usuário avaliar o grau de confiança antes de aplicar uma otimização,
/// diferenciando tweaks comprovados em benchmarks de sugestões populares sem base formal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceLevel {
    /// Benefício confirmado por benchmarks documentados e/ou documentação oficial
    Proven,
    /// Raciocínio técnico sólido e mecanismo bem compreendido, mas sem benchmarks
    /// rigorosos publicados que quantifiquem o ganho de forma reproduzível
    Plausible,
    /// Amplamente compartilhado na comunidade gamer, porém sem evidência formal
    /// — resultados variam ou são negligenciáveis em benchmarks independentes
    Unproven,
}

/// Informações completas de um tweak para exibição na UI.
///
/// Combina metadados estáticos (nome, descrição, risco) com o estado dinâmico
/// atual do sistema (is_applied, has_backup, last_applied). Enviado ao frontend
/// como resposta aos comandos `get_X_info`.
#[derive(Debug, Serialize)]
pub struct TweakInfo {
    /// Identificador único em snake_case (ex: `"disable_wallpaper_compression"`)
    pub id: String,
    /// Nome legível exibido na UI (ex: `"Desabilitar Compressão de Wallpaper"`)
    pub name: String,
    /// Descrição detalhada do efeito para o usuário final
    pub description: String,
    /// Categoria para agrupamento na UI (ex: `"optimization"`, `"cleanup"`)
    pub category: String,
    /// `true` se o tweak está atualmente ativo no sistema
    pub is_applied: bool,
    /// `true` se a mudança só tem efeito após reinicialização do Windows
    pub requires_restart: bool,
    /// Timestamp ISO 8601 UTC da última aplicação; `null` se nunca aplicado com backup
    pub last_applied: Option<String>,
    /// `true` se existe backup com status `Applied` disponível para reversão
    pub has_backup: bool,
    /// Nível de risco do tweak
    pub risk_level: RiskLevel,
    /// Grau de evidência técnica que sustenta o benefício declarado do tweak
    pub evidence_level: EvidenceLevel,
    /// Descrição do valor padrão do Windows para exibição no botão "Restaurar Padrão"
    pub default_value_description: String,
}

// ─── Tipos de status por tweak ────────────────────────────────────────────────

/// Estado atual do tweak de compressão de wallpaper.
#[derive(Debug, Serialize)]
pub struct WallpaperCompressionStatus {
    /// `true` se o tweak está ativo — qualidade configurada para 100% (sem compressão)
    pub enabled: bool,
    /// Valor atual de `JPEGImportQuality`; `0` indica chave ausente (padrão Windows = 85%)
    pub current_value: u32,
}

/// Estado atual do armazenamento reservado do Windows.
#[derive(Debug, Serialize)]
pub struct ReservedStorageStatus {
    /// `true` se o armazenamento reservado está **habilitado** no Windows
    pub enabled: bool,
    /// Saída bruta do DISM para diagnóstico e depuração
    pub raw_output: String,
}

/// Estado atual da Otimização de Entrega do Windows Update.
#[derive(Debug, Serialize)]
pub struct DeliveryOptimizationStatus {
    /// `true` se o tweak está ativo — P2P desabilitado (`DODownloadMode = 0`)
    pub enabled: bool,
    /// Valor atual de `DODownloadMode`; `1` indica chave ausente (padrão Windows)
    pub current_value: u32,
}

// ─── Utilitário interno ───────────────────────────────────────────────────────

/// Consulta `backups.json` e extrai `(has_backup, last_applied)` para um tweak.
///
/// - `has_backup`: `true` quando há entrada com status `Applied` (backup utilizável)
/// - `last_applied`: timestamp `backed_up_at` quando aplicado; `None` caso contrário
pub fn backup_info(tweak_id: &str) -> (bool, Option<String>) {
    match get_all_backups() {
        Ok(backups) => match backups.get(tweak_id) {
            Some(entry) if entry.status == BackupStatus::Applied => {
                (true, Some(entry.backed_up_at.clone()))
            }
            _ => (false, None),
        },
        Err(_) => (false, None),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 1 — Compressão de Wallpaper  (HKCU · Registro · Baixo Risco)
//
// O Windows comprime automaticamente imagens JPEG usadas como papel de parede
// para 85% de qualidade ao importá-las para o perfil do usuário. A chave
// `JPEGImportQuality` em HKCU\Control Panel\Desktop controla essa qualidade
// (intervalo aceito pelo Windows: 0–100). Definir o valor para 100 instrui o
// sistema a manter a imagem original sem qualquer perda.
//
// Cuidados:
//   - O efeito só é visível após o próximo logon (Windows reimporta o wallpaper)
//   - Ausência da chave = Windows aplica 85% internamente (padrão de fábrica)
//   - Não afeta wallpapers PNG/BMP (já sem compressão com perdas)
// ═══════════════════════════════════════════════════════════════════════════════

/// Verifica o estado atual da compressão de wallpaper lendo o registro.
///
/// Retorna `current_value = 0` quando a chave não existe, indicando que o Windows
/// aplicará seu padrão interno de 85% — semanticamente diferente de um valor
/// explicitamente configurado, mas suficiente para a lógica de UI.
#[tauri::command]
pub fn get_wallpaper_compression_status() -> Result<WallpaperCompressionStatus, String> {
    let current_value =
        read_dword(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?.unwrap_or(0);

    Ok(WallpaperCompressionStatus {
        // Tweak ativo apenas quando a qualidade está configurada no máximo (100)
        enabled: current_value == WALLPAPER_QUALITY_MAX,
        current_value,
    })
}

/// Retorna as informações completas do tweak de compressão de wallpaper.
///
/// Combina o estado atual do registro com metadados estáticos e informações
/// de backup para montar o `TweakInfo` enviado à UI.
#[tauri::command]
pub fn get_wallpaper_compression_info() -> Result<TweakInfo, String> {
    let status = get_wallpaper_compression_status()?;
    let (has_backup, last_applied) = backup_info("disable_wallpaper_compression");

    Ok(TweakInfo {
        id: "disable_wallpaper_compression".to_string(),
        name: "Desabilitar Compressão de Wallpaper".to_string(),
        description: "Desabilita a compressão automática de imagens JPEG usadas como papel de \
            parede. O Windows reduz a qualidade para 85% por padrão. Este tweak mantém a \
            qualidade original da imagem (100%)."
            .to_string(),
        category: "optimization".to_string(),
        is_applied: status.enabled,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description: "Padrão Windows: compressão JPEG em 85% (JPEGImportQuality ausente)".to_string(),
    })
}

/// Desabilita a compressão de wallpaper definindo `JPEGImportQuality = 100`.
///
/// Fluxo de execução:
/// 1. Lê estado atual — retorna erro se o tweak já estiver aplicado (idempotência)
/// 2. Lê o valor original de `JPEGImportQuality` (pode ser `None` se a chave não existir)
/// 3. Persiste backup do original em `backups.json` antes de qualquer modificação
/// 4. Escreve `JPEGImportQuality = 100` em `HKCU\Control Panel\Desktop`
///
/// O efeito visual só ocorre após reinicialização (ou novo logon do usuário).
#[tauri::command]
pub fn disable_wallpaper_compression() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação — evita sobrescrever o backup original
    let status = get_wallpaper_compression_status()?;
    if status.enabled {
        return Err(
            "Tweak 'disable_wallpaper_compression' já está aplicado (qualidade = 100)".to_string(),
        );
    }

    // Passo 2: Captura o valor original ANTES de qualquer modificação no sistema
    let original_dword = read_dword(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
    let original_json: Option<Value> = original_dword.map(|v| json!(v));

    // Passo 3: Salva backup — `value: null` indica que a chave não existia (padrão Windows 85%)
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

    // Passo 4: Aplica o tweak escrevendo o valor máximo de qualidade
    write_dword(
        Hive::CurrentUser,
        WALLPAPER_REG_PATH,
        WALLPAPER_REG_KEY,
        WALLPAPER_QUALITY_MAX,
    )
}

/// Reverte a compressão de wallpaper para o estado original salvo no backup.
///
/// Comportamento conforme o valor original:
/// - `null` (chave não existia): remove `JPEGImportQuality` → Windows volta a 85%
/// - número: restaura esse valor exato no registro
#[tauri::command]
pub fn revert_wallpaper_compression() -> Result<(), String> {
    // Recupera o original e atomicamente marca o backup como Reverted no disco
    let original = restore_from_backup("disable_wallpaper_compression")?;

    match original.value {
        // Chave não existia antes do tweak — remove para restaurar padrão Windows (85%)
        None => {
            if key_exists(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)? {
                delete_value(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
            }
        }
        // Chave existia com valor numérico — restaura o valor original exato
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

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 2 — Armazenamento Reservado  (DISM · Médio Risco)
//
// O Windows reserva aproximadamente 7 GB do disco para garantir espaço durante
// a instalação de atualizações, recursos opcionais e arquivos temporários do
// sistema. Esse espaço fica inacessível ao usuário normal. Desabilitar via DISM
// libera o espaço imediatamente, mas o usuário torna-se responsável por manter
// espaço livre suficiente quando updates forem instalados.
//
// Cuidados:
//   - Requer processo rodando como Administrador (DISM /Online)
//   - Pode impedir instalação de updates em discos muito cheios após desabilitar
//   - Não requer reinicialização — efeito imediato após DISM completar
// ═══════════════════════════════════════════════════════════════════════════════

/// Retorna o estado atual do armazenamento reservado consultando o DISM.
///
/// Executa `DISM /Online /Get-ReservedStorageState` e analisa a saída para
/// detectar se o estado é "Enabled" ou "Disabled". A saída típica do DISM é:
/// ```text
/// Reserved Storage State : Enabled
/// The operation completed successfully.
/// ```
#[tauri::command]
pub fn get_reserved_storage_status() -> Result<ReservedStorageStatus, String> {
    let output = run_command("dism.exe", &["/Online", "/Get-ReservedStorageState"])?;

    // Normaliza para lowercase — DISM pode variar maiúsculas entre versões do Windows
    let stdout_lower = output.stdout.to_lowercase();

    // "enabled" e "disabled" são mutuamente exclusivos na linha de estado do DISM.
    // A presença de "disabled" indica que o tweak já foi aplicado anteriormente.
    let enabled = stdout_lower.contains("enabled") && !stdout_lower.contains("disabled");

    Ok(ReservedStorageStatus {
        enabled,
        raw_output: output.stdout,
    })
}

/// Retorna as informações completas do tweak de armazenamento reservado.
///
/// `is_applied = true` quando o armazenamento reservado está **desabilitado**
/// (ou seja, o tweak foi aplicado e o espaço foi recuperado).
#[tauri::command]
pub fn get_reserved_storage_info() -> Result<TweakInfo, String> {
    let status = get_reserved_storage_status()?;
    let (has_backup, last_applied) = backup_info("disable_reserved_storage");

    Ok(TweakInfo {
        id: "disable_reserved_storage".to_string(),
        name: "Recuperar Armazenamento Reservado".to_string(),
        description: "Recupera o espaço de armazenamento reservado pelo Windows para \
            atualizações. O Windows reserva cerca de 7GB do disco para garantir que updates \
            possam ser instalados. Se você prefere gerenciar isso manualmente, pode desabilitar \
            e recuperar este espaço."
            .to_string(),
        category: "optimization".to_string(),
        // Tweak "aplicado" = armazenamento reservado DESABILITADO = espaço recuperado
        is_applied: !status.enabled,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: Armazenamento Reservado habilitado (~7 GB)".to_string(),
    })
}

/// Desabilita o armazenamento reservado via DISM com streaming de progresso.
///
/// Fluxo de execução:
/// 1. Verifica estado atual — rejeita se já estiver desabilitado
/// 2. Salva backup do estado atual (`"Enabled"`) para reversão futura
/// 3. Executa `DISM /Online /Set-ReservedStorageState /State:Disabled`
///    com streaming de output para o frontend via evento `"dism-reserved-storage"`
///
/// O frontend deve registrar `listen("dism-reserved-storage", handler)` para
/// acompanhar o progresso linha a linha.
#[tauri::command]
pub fn disable_reserved_storage(app_handle: tauri::AppHandle) -> Result<(), String> {
    // Passo 1: Rejeita se o armazenamento já estiver desabilitado
    let status = get_reserved_storage_status()?;
    if !status.enabled {
        return Err(
            "Armazenamento reservado já está desabilitado — tweak já aplicado".to_string(),
        );
    }

    // Passo 2: Salva backup do estado atual (Enabled) antes de qualquer alteração.
    // Usa value_type "STATE" pois é um estado DISM, não um valor de registro.
    backup_before_apply(
        "disable_reserved_storage",
        TweakCategory::Dism,
        "Estado do armazenamento reservado — DISM /Online /Get-ReservedStorageState",
        OriginalValue {
            path: "DISM /Online".to_string(),
            key: "ReservedStorageState".to_string(),
            value: Some(json!("Enabled")),
            value_type: "STATE".to_string(),
        },
        json!("Disabled"),
    )?;

    // Passo 3: Executa DISM com streaming de progresso linha a linha para o frontend
    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Disabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Disabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

/// Reabilita o armazenamento reservado do Windows via DISM.
///
/// Executa `DISM /Online /Set-ReservedStorageState /State:Enabled` com streaming.
/// Marca o backup como `Reverted` para liberar o tweak para nova aplicação futura.
#[tauri::command]
pub fn enable_reserved_storage(app_handle: tauri::AppHandle) -> Result<(), String> {
    // Marca o backup como Reverted — libera o tweak para ser aplicado novamente.
    // O valor original ("Enabled") não precisa ser lido: o comando DISM já faz o restore.
    restore_from_backup("disable_reserved_storage")?;

    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Enabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Enabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 3 — Otimização de Entrega  (HKLM · Registro · Baixo Risco)
//
// O Windows Update usa P2P por padrão (DODownloadMode = 1 ou 3) para distribuir
// partes de atualizações entre computadores da rede local e da internet. Esse
// processo consome upload do usuário de forma silenciosa e pode aumentar a
// latência em conexões saturadas ou durante sessões de jogos online.
//
// DODownloadMode = 0 (HTTP only) desabilita completamente o P2P, forçando o
// Windows a baixar atualizações exclusivamente dos servidores da Microsoft.
//
// Valores possíveis de DODownloadMode:
//   0 = HTTP apenas (sem P2P)
//   1 = HTTP + P2P na rede local (padrão residencial)
//   2 = HTTP + P2P na rede local (gerenciado por MDM)
//   3 = HTTP + P2P na rede local e internet
//   99 = Modo simples (sem otimização de entrega)
//   100 = Modo bypass (encaminha para BITS)
//
// Cuidados:
//   - Requer processo rodando como Administrador (escrita em HKLM)
//   - Não afeta a instalação de updates — apenas a origem dos downloads
//   - Não requer reinicialização
// ═══════════════════════════════════════════════════════════════════════════════

/// Verifica o estado atual da Otimização de Entrega no registro.
///
/// Lê `DODownloadMode` em `HKLM\...\DeliveryOptimization\Config`.
/// Usa `1` como fallback quando a chave não existe (padrão implícito do Windows).
#[tauri::command]
pub fn get_delivery_optimization_status() -> Result<DeliveryOptimizationStatus, String> {
    // Fallback para 1: P2P de rede local habilitado (padrão do Windows)
    let current_value =
        read_dword(Hive::LocalMachine, DELIVERY_OPT_REG_PATH, DELIVERY_OPT_REG_KEY)?
            .unwrap_or(DELIVERY_OPT_DEFAULT);

    Ok(DeliveryOptimizationStatus {
        // Tweak ativo = P2P desabilitado = DODownloadMode é 0
        enabled: current_value == DELIVERY_OPT_DISABLED,
        current_value,
    })
}

/// Retorna as informações completas do tweak de Otimização de Entrega.
#[tauri::command]
pub fn get_delivery_optimization_info() -> Result<TweakInfo, String> {
    let status = get_delivery_optimization_status()?;
    let (has_backup, last_applied) = backup_info("disable_delivery_optimization");

    Ok(TweakInfo {
        id: "disable_delivery_optimization".to_string(),
        name: "Desabilitar Otimização de Entrega".to_string(),
        description: "Desabilita o compartilhamento P2P de atualizações do Windows. Por padrão, \
            o Windows usa sua conexão de internet para enviar partes de updates para outros PCs. \
            Desabilitar libera banda de rede e pode melhorar latência em jogos online."
            .to_string(),
        category: "optimization".to_string(),
        is_applied: status.enabled,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: P2P habilitado (DODownloadMode = 1)".to_string(),
    })
}

/// Desabilita a Otimização de Entrega definindo `DODownloadMode = 0`.
///
/// Fluxo de execução:
/// 1. Verifica estado atual — rejeita se P2P já estiver desabilitado (idempotência)
/// 2. Lê o valor original de `DODownloadMode` (pode ser `None` se a chave não existir)
/// 3. Persiste backup do original em `backups.json` antes de qualquer modificação
/// 4. Escreve `DODownloadMode = 0` em HKLM (requer privilégios de administrador)
#[tauri::command]
pub fn disable_delivery_optimization() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação
    let status = get_delivery_optimization_status()?;
    if status.enabled {
        return Err(
            "Tweak 'disable_delivery_optimization' já está aplicado (DODownloadMode = 0)"
                .to_string(),
        );
    }

    // Passo 2: Captura o valor original ANTES de qualquer modificação
    let original_dword =
        read_dword(Hive::LocalMachine, DELIVERY_OPT_REG_PATH, DELIVERY_OPT_REG_KEY)?;
    let original_json: Option<Value> = original_dword.map(|v| json!(v));

    // Passo 3: Salva backup — `value: null` indica chave ausente (padrão implícito = 1)
    backup_before_apply(
        "disable_delivery_optimization",
        TweakCategory::Registry,
        "DODownloadMode em HKLM\\...\\DeliveryOptimization\\Config — modo P2P de updates",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", DELIVERY_OPT_REG_PATH),
            key: DELIVERY_OPT_REG_KEY.to_string(),
            value: original_json,
            value_type: "DWORD".to_string(),
        },
        json!(DELIVERY_OPT_DISABLED),
    )?;

    // Passo 4: Aplica o tweak — processo deve estar rodando como Administrador
    write_dword(
        Hive::LocalMachine,
        DELIVERY_OPT_REG_PATH,
        DELIVERY_OPT_REG_KEY,
        DELIVERY_OPT_DISABLED,
    )
}

/// Reverte a Otimização de Entrega para o estado original salvo no backup.
///
/// Comportamento conforme o valor original:
/// - `null` (chave não existia): remove `DODownloadMode` → Windows volta ao P2P padrão
/// - número: restaura esse valor exato no registro (ex: `1`, `3`)
#[tauri::command]
pub fn revert_delivery_optimization() -> Result<(), String> {
    // Recupera o original e atomicamente marca o backup como Reverted no disco
    let original = restore_from_backup("disable_delivery_optimization")?;

    match original.value {
        // Chave não existia antes — remove para restaurar comportamento padrão implícito
        None => {
            if key_exists(Hive::LocalMachine, DELIVERY_OPT_REG_PATH, DELIVERY_OPT_REG_KEY)? {
                delete_value(
                    Hive::LocalMachine,
                    DELIVERY_OPT_REG_PATH,
                    DELIVERY_OPT_REG_KEY,
                )?;
            }
        }
        // Chave existia com valor numérico — restaura o valor original exato
        Some(Value::Number(n)) => {
            let v = n.as_u64().unwrap_or(DELIVERY_OPT_DEFAULT as u64) as u32;
            write_dword(
                Hive::LocalMachine,
                DELIVERY_OPT_REG_PATH,
                DELIVERY_OPT_REG_KEY,
                v,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_delivery_optimization': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 4 — HAGS (Hardware-Accelerated GPU Scheduling)
//
// Permite que a GPU gerencie sua própria memória de vídeo diretamente, reduzindo
// a latência de renderização e a carga sobre a CPU. Recomendado para gaming.
//
// HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode
//   2 = habilitado (padrão no Windows 11 para GPUs compatíveis)
//   0 = desabilitado
// ═══════════════════════════════════════════════════════════════════════════════

const HAGS_REG_PATH: &str = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
const HAGS_REG_KEY: &str = "HwSchMode";
const HAGS_ENABLED_VALUE: u32 = 2;
const HAGS_DISABLED_VALUE: u32 = 0;

/// Retorna as informações do tweak HAGS com o estado atual do registro.
#[tauri::command]
pub fn get_hags_info() -> Result<TweakInfo, String> {
    let is_enabled = read_dword(Hive::LocalMachine, HAGS_REG_PATH, HAGS_REG_KEY)?
        .map(|v| v == HAGS_ENABLED_VALUE)
        .unwrap_or(true); // padrão Windows 11: HAGS ativo para GPUs compatíveis

    let (has_backup, last_applied) = backup_info("enable_hags");

    Ok(TweakInfo {
        id: "enable_hags".to_string(),
        name: "Hardware-Accelerated GPU Scheduling (HAGS)".to_string(),
        description: "Permite que a GPU gerencie sua própria memória de vídeo diretamente, \
            reduzindo a latência de renderização e a carga sobre a CPU. Recomendado para gaming."
            .to_string(),
        category: "gamer".to_string(),
        is_applied: is_enabled,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows 11: HAGS ativo (HwSchMode = 2)".to_string(),
    })
}

/// Habilita HAGS definindo HwSchMode = 2 no registro.
#[tauri::command]
pub fn enable_hags() -> Result<(), String> {
    write_dword(Hive::LocalMachine, HAGS_REG_PATH, HAGS_REG_KEY, HAGS_ENABLED_VALUE)
}

/// Desabilita HAGS definindo HwSchMode = 0 no registro.
#[tauri::command]
pub fn disable_hags() -> Result<(), String> {
    write_dword(Hive::LocalMachine, HAGS_REG_PATH, HAGS_REG_KEY, HAGS_DISABLED_VALUE)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 5 — Windows Game Mode
//
// Prioriza recursos de CPU e GPU para o jogo em execução, reduzindo a interferência
// de processos em segundo plano como atualizações do Windows.
//
// HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled
//   1 = habilitado (padrão)
//   0 = desabilitado
// ═══════════════════════════════════════════════════════════════════════════════

const GAME_MODE_REG_PATH: &str = r"Software\Microsoft\GameBar";
const GAME_MODE_REG_KEY: &str = "AutoGameModeEnabled";
const GAME_MODE_ENABLED: u32 = 1;
const GAME_MODE_DISABLED: u32 = 0;

/// Retorna as informações do tweak Game Mode com o estado atual do registro.
#[tauri::command]
pub fn get_game_mode_info() -> Result<TweakInfo, String> {
    let is_enabled = read_dword(Hive::CurrentUser, GAME_MODE_REG_PATH, GAME_MODE_REG_KEY)?
        .map(|v| v == GAME_MODE_ENABLED)
        .unwrap_or(true); // padrão: Game Mode ativo desde Windows 10 Creators Update

    let (has_backup, last_applied) = backup_info("enable_game_mode");

    Ok(TweakInfo {
        id: "enable_game_mode".to_string(),
        name: "Windows Game Mode".to_string(),
        description: "Prioriza recursos de CPU e GPU para o jogo em execução, reduzindo a \
            interferência de processos em segundo plano como atualizações do Windows. \
            Recomendado para melhor desempenho em jogos."
            .to_string(),
        category: "gamer".to_string(),
        is_applied: is_enabled,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description: "Padrão Windows: Game Mode ativo (AutoGameModeEnabled = 1)".to_string(),
    })
}

/// Habilita Game Mode definindo AutoGameModeEnabled = 1 no registro.
#[tauri::command]
pub fn enable_game_mode() -> Result<(), String> {
    write_dword(Hive::CurrentUser, GAME_MODE_REG_PATH, GAME_MODE_REG_KEY, GAME_MODE_ENABLED)
}

/// Desabilita Game Mode definindo AutoGameModeEnabled = 0 no registro.
#[tauri::command]
pub fn disable_game_mode() -> Result<(), String> {
    write_dword(Hive::CurrentUser, GAME_MODE_REG_PATH, GAME_MODE_REG_KEY, GAME_MODE_DISABLED)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK 6 — VBS (Virtualization Based Security)
//
// A VBS usa virtualização de hardware para isolar partes críticas do Windows,
// mas pode reduzir o desempenho em jogos em até 10–15%.
//
// HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\EnableVirtualizationBasedSecurity
//   1 = habilitado
//   0 = desabilitado (padrão em hardware sem TPM ou com VBS desativado na BIOS)
// ═══════════════════════════════════════════════════════════════════════════════

const VBS_REG_PATH: &str = r"SYSTEM\CurrentControlSet\Control\DeviceGuard";
const VBS_REG_KEY: &str = "EnableVirtualizationBasedSecurity";
const VBS_ENABLED: u32 = 1;
const VBS_DISABLED: u32 = 0;

/// Retorna as informações do tweak VBS com o estado atual do registro.
///
/// `is_applied = true` indica que a VBS está **desabilitada** — estado recomendado
/// para maximizar performance em jogos.
#[tauri::command]
pub fn get_vbs_info() -> Result<TweakInfo, String> {
    let vbs_enabled = read_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY)?
        .map(|v| v == VBS_ENABLED)
        .unwrap_or(false); // padrão: VBS inativo em muitos sistemas

    let (has_backup, last_applied) = backup_info("disable_vbs");

    Ok(TweakInfo {
        id: "disable_vbs".to_string(),
        name: "Virtualização Baseada em Segurança (VBS)".to_string(),
        description: "A VBS usa virtualização de hardware para isolar partes críticas do Windows, \
            mas pode reduzir o desempenho em jogos em até 10–15%. Desabilitar melhora FPS, \
            especialmente em CPUs sem hardware de virtualização otimizado."
            .to_string(),
        category: "gamer".to_string(),
        is_applied: !vbs_enabled, // tweak "aplicado" = VBS desabilitada = bom para gaming
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows 11: VBS ativa (EnableVirtualizationBasedSecurity = 1)".to_string(),
    })
}

/// Desabilita VBS definindo EnableVirtualizationBasedSecurity = 0.
/// Requer reinicialização para ter efeito.
#[tauri::command]
pub fn disable_vbs() -> Result<(), String> {
    write_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY, VBS_DISABLED)
}

/// Reabilita VBS definindo EnableVirtualizationBasedSecurity = 1.
/// Requer reinicialização para ter efeito.
#[tauri::command]
pub fn enable_vbs() -> Result<(), String> {
    write_dword(Hive::LocalMachine, VBS_REG_PATH, VBS_REG_KEY, VBS_ENABLED)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Restaurar Padrão Windows — sem backup
//
// Esses comandos restauram o valor padrão conhecido do Windows para tweaks
// que foram aplicados externamente (sem backup no FrameGuard). Não dependem
// de entrada em backups.json. Permitem que o usuário desfaça o tweak e depois
// use o fluxo normal "Aplicar" para criar o backup pela primeira vez.
// ═══════════════════════════════════════════════════════════════════════════════

/// Remove a chave `JPEGImportQuality`, restaurando a compressão padrão do Windows (85%).
///
/// Equivale ao estado de fábrica do Windows — o sistema aplica 85% internamente
/// quando a chave está ausente.
#[tauri::command]
pub fn restore_wallpaper_default() -> Result<(), String> {
    if key_exists(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)? {
        delete_value(Hive::CurrentUser, WALLPAPER_REG_PATH, WALLPAPER_REG_KEY)?;
    }
    Ok(())
}

/// Remove a chave `DODownloadMode`, restaurando o modo P2P padrão do Windows Update.
///
/// Com a chave ausente, o Windows usa o padrão implícito DODownloadMode = 1
/// (P2P habilitado entre PCs da rede local).
#[tauri::command]
pub fn restore_delivery_optimization_default() -> Result<(), String> {
    if key_exists(Hive::LocalMachine, DELIVERY_OPT_REG_PATH, DELIVERY_OPT_REG_KEY)? {
        delete_value(Hive::LocalMachine, DELIVERY_OPT_REG_PATH, DELIVERY_OPT_REG_KEY)?;
    }
    Ok(())
}

/// Reabilita o Armazenamento Reservado via DISM sem precisar de backup.
///
/// Diferente de `enable_reserved_storage`, não chama `restore_from_backup` —
/// pode ser usado quando o tweak foi aplicado externamente e não há entrada
/// no backups.json do FrameGuard.
#[tauri::command]
pub fn restore_reserved_storage_default(app_handle: tauri::AppHandle) -> Result<(), String> {
    let result = run_command_with_progress(
        &app_handle,
        "dism-reserved-storage",
        "powershell.exe",
        &[
            "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe /Online /Set-ReservedStorageState /State:Enabled",
        ],
        Some("dism.exe /Online /Set-ReservedStorageState /State:Enabled"),
    )?;

    if !result.success {
        return Err(format!(
            "DISM retornou código de erro {}: {}",
            result.exit_code,
            result.stderr.trim()
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// GPU e Display — Utilitário interno para backup multi-chave
// ═══════════════════════════════════════════════════════════════════════════════

/// Restaura múltiplas entradas de registro (e opcionalmente um serviço Windows) a partir
/// de um array JSON armazenado no campo `value` de um backup `MULTI_DWORD`.
///
/// Cada elemento do array deve ter o formato:
/// - Registro: `{"hive":"HKCU"|"HKLM", "path":"...", "key":"...", "value":null|number}`
/// - Serviço:  `{"type":"service", "name":"...", "value":"Automatic"|"Manual"|"Disabled"|null}`
///
/// Para entradas de registro, `"value": null` significa que a chave não existia antes
/// do tweak — nesse caso ela é deletada para restaurar o padrão do Windows.
fn restore_multi_entries(entries: &[Value]) -> Result<(), String> {
    for entry in entries {
        let entry_type = entry
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("registry");

        match entry_type {
            "service" => {
                let name = entry["name"].as_str().unwrap_or("");
                // null = serviço não existia antes; não há nada a restaurar
                if let Some(start_type) = entry["value"].as_str() {
                    let script = format!(
                        "Set-Service -Name '{}' -StartupType {} -ErrorAction SilentlyContinue",
                        name, start_type
                    );
                    run_powershell(&script)?;
                }
            }
            _ => {
                let hive_str = entry["hive"].as_str().unwrap_or("HKCU");
                let path = entry["path"].as_str().unwrap_or("");
                let key = entry["key"].as_str().unwrap_or("");
                let hive = if hive_str == "HKLM" {
                    Hive::LocalMachine
                } else {
                    Hive::CurrentUser
                };

                match &entry["value"] {
                    Value::Null => {
                        if key_exists(hive, path, key)? {
                            delete_value(hive, path, key)?;
                        }
                    }
                    Value::Number(n) => {
                        write_dword(hive, path, key, n.as_u64().unwrap_or(0) as u32)?;
                    }
                    // REG_SZ: usado por tweaks como aceleração do mouse (MouseSpeed, etc.)
                    Value::String(s) => {
                        write_string(hive, path, key, s)?;
                    }
                    other => {
                        return Err(format!(
                            "Tipo inesperado no backup multi-entry (key={}): {:?}",
                            key, other
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Game DVR / Background Recording
//
// Três chaves de registro controlam o Game DVR:
//   1. HKCU\System\GameConfigStore -> GameDVR_Enabled
//   2. HKLM\SOFTWARE\Policies\Microsoft\Windows\GameDVR -> AllowGameDVR
//   3. HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR -> AppCaptureEnabled
//
// Todas devem ser 0 para considerar o tweak ativo. O encoder de vídeo da GPU
// fica inativo e o buffer circular de memória não é alocado.
// ═══════════════════════════════════════════════════════════════════════════════

fn get_game_dvr_is_applied() -> Result<bool, String> {
    let v1 = read_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, GAME_DVR_KEY_ENABLED)?
        .unwrap_or(1);
    let v2 = read_dword(Hive::LocalMachine, GAME_DVR_PATH_POLICIES, GAME_DVR_KEY_ALLOW)?
        .unwrap_or(1);
    let v3 = read_dword(Hive::CurrentUser, GAME_DVR_PATH_APPCAP, GAME_DVR_KEY_APPCAP)?
        .unwrap_or(1);
    Ok(v1 == 0 && v2 == 0 && v3 == 0)
}

#[tauri::command]
pub fn get_game_dvr_info() -> Result<TweakInfo, String> {
    let is_applied = get_game_dvr_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_game_dvr");
    Ok(TweakInfo {
        id: "disable_game_dvr".to_string(),
        name: "Desabilitar Game DVR / Gravação em Segundo Plano".to_string(),
        description: "Desabilita a gravação em segundo plano do Game DVR, liberando recursos da \
            GPU (encoder) e CPU. Diferente do Game Mode, que prioriza recursos, o Game DVR \
            ativamente grava vídeo em buffer circular mesmo quando você não está gravando."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: Game DVR habilitado (GameDVR_Enabled = 1)"
            .to_string(),
    })
}

/// Desabilita o Game DVR zerando as três chaves de registro que o controlam.
///
/// Persiste backup multi-chave em `backups.json` antes de qualquer modificação.
#[tauri::command]
pub fn disable_game_dvr() -> Result<(), String> {
    if get_game_dvr_is_applied()? {
        return Err("Tweak 'disable_game_dvr' já está aplicado".to_string());
    }

    let orig_1 = read_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, GAME_DVR_KEY_ENABLED)?;
    let orig_2 = read_dword(Hive::LocalMachine, GAME_DVR_PATH_POLICIES, GAME_DVR_KEY_ALLOW)?;
    let orig_3 = read_dword(Hive::CurrentUser, GAME_DVR_PATH_APPCAP, GAME_DVR_KEY_APPCAP)?;

    let v1 = orig_1.map(|v| json!(v)).unwrap_or(Value::Null);
    let v2 = orig_2.map(|v| json!(v)).unwrap_or(Value::Null);
    let v3 = orig_3.map(|v| json!(v)).unwrap_or(Value::Null);

    backup_before_apply(
        "disable_game_dvr",
        TweakCategory::Registry,
        "Game DVR — GameDVR_Enabled + AllowGameDVR + AppCaptureEnabled",
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
                    "hive": "HKLM",
                    "path": GAME_DVR_PATH_POLICIES,
                    "key": GAME_DVR_KEY_ALLOW,
                    "value": v2
                },
                {
                    "hive": "HKCU",
                    "path": GAME_DVR_PATH_APPCAP,
                    "key": GAME_DVR_KEY_APPCAP,
                    "value": v3
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([0, 0, 0]),
    )?;

    write_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, GAME_DVR_KEY_ENABLED, 0)?;
    write_dword(Hive::LocalMachine, GAME_DVR_PATH_POLICIES, GAME_DVR_KEY_ALLOW, 0)?;
    write_dword(Hive::CurrentUser, GAME_DVR_PATH_APPCAP, GAME_DVR_KEY_APPCAP, 0)?;
    Ok(())
}

/// Restaura as três chaves do Game DVR para seus valores originais a partir do backup.
#[tauri::command]
pub fn revert_game_dvr() -> Result<(), String> {
    let original = restore_from_backup("disable_game_dvr")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_game_dvr' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_game_dvr'")?;
    restore_multi_entries(arr)
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

fn get_xbox_overlay_is_applied() -> Result<bool, String> {
    let v1 = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_NEXUS)?
        .unwrap_or(1);
    let v2 = read_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_PANEL)?
        .unwrap_or(1);
    Ok(v1 == 0 && v2 == 0)
}

#[tauri::command]
pub fn get_xbox_overlay_info() -> Result<TweakInfo, String> {
    let is_applied = get_xbox_overlay_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_xbox_overlay");
    Ok(TweakInfo {
        id: "disable_xbox_overlay".to_string(),
        name: "Desabilitar Xbox Game Bar Overlay".to_string(),
        description: "Remove o overlay da Xbox Game Bar que pode ser ativado acidentalmente \
            durante jogos (Win+G). Impacto em recursos é mínimo, mas elimina o processo \
            GameBarPresenceWriter.exe."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: Xbox Game Bar habilitado (UseNexusForGameBarEnabled = 1)".to_string(),
    })
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

    write_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_NEXUS, 0)?;
    write_dword(Hive::CurrentUser, XBOX_OVERLAY_PATH, XBOX_OVERLAY_KEY_PANEL, 0)?;
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
// TWEAK — Habilitar MSI Mode para GPU NVIDIA
//
// Detecta o InstanceId da GPU NVIDIA via Get-PnpDevice e configura:
//   HKLM\SYSTEM\CurrentControlSet\Enum\{InstanceId}\Device Parameters\
//       Interrupt Management\MessageSignaledInterruptProperties -> MSISupported = 1
//
// GPUs RTX 40+ já usam MSI por padrão. Benefício principal em RTX 30 e anteriores.
// O caminho dinâmico é salvo integralmente no backup para reversão segura.
// Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

/// Busca o InstanceId da primeira GPU NVIDIA de Display encontrada via Get-PnpDevice.
/// Retorna `None` se nenhuma GPU NVIDIA estiver instalada.
fn get_nvidia_instance_id() -> Result<Option<String>, String> {
    let output = run_powershell(
        "(Get-PnpDevice | Where-Object { \
            $_.FriendlyName -like '*NVIDIA*' -and $_.Class -eq 'Display' \
        } | Select-Object -First 1).InstanceId",
    )?;
    let id = output.stdout.trim().to_string();
    if id.is_empty() {
        Ok(None)
    } else {
        Ok(Some(id))
    }
}

/// Monta o caminho de registro MSI Interrupt Properties para o InstanceId fornecido.
fn msi_reg_path(instance_id: &str) -> String {
    format!(
        r"SYSTEM\CurrentControlSet\Enum\{}\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties",
        instance_id
    )
}

fn get_msi_mode_is_applied(instance_id: &str) -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, &msi_reg_path(instance_id), "MSISupported")?
        .unwrap_or(0);
    Ok(val == 1)
}

#[tauri::command]
pub fn get_msi_mode_gpu_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("enable_msi_mode_gpu");
    let is_applied = match get_nvidia_instance_id()? {
        Some(id) => get_msi_mode_is_applied(&id).unwrap_or(false),
        None => false,
    };
    Ok(TweakInfo {
        id: "enable_msi_mode_gpu".to_string(),
        name: "Habilitar MSI Mode para GPU".to_string(),
        description: "Habilita Message Signaled Interrupts para a GPU, reduzindo latência de \
            DPC. GPUs RTX 40+ já usam MSI por padrão. Benefício principal em GPUs RTX 30 e \
            anteriores."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: MSI Mode desabilitado para GPU (MSISupported ausente ou 0)"
                .to_string(),
    })
}

/// Habilita MSI Mode na GPU NVIDIA detectada automaticamente.
///
/// O caminho de registro dinâmico (contendo o InstanceId do dispositivo) é salvo
/// integralmente no backup para garantir reversão correta mesmo após updates de driver.
#[tauri::command]
pub fn enable_msi_mode_gpu() -> Result<(), String> {
    let instance_id = get_nvidia_instance_id()?
        .ok_or("Nenhuma GPU NVIDIA detectada — MSI Mode não pode ser configurado")?;

    if get_msi_mode_is_applied(&instance_id)? {
        return Err("Tweak 'enable_msi_mode_gpu' já está aplicado".to_string());
    }

    let reg_path = msi_reg_path(&instance_id);
    let original = read_dword(Hive::LocalMachine, &reg_path, "MSISupported")?;

    backup_before_apply(
        "enable_msi_mode_gpu",
        TweakCategory::Registry,
        "MSI Mode GPU — MSISupported no caminho do dispositivo NVIDIA",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", reg_path),
            key: "MSISupported".to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(1),
    )?;

    write_dword(Hive::LocalMachine, &reg_path, "MSISupported", 1)
}

/// Reverte MSI Mode para o estado original; usa o caminho salvo no backup para
/// acessar o mesmo dispositivo que foi configurado durante o apply.
#[tauri::command]
pub fn disable_msi_mode_gpu() -> Result<(), String> {
    let original = restore_from_backup("enable_msi_mode_gpu")?;

    // O caminho dinâmico foi salvo como "HKEY_LOCAL_MACHINE\..."
    let full_path = &original.path;
    let reg_path = full_path
        .strip_prefix("HKEY_LOCAL_MACHINE\\")
        .ok_or_else(|| {
            format!(
                "Caminho de backup inválido para MSI Mode: {}",
                full_path
            )
        })?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, reg_path, "MSISupported")? {
                delete_value(Hive::LocalMachine, reg_path, "MSISupported")?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                reg_path,
                "MSISupported",
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'enable_msi_mode_gpu': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Multiplane Overlay (MPO)
//
// HKLM\SOFTWARE\Microsoft\Windows\Dwm -> OverlayTestMode = 5
//
// Ausência da chave ou valor != 5 significa MPO habilitado (padrão Windows).
// Recomendado para configurações dual-monitor com refresh rates diferentes.
// Requer reinicialização.
// ═══════════════════════════════════════════════════════════════════════════════

fn get_mpo_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY)?.unwrap_or(0);
    Ok(val == MPO_DISABLED_VALUE)
}

#[tauri::command]
pub fn get_mpo_info() -> Result<TweakInfo, String> {
    let is_applied = get_mpo_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_mpo");
    Ok(TweakInfo {
        id: "disable_mpo".to_string(),
        name: "Desabilitar Multiplane Overlay (MPO)".to_string(),
        description: "Desabilita o Multiplane Overlay do DWM, que pode causar stuttering e \
            flickering em configurações multi-monitor com refresh rates diferentes. Recomendado \
            se você usa dois monitores com Hz diferentes."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: MPO habilitado (OverlayTestMode ausente)"
            .to_string(),
    })
}

/// Desabilita o MPO escrevendo `OverlayTestMode = 5` em HKLM\SOFTWARE\Microsoft\Windows\Dwm.
#[tauri::command]
pub fn disable_mpo() -> Result<(), String> {
    if get_mpo_is_applied()? {
        return Err("Tweak 'disable_mpo' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY)?;

    backup_before_apply(
        "disable_mpo",
        TweakCategory::Registry,
        "MPO — OverlayTestMode em HKLM\\SOFTWARE\\Microsoft\\Windows\\Dwm",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", MPO_PATH),
            key: MPO_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(MPO_DISABLED_VALUE),
    )?;

    write_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY, MPO_DISABLED_VALUE)
}

/// Reverte o MPO para o estado original: remove a chave (se ausente antes) ou restaura o valor.
#[tauri::command]
pub fn revert_mpo() -> Result<(), String> {
    let original = restore_from_backup("disable_mpo")?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, MPO_PATH, MPO_KEY)? {
                delete_value(Hive::LocalMachine, MPO_PATH, MPO_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(Hive::LocalMachine, MPO_PATH, MPO_KEY, n.as_u64().unwrap_or(0) as u32)?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_mpo': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar NVIDIA Telemetry
//
// Quatro chaves de registro + serviço NvTelemetryContainer:
//   1. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID44231 = 0
//   2. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID64640 = 0
//   3. HKLM\SOFTWARE\NVIDIA Corporation\Global\FTS -> EnableRID66610 = 0
//   4. HKLM\SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client -> OptInOrOutPreference = 0
//   5. Serviço NvTelemetryContainer -> Disabled (se existir)
//
// Não afeta funcionalidade do driver.
// ═══════════════════════════════════════════════════════════════════════════════

/// Consulta o tipo de inicialização atual de um serviço Windows via PowerShell.
/// Retorna `None` se o serviço não existir no sistema.
fn get_service_start_type(name: &str) -> Result<Option<String>, String> {
    let script = format!(
        "(Get-Service -Name '{}' -ErrorAction SilentlyContinue).StartType",
        name
    );
    let output = run_powershell(&script)?;
    let trimmed = output.stdout.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

fn get_nvidia_telemetry_is_applied() -> Result<bool, String> {
    let v1 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1)?.unwrap_or(1);
    let v2 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2)?.unwrap_or(1);
    let v3 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3)?.unwrap_or(1);
    let v4 = read_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY)?.unwrap_or(1);

    let regs_disabled = v1 == 0 && v2 == 0 && v3 == 0 && v4 == 0;

    // Serviço desabilitado ou inexistente também indica tweak aplicado
    let svc_ok = match get_service_start_type(NVIDIA_TELEMETRY_SERVICE)? {
        None => true, // serviço não existe — sem telemetria ativa
        Some(t) => t.eq_ignore_ascii_case("Disabled"),
    };

    Ok(regs_disabled && svc_ok)
}

#[tauri::command]
pub fn get_nvidia_telemetry_info() -> Result<TweakInfo, String> {
    let is_applied = get_nvidia_telemetry_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_nvidia_telemetry");
    Ok(TweakInfo {
        id: "disable_nvidia_telemetry".to_string(),
        name: "Desabilitar Telemetria NVIDIA".to_string(),
        description: "Desabilita a coleta de telemetria do driver NVIDIA. Remove uso de CPU e \
            rede em segundo plano sem afetar funcionalidade do driver."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão NVIDIA: telemetria habilitada e serviço NvTelemetryContainer ativo"
                .to_string(),
    })
}

/// Desabilita a telemetria NVIDIA: zera 4 chaves de registro e desabilita o serviço.
///
/// O estado original do serviço (Automatic/Manual/Disabled/inexistente) é preservado
/// no backup para reversão precisa.
#[tauri::command]
pub fn disable_nvidia_telemetry() -> Result<(), String> {
    if get_nvidia_telemetry_is_applied()? {
        return Err("Tweak 'disable_nvidia_telemetry' já está aplicado".to_string());
    }

    let orig_1 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1)?;
    let orig_2 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2)?;
    let orig_3 = read_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3)?;
    let orig_4 = read_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY)?;
    let orig_svc = get_service_start_type(NVIDIA_TELEMETRY_SERVICE)?;

    let v1 = orig_1.map(|v| json!(v)).unwrap_or(Value::Null);
    let v2 = orig_2.map(|v| json!(v)).unwrap_or(Value::Null);
    let v3 = orig_3.map(|v| json!(v)).unwrap_or(Value::Null);
    let v4 = orig_4.map(|v| json!(v)).unwrap_or(Value::Null);
    let svc_val: Value = orig_svc
        .as_deref()
        .map(|t| json!(t))
        .unwrap_or(Value::Null);

    backup_before_apply(
        "disable_nvidia_telemetry",
        TweakCategory::Registry,
        "NVIDIA Telemetria — 4 chaves FTS/CP + serviço NvTelemetryContainer",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "nvidia_telemetry_keys".to_string(),
            value: Some(json!([
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_1,
                    "value": v1
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_2,
                    "value": v2
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_FTS_PATH,
                    "key": NVIDIA_FTS_KEY_3,
                    "value": v3
                },
                {
                    "hive": "HKLM",
                    "path": NVIDIA_CP_PATH,
                    "key": NVIDIA_CP_KEY,
                    "value": v4
                },
                {
                    "type": "service",
                    "name": NVIDIA_TELEMETRY_SERVICE,
                    "value": svc_val
                }
            ])),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([0, 0, 0, 0, "Disabled"]),
    )?;

    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_1, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_2, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_FTS_PATH, NVIDIA_FTS_KEY_3, 0)?;
    write_dword(Hive::LocalMachine, NVIDIA_CP_PATH, NVIDIA_CP_KEY, 0)?;

    // Desabilita o serviço apenas se ele existir no sistema
    if orig_svc.is_some() {
        let script = format!(
            "Set-Service -Name '{}' -StartupType Disabled -ErrorAction SilentlyContinue",
            NVIDIA_TELEMETRY_SERVICE
        );
        run_powershell(&script)?;
    }

    Ok(())
}

/// Restaura todas as chaves e o serviço NvTelemetryContainer para os estados originais.
#[tauri::command]
pub fn revert_nvidia_telemetry() -> Result<(), String> {
    let original = restore_from_backup("disable_nvidia_telemetry")?;
    let entries = original
        .value
        .ok_or("Backup de 'disable_nvidia_telemetry' está vazio")?;
    let arr = entries
        .as_array()
        .ok_or("Formato de backup inválido para 'disable_nvidia_telemetry'")?;
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

fn get_timer_resolution_is_applied() -> Result<bool, String> {
    let val = read_dword(Hive::LocalMachine, TIMER_RES_PATH, TIMER_RES_KEY)?.unwrap_or(0);
    Ok(val == 1)
}

#[tauri::command]
pub fn get_timer_resolution_info() -> Result<TweakInfo, String> {
    let is_applied = get_timer_resolution_is_applied()?;
    let (has_backup, last_applied) = backup_info("enable_timer_resolution");
    Ok(TweakInfo {
        id: "enable_timer_resolution".to_string(),
        name: "Timer de Alta Resolução (GlobalTimerResolutionRequests)".to_string(),
        description: "Permite que aplicações solicitem timer resolution global de 1ms em vez \
            do padrão 15,6ms. Melhora frame pacing e reduz input lag, especialmente em \
            monitores 144Hz+. Específico do Windows 11."
            .to_string(),
        category: "gaming".to_string(),
        is_applied,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: GlobalTimerResolutionRequests ausente (timer 15,6ms)".to_string(),
    })
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

fn get_mouse_acc_is_applied() -> Result<bool, String> {
    let speed =
        read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_SPEED_KEY)?.unwrap_or_default();
    let thr1 = read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD1_KEY)?
        .unwrap_or_default();
    let thr2 = read_string(Hive::CurrentUser, MOUSE_ACC_PATH, MOUSE_THRESHOLD2_KEY)?
        .unwrap_or_default();
    Ok(speed == "0" && thr1 == "0" && thr2 == "0")
}

#[tauri::command]
pub fn get_mouse_acceleration_info() -> Result<TweakInfo, String> {
    let is_applied = get_mouse_acc_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_mouse_acceleration");
    Ok(TweakInfo {
        id: "disable_mouse_acceleration".to_string(),
        name: "Desabilitar Aceleração do Mouse".to_string(),
        description: "Remove a curva não-linear de resposta do mouse do Windows. Essencial \
            para mira consistente em jogos FPS. O movimento do cursor passa a ser 1:1 com \
            o movimento físico do mouse."
            .to_string(),
        category: "gaming".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: MouseSpeed = \"1\", Threshold1 = \"6\", Threshold2 = \"10\""
                .to_string(),
    })
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

fn get_fso_is_applied() -> Result<bool, String> {
    for (key, target) in &FSO_TARGET {
        // unwrap_or(99): chave ausente nunca coincide com valores alvo (0, 1, 2)
        let val =
            read_dword(Hive::CurrentUser, GAME_DVR_PATH_GAMECONFIG, key)?.unwrap_or(99);
        if val != *target {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn get_fullscreen_optimizations_info() -> Result<TweakInfo, String> {
    let is_applied = get_fso_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_fullscreen_optimizations");
    Ok(TweakInfo {
        id: "disable_fullscreen_optimizations".to_string(),
        name: "Desabilitar Fullscreen Optimizations (global)".to_string(),
        description: "Força jogos a usar fullscreen exclusivo em vez do modo otimizado do \
            Windows. Era relevante no Windows 10, mas no Windows 11 o sistema de FSO foi \
            significativamente melhorado. Pode beneficiar jogos DX9/DX11 mais antigos. Para \
            jogos DX12/Vulkan modernos, o impacto é negligível ou inexistente."
            .to_string(),
        category: "gaming".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description:
            "Padrão Windows: Fullscreen Optimizations habilitado (chaves FSE ausentes)"
                .to_string(),
    })
}

/// Aplica as 5 chaves FSO de uma vez, preservando cada valor original no backup.
#[tauri::command]
pub fn disable_fullscreen_optimizations() -> Result<(), String> {
    if get_fso_is_applied()? {
        return Err("Tweak 'disable_fullscreen_optimizations' já está aplicado".to_string());
    }

    // Lê todos os originais antes de qualquer modificação
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

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Ultimate Performance Power Plan
//
// Duplica o plano template (GUID e9a42b02-...) com powercfg -duplicatescheme
// e o ativa. O GUID do plano ativo original é salvo no backup para reversão.
//
// Se o sistema usa Modern Standby (bloqueio à duplicação), escreve
// PlatformAoAcOverride = 0 em HKLM\SYSTEM\...\Power antes de tentar de novo.
// ═══════════════════════════════════════════════════════════════════════════════

/// Extrai um GUID UUID (8-4-4-4-12) da linha de saída do powercfg.
/// Funciona tanto em Windows PT-BR quanto EN, ignorando texto ao redor.
fn extract_guid_from_powercfg(s: &str) -> Result<String, String> {
    for word in s.split_whitespace() {
        // Remove parênteses/aspas que possam envolver o GUID
        let clean = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
        if clean.len() == 36 {
            let b = clean.as_bytes();
            if b[8] == b'-' && b[13] == b'-' && b[18] == b'-' && b[23] == b'-' {
                if clean.replace('-', "").chars().all(|c| c.is_ascii_hexdigit()) {
                    return Ok(clean.to_string());
                }
            }
        }
    }
    Err(format!(
        "GUID não encontrado na saída do powercfg: {}",
        s.trim()
    ))
}

/// Retorna o GUID do plano de energia atualmente ativo.
fn get_active_power_scheme_guid() -> Result<String, String> {
    let output = run_powershell("powercfg /getactivescheme")?;
    extract_guid_from_powercfg(&output.stdout)
}

fn get_ultimate_performance_is_applied() -> Result<bool, String> {
    let output = run_powershell("powercfg /getactivescheme")?;
    // "Ultimate Performance" é o nome em EN; Windows PT-BR também usa este nome
    // após duplicação do template GUID e9a42b02-...
    Ok(output.stdout.to_lowercase().contains("ultimate"))
}

#[tauri::command]
pub fn get_ultimate_performance_info() -> Result<TweakInfo, String> {
    let is_applied = get_ultimate_performance_is_applied().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("enable_ultimate_performance");
    Ok(TweakInfo {
        id: "enable_ultimate_performance".to_string(),
        name: "Plano de Energia: Ultimate Performance".to_string(),
        description: "Ativa o plano de energia Ultimate Performance, que mantém o processador \
            em frequência máxima constantemente. Elimina latência de boost de CPU. Escondido \
            por padrão no Windows 11."
            .to_string(),
        category: "energy_cpu".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: plano Balanceado ou Alto Desempenho ativo"
            .to_string(),
    })
}

/// Ativa o plano Ultimate Performance duplicando-o a partir do GUID template.
///
/// Fluxo:
/// 1. Captura GUID ativo original (para backup de reversão)
/// 2. Executa `powercfg -duplicatescheme` — obtém novo GUID
/// 3. Se bloqueado por Modern Standby, escreve PlatformAoAcOverride=0 e tenta de novo
/// 4. Salva backup com GUID original antes de ativar
/// 5. Executa `powercfg -setactive [novo GUID]`
#[tauri::command]
pub fn enable_ultimate_performance() -> Result<(), String> {
    if get_ultimate_performance_is_applied()? {
        return Err("Tweak 'enable_ultimate_performance' já está aplicado".to_string());
    }

    // Passo 1: captura GUID ativo antes de qualquer mudança
    let original_guid = get_active_power_scheme_guid()?;

    // Passo 2: tenta duplicar o plano template
    let dup_cmd = format!("powercfg -duplicatescheme {}", ULTIMATE_PERF_GUID);
    let dup_output = match run_powershell(&dup_cmd) {
        Ok(out) if out.success && !out.stdout.trim().is_empty() => out,
        // Falhou (Modern Standby bloqueia) — aplica override e tenta novamente
        _ => {
            write_dword(Hive::LocalMachine, MODERN_STANDBY_PATH, MODERN_STANDBY_KEY, 0)?;
            let retry = run_powershell(&dup_cmd)?;
            if !retry.success || retry.stdout.trim().is_empty() {
                return Err(format!(
                    "Falha ao duplicar plano Ultimate Performance (Modern Standby): {}",
                    retry.stderr
                ));
            }
            retry
        }
    };

    // Passo 3: extrai o novo GUID da saída do powercfg
    let new_guid = extract_guid_from_powercfg(&dup_output.stdout)?;

    // Passo 4: backup ANTES de ativar — preserva o plano original para reversão
    backup_before_apply(
        "enable_ultimate_performance",
        TweakCategory::Powershell,
        "Ultimate Performance — GUID do plano de energia ativo antes da troca",
        OriginalValue {
            path: "powercfg".to_string(),
            key: "active_scheme_guid".to_string(),
            value: Some(json!(original_guid)),
            value_type: "STRING".to_string(),
        },
        json!(new_guid),
    )?;

    // Passo 5: ativa o novo plano
    let activate = run_powershell(&format!("powercfg -setactive {}", new_guid))?;
    if !activate.success {
        return Err(format!(
            "Falha ao ativar plano Ultimate Performance ({}): {}",
            new_guid, activate.stderr
        ));
    }

    Ok(())
}

/// Restaura o plano de energia que estava ativo antes da aplicação do tweak.
#[tauri::command]
pub fn revert_ultimate_performance() -> Result<(), String> {
    let original = restore_from_backup("enable_ultimate_performance")?;

    let original_guid = original
        .value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("GUID original não encontrado no backup de 'enable_ultimate_performance'")?;

    let output = run_powershell(&format!("powercfg -setactive {}", original_guid))?;
    if !output.success {
        return Err(format!(
            "Falha ao restaurar plano de energia ({}): {}",
            original_guid, output.stderr
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TWEAK — Desabilitar Power Throttling
//
// HKLM\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling
//   -> PowerThrottlingOff = 1 (DWORD)
//
// Cria o caminho automaticamente se não existir. Ausente ou 0 = throttling ativo.
// ═══════════════════════════════════════════════════════════════════════════════

fn get_power_throttling_is_applied() -> Result<bool, String> {
    let val =
        read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?.unwrap_or(0);
    Ok(val == 1)
}

#[tauri::command]
pub fn get_power_throttling_info() -> Result<TweakInfo, String> {
    let is_applied = get_power_throttling_is_applied()?;
    let (has_backup, last_applied) = backup_info("disable_power_throttling");
    Ok(TweakInfo {
        id: "disable_power_throttling".to_string(),
        name: "Desabilitar Power Throttling".to_string(),
        description: "Impede que o Windows reduza a frequência de CPU para processos em \
            segundo plano. Útil para garantir que nenhum processo relacionado ao jogo seja \
            limitado."
            .to_string(),
        category: "energy_cpu".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description:
            "Padrão Windows: Power Throttling habilitado (PowerThrottlingOff ausente)".to_string(),
    })
}

/// Desabilita o Power Throttling escrevendo `PowerThrottlingOff = 1`.
/// Cria o caminho de registro automaticamente se não existir.
#[tauri::command]
pub fn disable_power_throttling() -> Result<(), String> {
    if get_power_throttling_is_applied()? {
        return Err("Tweak 'disable_power_throttling' já está aplicado".to_string());
    }

    let original = read_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?;

    backup_before_apply(
        "disable_power_throttling",
        TweakCategory::Registry,
        "Power Throttling — PowerThrottlingOff em HKLM\\...\\Power\\PowerThrottling",
        OriginalValue {
            path: format!("HKEY_LOCAL_MACHINE\\{}", POWER_THROTTLE_PATH),
            key: POWER_THROTTLE_KEY.to_string(),
            value: original.map(|v| json!(v)),
            value_type: "DWORD".to_string(),
        },
        json!(1),
    )?;

    write_dword(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY, 1)
}

/// Reverte o Power Throttling: remove a chave (se ausente antes) ou restaura o valor original.
#[tauri::command]
pub fn revert_power_throttling() -> Result<(), String> {
    let original = restore_from_backup("disable_power_throttling")?;

    match original.value {
        None => {
            if key_exists(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)? {
                delete_value(Hive::LocalMachine, POWER_THROTTLE_PATH, POWER_THROTTLE_KEY)?;
            }
        }
        Some(Value::Number(n)) => {
            write_dword(
                Hive::LocalMachine,
                POWER_THROTTLE_PATH,
                POWER_THROTTLE_KEY,
                n.as_u64().unwrap_or(0) as u32,
            )?;
        }
        Some(other) => {
            return Err(format!(
                "Tipo inesperado no backup de 'disable_power_throttling': {:?}",
                other
            ));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// ═══════════════════════════════════════════════════════════════════════════════
// Armazenamento — Hibernação
// ═══════════════════════════════════════════════════════════════════════════════

/// Verifica se a hibernação está habilitada checando a existência de `hiberfil.sys`.
/// Retorna `true` se a hibernação estiver ON (arquivo presente no sistema).
fn get_hibernation_status() -> bool {
    std::path::Path::new(r"C:\hiberfil.sys").exists()
}

#[tauri::command]
pub fn get_hibernation_info() -> Result<TweakInfo, String> {
    // Tweak aplicado = hibernação desabilitada = arquivo ausente
    let is_applied = !get_hibernation_status();
    let (has_backup, last_applied) = backup_info("disable_hibernation");
    Ok(TweakInfo {
        id: "disable_hibernation".to_string(),
        name: "Desabilitar Hibernação".to_string(),
        description: "Desabilita a hibernação e remove o arquivo hiberfil.sys, liberando \
            8-16 GB de espaço no disco do sistema. Também desabilita o Fast Startup, que \
            pode causar problemas de driver e estado do sistema."
            .to_string(),
        category: "storage".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: hibernação habilitada (hiberfil.sys presente)".to_string(),
    })
}

/// Desabilita a hibernação via `powercfg /h off`.
///
/// Remove `hiberfil.sys` e desabilita o Fast Startup automaticamente.
#[tauri::command]
pub fn disable_hibernation() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação
    if !get_hibernation_status() {
        return Err(
            "Tweak 'disable_hibernation' já está aplicado (hibernação já desabilitada)".to_string(),
        );
    }

    // Passo 2: Salva backup do estado original antes de qualquer modificação
    backup_before_apply(
        "disable_hibernation",
        TweakCategory::Powershell,
        "Estado da hibernação do Windows — hiberfil.sys e Fast Startup",
        OriginalValue {
            path: "powercfg".to_string(),
            key: "hibernate_state".to_string(),
            value: Some(json!("on")),
            value_type: "STATE".to_string(),
        },
        json!("off"),
    )?;

    // Passo 3: Executa powercfg /h off
    let result = run_command("powercfg.exe", &["/h", "off"])?;
    if !result.success {
        return Err(format!(
            "powercfg /h off falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

/// Reverte a hibernação para o estado original (`powercfg /h on`).
#[tauri::command]
pub fn enable_hibernation() -> Result<(), String> {
    // Recupera o backup e marca como Reverted atomicamente
    restore_from_backup("disable_hibernation")?;

    let result = run_command("powercfg.exe", &["/h", "on"])?;
    if !result.success {
        return Err(format!(
            "powercfg /h on falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Armazenamento — NTFS Last Access Timestamp
// ═══════════════════════════════════════════════════════════════════════════════

/// Consulta o valor atual de `disablelastaccess` via `fsutil behavior query`.
/// Retorna o inteiro (0–3) conforme documentação do Windows.
fn query_ntfs_last_access() -> Result<u32, String> {
    let result = run_command(
        "fsutil.exe",
        &["behavior", "query", "disablelastaccess"],
    )?;

    if !result.success {
        return Err(format!(
            "fsutil behavior query falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    // Formato esperado: "NtfsDisableLastAccessUpdate = 1  (User Managed, Disabled)"
    // Parseia o número imediatamente após o sinal "="
    let output = result.stdout.trim().to_string();
    let mut iter = output.splitn(2, '=');
    let _ = iter.next(); // descarta o label antes do "="
    if let Some(rhs) = iter.next() {
        if let Some(tok) = rhs.trim().split_whitespace().next() {
            if let Ok(n) = tok.parse::<u32>() {
                return Ok(n);
            }
        }
    }

    Err(format!(
        "Não foi possível parsear saída do fsutil: '{}'",
        output
    ))
}

#[tauri::command]
pub fn get_ntfs_last_access_info() -> Result<TweakInfo, String> {
    // Applied = valor 1 (User Managed, Disabled — definido explicitamente pelo usuário)
    let is_applied = query_ntfs_last_access().map(|v| v == 1).unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_ntfs_last_access");
    Ok(TweakInfo {
        id: "disable_ntfs_last_access".to_string(),
        name: "Desabilitar Timestamp de Último Acesso NTFS".to_string(),
        description: "Impede o NTFS de atualizar o timestamp de último acesso em cada leitura \
            de arquivo. Reduz operações de escrita no disco. No Windows 11, volumes >128GB já \
            têm isso desabilitado por padrão, mas este tweak garante a configuração \
            independente do tamanho."
            .to_string(),
        category: "storage".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description:
            "Padrão Windows: timestamps habilitados (0) ou desabilitados pelo sistema (2) em volumes grandes"
                .to_string(),
    })
}

/// Desabilita os timestamps de último acesso NTFS via `fsutil behavior set disablelastaccess 1`.
#[tauri::command]
pub fn disable_ntfs_last_access() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação
    let current_val = query_ntfs_last_access()?;
    if current_val == 1 {
        return Err(
            "Tweak 'disable_ntfs_last_access' já está aplicado (disablelastaccess = 1)".to_string(),
        );
    }

    // Passo 2: Salva backup do valor original
    backup_before_apply(
        "disable_ntfs_last_access",
        TweakCategory::Powershell,
        "NtfsDisableLastAccessUpdate — controla atualização de timestamps de leitura NTFS",
        OriginalValue {
            path: "fsutil behavior".to_string(),
            key: "disablelastaccess".to_string(),
            value: Some(json!(current_val)),
            value_type: "FSUTIL_STATE".to_string(),
        },
        json!(1u32),
    )?;

    // Passo 3: Aplica o tweak
    let result = run_command(
        "fsutil.exe",
        &["behavior", "set", "disablelastaccess", "1"],
    )?;
    if !result.success {
        return Err(format!(
            "fsutil behavior set falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

/// Reverte o timestamp de último acesso para o valor original salvo no backup.
#[tauri::command]
pub fn revert_ntfs_last_access() -> Result<(), String> {
    let original = restore_from_backup("disable_ntfs_last_access")?;

    // Extrai o valor numérico salvo no backup (0, 2 ou 3 tipicamente)
    let original_val = match original.value {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0) as u32,
        _ => 0, // Fallback: reabilita completamente (valor padrão habilitado)
    };

    let val_str = original_val.to_string();
    let result = run_command(
        "fsutil.exe",
        &["behavior", "set", "disablelastaccess", val_str.as_str()],
    )?;
    if !result.success {
        return Err(format!(
            "fsutil behavior set (reversão) falhou (código {}): {}",
            result.exit_code, result.stderr
        ));
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Rede — Algoritmo de Nagle
// ═══════════════════════════════════════════════════════════════════════════════

/// Caminho base das interfaces TCP no registro (HKLM)
const NAGLE_INTERFACES_BASE: &str =
    r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
/// Chave que controla a frequência de ACK — valor 1 = ACK imediato (sem delay)
const NAGLE_ACK_FREQ_KEY: &str = "TcpAckFrequency";
/// Chave que desabilita explicitamente o algoritmo de Nagle — valor 1 = sem agrupamento
const NAGLE_NO_DELAY_KEY: &str = "TCPNoDelay";

struct NagleStatus {
    is_applied: bool,
    guid: Option<String>,
}

/// Obtém o GUID da NIC ativa principal via PowerShell.
fn get_active_nic_guid() -> Result<String, String> {
    let result = run_powershell(
        "(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | Select-Object -First 1).InterfaceGuid",
    )?;

    let guid = result.stdout.trim().to_string();
    if guid.is_empty() {
        return Err("Nenhum adaptador de rede ativo encontrado".to_string());
    }

    Ok(guid)
}

/// Constrói o caminho de registro para as interfaces TCP da NIC informada.
fn nagle_reg_path(guid: &str) -> String {
    format!(r"{}\{}", NAGLE_INTERFACES_BASE, guid)
}

/// Verifica o estado atual do tweak de Nagle na NIC ativa.
fn get_nagle_status() -> NagleStatus {
    let guid = match get_active_nic_guid() {
        Ok(g) => g,
        Err(_) => return NagleStatus { is_applied: false, guid: None },
    };

    let path = nagle_reg_path(&guid);
    let ack_freq = read_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY)
        .unwrap_or(None)
        .unwrap_or(0);
    let no_delay = read_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY)
        .unwrap_or(None)
        .unwrap_or(0);

    NagleStatus {
        is_applied: ack_freq == 1 && no_delay == 1,
        guid: Some(guid),
    }
}

#[tauri::command]
pub fn get_nagle_info() -> Result<TweakInfo, String> {
    let status = get_nagle_status();
    let (has_backup, last_applied) = backup_info("disable_nagle");

    Ok(TweakInfo {
        id: "disable_nagle".to_string(),
        name: "Desabilitar Algoritmo de Nagle".to_string(),
        description: "Desabilita o algoritmo de Nagle e força ACK imediato em conexões TCP. \
            Pode reduzir latência em 10-20ms para jogos que usam TCP (alguns MMOs, League of \
            Legends). A maioria dos jogos modernos usa UDP, onde este tweak não tem efeito."
            .to_string(),
        category: "network".to_string(),
        is_applied: status.is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: algoritmo de Nagle habilitado".to_string(),
    })
}

/// Desabilita o algoritmo de Nagle na NIC ativa escrevendo `TcpAckFrequency = 1`
/// e `TCPNoDelay = 1` no caminho de registro da interface detectada dinamicamente.
#[tauri::command]
pub fn disable_nagle() -> Result<(), String> {
    let status = get_nagle_status();

    // Passo 1: Rejeita dupla aplicação
    if status.is_applied {
        return Err("Tweak 'disable_nagle' já está aplicado".to_string());
    }

    let guid = status
        .guid
        .ok_or("Nenhum adaptador de rede ativo encontrado para aplicar o tweak")?;
    let path = nagle_reg_path(&guid);
    let hklm_path = format!(r"HKEY_LOCAL_MACHINE\{}", path);

    // Passo 2: Lê os valores originais de ambas as chaves
    let orig_ack = read_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY)?;
    let orig_delay = read_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY)?;

    let orig_ack_json = orig_ack.map(|v| json!(v)).unwrap_or(Value::Null);
    let orig_delay_json = orig_delay.map(|v| json!(v)).unwrap_or(Value::Null);

    // Passo 3: Salva backup com padrão MULTI (dois valores na mesma NIC)
    let backup_entries = json!([
        {
            "hive": "HKLM",
            "path": hklm_path,
            "key": NAGLE_ACK_FREQ_KEY,
            "value": orig_ack_json
        },
        {
            "hive": "HKLM",
            "path": hklm_path,
            "key": NAGLE_NO_DELAY_KEY,
            "value": orig_delay_json
        }
    ]);

    backup_before_apply(
        "disable_nagle",
        TweakCategory::Registry,
        "TcpAckFrequency e TCPNoDelay na NIC ativa — desabilita algoritmo de Nagle",
        OriginalValue {
            path: "MULTI".to_string(),
            key: "nagle_keys".to_string(),
            value: Some(backup_entries),
            value_type: "MULTI_DWORD".to_string(),
        },
        json!([1, 1]),
    )?;

    // Passo 4: Escreve ambas as chaves
    write_dword(Hive::LocalMachine, &path, NAGLE_ACK_FREQ_KEY, 1)?;
    write_dword(Hive::LocalMachine, &path, NAGLE_NO_DELAY_KEY, 1)?;

    Ok(())
}

/// Reverte o algoritmo de Nagle restaurando os valores originais das duas chaves de registro.
#[tauri::command]
pub fn revert_nagle() -> Result<(), String> {
    let original = restore_from_backup("disable_nagle")?;

    let entries = match original.value {
        Some(Value::Array(arr)) => arr,
        _ => return Err("Formato de backup de Nagle inválido — esperado array MULTI".to_string()),
    };

    for entry in &entries {
        let path_full = entry["path"]
            .as_str()
            .ok_or("Backup Nagle: campo 'path' ausente ou inválido")?;
        let key = entry["key"]
            .as_str()
            .ok_or("Backup Nagle: campo 'key' ausente ou inválido")?;

        // Remove o prefixo "HKEY_LOCAL_MACHINE\" para uso com Hive::LocalMachine
        let reg_path = path_full
            .strip_prefix(r"HKEY_LOCAL_MACHINE\")
            .unwrap_or(path_full);

        match &entry["value"] {
            Value::Null => {
                // Chave não existia — remove para restaurar padrão implícito
                if key_exists(Hive::LocalMachine, reg_path, key)? {
                    delete_value(Hive::LocalMachine, reg_path, key)?;
                }
            }
            Value::Number(n) => {
                let v = n.as_u64().unwrap_or(0) as u32;
                write_dword(Hive::LocalMachine, reg_path, key, v)?;
            }
            other => {
                return Err(format!(
                    "Tipo inesperado no backup de Nagle para chave '{}': {:?}",
                    key, other
                ));
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Visual — Sticky Keys (Teclas de Aderência)
// ═══════════════════════════════════════════════════════════════════════════════

/// Caminho do registro de acessibilidade no perfil do usuário atual
const STICKY_KEYS_PATH: &str = r"Control Panel\Accessibility\StickyKeys";
/// Nome do valor de flags que controla o comportamento do atalho
const STICKY_KEYS_FLAGS_KEY: &str = "Flags";
/// Valor que desabilita o atalho de 5x Shift (bit 1 em 0)
const STICKY_KEYS_APPLIED_FLAGS: &str = "506";

/// Verifica se o atalho de Sticky Keys está desabilitado (Flags = "506").
fn get_sticky_keys_status() -> Result<bool, String> {
    let flags = read_string(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
    Ok(flags.as_deref() == Some(STICKY_KEYS_APPLIED_FLAGS))
}

#[tauri::command]
pub fn get_sticky_keys_info() -> Result<TweakInfo, String> {
    let is_applied = get_sticky_keys_status().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_sticky_keys");

    Ok(TweakInfo {
        id: "disable_sticky_keys".to_string(),
        name: "Desabilitar Teclas de Aderência (Sticky Keys)".to_string(),
        description: "Desabilita o atalho de ativação do Sticky Keys (5x Shift), prevenindo \
            interrupções acidentais durante sessões de jogo."
            .to_string(),
        category: "visual".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: atalho de Sticky Keys habilitado (Flags = 510)".to_string(),
    })
}

/// Define `Flags = "506"` em `HKCU\Control Panel\Accessibility\StickyKeys`
/// para desabilitar o atalho de ativação por 5x Shift.
#[tauri::command]
pub fn disable_sticky_keys() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação
    if get_sticky_keys_status()? {
        return Err(
            "Tweak 'disable_sticky_keys' já está aplicado (Flags = 506)".to_string(),
        );
    }

    // Passo 2: Lê o valor original das Flags
    let original_flags =
        read_string(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
    let original_json = original_flags.as_ref().map(|v| json!(v));

    // Passo 3: Salva backup antes de modificar
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

    // Passo 4: Aplica o tweak — escreve "506" no registro
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
            // Chave não existia — remove para restaurar comportamento padrão
            if key_exists(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)? {
                delete_value(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY)?;
            }
        }
        Some(Value::String(s)) => {
            write_string(Hive::CurrentUser, STICKY_KEYS_PATH, STICKY_KEYS_FLAGS_KEY, &s)?;
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
// Visual — Busca Bing no Menu Iniciar
// ═══════════════════════════════════════════════════════════════════════════════

/// Caminho do registro da configuração de busca do Windows (HKCU)
const BING_SEARCH_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";
/// Chave que controla a integração Bing na pesquisa do Menu Iniciar
const BING_SEARCH_KEY: &str = "BingSearchEnabled";

/// Verifica se o Bing está desabilitado no Menu Iniciar (BingSearchEnabled = 0).
fn get_bing_search_status() -> Result<bool, String> {
    let val = read_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)?;
    // Applied = 0 (desabilitado); None = chave ausente (Bing habilitado por padrão)
    Ok(val == Some(0))
}

#[tauri::command]
pub fn get_bing_search_info() -> Result<TweakInfo, String> {
    let is_applied = get_bing_search_status().unwrap_or(false);
    let (has_backup, last_applied) = backup_info("disable_bing_search");

    Ok(TweakInfo {
        id: "disable_bing_search".to_string(),
        name: "Desabilitar Busca Bing no Menu Iniciar".to_string(),
        description: "Remove a integração do Bing no menu Iniciar do Windows. Buscas ficam \
            apenas locais, mais rápidas e sem envio de dados para a Microsoft."
            .to_string(),
        category: "visual".to_string(),
        is_applied,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description:
            "Padrão Windows: busca Bing habilitada no Menu Iniciar (chave ausente ou = 1)"
                .to_string(),
    })
}

/// Define `BingSearchEnabled = 0` em `HKCU\...\Search` para remover integração Bing.
#[tauri::command]
pub fn disable_bing_search() -> Result<(), String> {
    // Passo 1: Rejeita dupla aplicação
    if get_bing_search_status()? {
        return Err(
            "Tweak 'disable_bing_search' já está aplicado (BingSearchEnabled = 0)".to_string(),
        );
    }

    // Passo 2: Lê o valor original
    let original_val = read_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY)?;
    let original_json = original_val.map(|v| json!(v));

    // Passo 3: Salva backup
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

    // Passo 4: Desabilita o Bing no Menu Iniciar
    write_dword(Hive::CurrentUser, BING_SEARCH_PATH, BING_SEARCH_KEY, 0)
}

/// Reverte o Bing no Menu Iniciar para o estado original salvo no backup.
#[tauri::command]
pub fn revert_bing_search() -> Result<(), String> {
    let original = restore_from_backup("disable_bing_search")?;

    match original.value {
        // Chave não existia — remove (Bing volta habilitado pelo comportamento padrão)
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
