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
use crate::utils::command_runner::{run_command, run_command_with_progress};
use crate::utils::registry::{delete_value, key_exists, read_dword, write_dword, Hive};

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
fn backup_info(tweak_id: &str) -> (bool, Option<String>) {
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
