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
// PLACEHOLDERS — GPU e Display
//
// Implementação real prevista para próxima fase de desenvolvimento.
// get_X_info devolve TweakInfo com is_applied = false (estado não lido).
// Comandos de aplicar/reverter retornam erro informativo.
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_game_dvr_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_game_dvr");
    Ok(TweakInfo {
        id: "disable_game_dvr".to_string(),
        name: "Desabilitar Game DVR / Captura Game Bar".to_string(),
        description: "Desabilita a gravação em segundo plano do Xbox Game Bar, que mantém \
            um buffer circular de vídeo consumindo GPU encode, RAM e CPU continuamente."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description: "Padrão Windows: Game DVR habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_game_dvr() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_game_dvr() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_xbox_overlay_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_xbox_overlay");
    Ok(TweakInfo {
        id: "disable_xbox_overlay".to_string(),
        name: "Desabilitar Xbox Game Bar".to_string(),
        description: "Desabilita completamente o overlay do Xbox Game Bar, liberando recursos \
            usados pelo processo GameBarPresenceWriter em segundo plano."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description: "Padrão Windows: Xbox Game Bar habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_xbox_overlay() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_xbox_overlay() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_msi_mode_gpu_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("enable_msi_mode_gpu");
    Ok(TweakInfo {
        id: "enable_msi_mode_gpu".to_string(),
        name: "Ativar MSI Mode para GPU".to_string(),
        description: "Configura a GPU para usar Message Signaled Interrupts (MSI) em vez de \
            interrupts baseados em linha, reduzindo latência de interrupção e possíveis \
            microstutters."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied: false,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: MSI Mode desabilitado para GPU".to_string(),
    })
}

#[tauri::command]
pub fn enable_msi_mode_gpu() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn disable_msi_mode_gpu() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_mpo_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_mpo");
    Ok(TweakInfo {
        id: "disable_mpo".to_string(),
        name: "Desabilitar Multiplane Overlay (MPO)".to_string(),
        description: "Desabilita o Multiplane Overlay do DirectX, que pode causar stuttering \
            e artefatos visuais em alguns sistemas com múltiplos monitores ou janelas sobrepostas."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied: false,
        requires_restart: true,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: MPO habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_mpo() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_mpo() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_nvidia_telemetry_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_nvidia_telemetry");
    Ok(TweakInfo {
        id: "disable_nvidia_telemetry".to_string(),
        name: "Desabilitar Telemetria NVIDIA".to_string(),
        description: "Desabilita os serviços de telemetria e coleta de dados do driver NVIDIA \
            que rodam em segundo plano, liberando CPU e memória desnecessariamente consumidos."
            .to_string(),
        category: "gpu_display".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Unproven,
        default_value_description: "Padrão NVIDIA: telemetria habilitada".to_string(),
    })
}

#[tauri::command]
pub fn disable_nvidia_telemetry() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_nvidia_telemetry() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// PLACEHOLDERS — Gaming
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_timer_resolution_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("enable_timer_resolution");
    Ok(TweakInfo {
        id: "enable_timer_resolution".to_string(),
        name: "Timer de Alta Resolução (1 ms)".to_string(),
        description: "Configura o timer do sistema Windows para 1 ms de resolução em vez dos \
            15,6 ms padrão, reduzindo latência de entrada e melhorando consistência de framerates."
            .to_string(),
        category: "gaming".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: timer de 15,6 ms".to_string(),
    })
}

#[tauri::command]
pub fn enable_timer_resolution() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn disable_timer_resolution() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_mouse_acceleration_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_mouse_acceleration");
    Ok(TweakInfo {
        id: "disable_mouse_acceleration".to_string(),
        name: "Desabilitar Aceleração do Mouse".to_string(),
        description: "Remove a curva de aceleração do ponteiro (Enhanced Pointer Precision), \
            tornando o movimento do mouse linear e previsível — essencial para jogos FPS."
            .to_string(),
        category: "gaming".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: Enhanced Pointer Precision habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_mouse_acceleration() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_mouse_acceleration() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_fullscreen_optimizations_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_fullscreen_optimizations");
    Ok(TweakInfo {
        id: "disable_fullscreen_optimizations".to_string(),
        name: "Desabilitar Otimizações de Tela Cheia".to_string(),
        description: "Remove as 'Otimizações de Tela Cheia' do Windows, que convertem janelas \
            exclusivas em borderless window. Em alguns jogos melhora latência; em outros pode \
            piorar — resultados variam por jogo e GPU."
            .to_string(),
        category: "gaming".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: Otimizações de Tela Cheia habilitadas".to_string(),
    })
}

#[tauri::command]
pub fn disable_fullscreen_optimizations() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_fullscreen_optimizations() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// PLACEHOLDERS — Energia e CPU
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_ultimate_performance_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("enable_ultimate_performance");
    Ok(TweakInfo {
        id: "enable_ultimate_performance".to_string(),
        name: "Plano de Energia: Desempenho Máximo".to_string(),
        description: "Ativa o plano de energia 'Desempenho Máximo' oculto do Windows, que \
            elimina micro-sleep states do processador e prioriza desempenho bruto sobre \
            eficiência energética."
            .to_string(),
        category: "energy_cpu".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: plano Balanceado ou Alto Desempenho".to_string(),
    })
}

#[tauri::command]
pub fn enable_ultimate_performance() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_ultimate_performance() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_power_throttling_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_power_throttling");
    Ok(TweakInfo {
        id: "disable_power_throttling".to_string(),
        name: "Desabilitar Power Throttling".to_string(),
        description: "Desabilita o Power Throttling do Windows, que limita automaticamente \
            recursos de CPU para processos em segundo plano. Com este tweak, todos os processos \
            recebem CPU sem limitação artificial."
            .to_string(),
        category: "energy_cpu".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: Power Throttling habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_power_throttling() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_power_throttling() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// PLACEHOLDERS — Armazenamento
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_hibernation_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_hibernation");
    Ok(TweakInfo {
        id: "disable_hibernation".to_string(),
        name: "Desabilitar Hibernação".to_string(),
        description: "Desabilita a hibernação do Windows, removendo o arquivo hiberfil.sys \
            (geralmente 40–70% da RAM em tamanho). Libera espaço significativo em SSDs."
            .to_string(),
        category: "storage".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: hibernação habilitada (hiberfil.sys presente)".to_string(),
    })
}

#[tauri::command]
pub fn disable_hibernation() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn enable_hibernation() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_ntfs_last_access_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_ntfs_last_access");
    Ok(TweakInfo {
        id: "disable_ntfs_last_access".to_string(),
        name: "Desabilitar Timestamp de Último Acesso NTFS".to_string(),
        description: "Desabilita a atualização automática do timestamp de último acesso em \
            arquivos NTFS, reduzindo escritas desnecessárias no disco — especialmente benéfico \
            para SSDs com muitas operações de leitura."
            .to_string(),
        category: "storage".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: timestamps de último acesso habilitados".to_string(),
    })
}

#[tauri::command]
pub fn disable_ntfs_last_access() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_ntfs_last_access() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// PLACEHOLDER — Rede
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_nagle_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_nagle");
    Ok(TweakInfo {
        id: "disable_nagle".to_string(),
        name: "Desabilitar Algoritmo de Nagle".to_string(),
        description: "Desabilita o algoritmo de Nagle nas conexões TCP, que agrupa pacotes \
            pequenos para otimizar largura de banda. Em jogos com protocolo TCP, pode reduzir \
            latência às custas de maior uso de banda."
            .to_string(),
        category: "network".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Medium,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: algoritmo de Nagle habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_nagle() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_nagle() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// PLACEHOLDERS — Visual e Experiência
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_sticky_keys_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_sticky_keys");
    Ok(TweakInfo {
        id: "disable_sticky_keys".to_string(),
        name: "Desabilitar Teclas de Aderência (Sticky Keys)".to_string(),
        description: "Desabilita o atalho que ativa Sticky Keys ao pressionar Shift 5 vezes, \
            eliminando a janela de diálogo que pode interromper sessões de jogo inesperadamente."
            .to_string(),
        category: "visual".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Proven,
        default_value_description: "Padrão Windows: atalho de Sticky Keys habilitado".to_string(),
    })
}

#[tauri::command]
pub fn disable_sticky_keys() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_sticky_keys() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn get_bing_search_info() -> Result<TweakInfo, String> {
    let (has_backup, last_applied) = backup_info("disable_bing_search");
    Ok(TweakInfo {
        id: "disable_bing_search".to_string(),
        name: "Desabilitar Busca Bing no Menu Iniciar".to_string(),
        description: "Desabilita a integração do Bing no Menu Iniciar, evitando requisições de \
            rede ao pesquisar localmente e acelerando resultados de busca."
            .to_string(),
        category: "visual".to_string(),
        is_applied: false,
        requires_restart: false,
        last_applied,
        has_backup,
        risk_level: RiskLevel::Low,
        evidence_level: EvidenceLevel::Plausible,
        default_value_description: "Padrão Windows: busca Bing habilitada no Menu Iniciar".to_string(),
    })
}

#[tauri::command]
pub fn disable_bing_search() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}

#[tauri::command]
pub fn revert_bing_search() -> Result<(), String> {
    Err("Não implementado — próxima fase de desenvolvimento".to_string())
}
