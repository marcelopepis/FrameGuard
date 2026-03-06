//! Sistema de backup de configurações do FrameGuard.
//!
//! Persiste backups em `%APPDATA%\FrameGuard\backups.json` antes de cada tweak
//! aplicado, permitindo reversão segura para o estado original do sistema.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::{fs, path::PathBuf};

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Categoria do tweak — determina como a reversão deve ser tratada
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TweakCategory {
    /// Modificação direta no registro do Windows
    Registry,
    /// Comando DISM (ex: desabilitar recursos do Windows)
    Dism,
    /// Script PowerShell
    Powershell,
    /// Operação de limpeza de arquivos/pastas
    Cleanup,
}

/// Estado atual do tweak no sistema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    /// Tweak foi aplicado e está ativo no sistema
    Applied,
    /// Tweak foi revertido; sistema voltou ao estado original
    Reverted,
}

/// Valor original de uma entrada de registro antes da modificação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalValue {
    /// Caminho completo da chave (ex: `"HKEY_CURRENT_USER\\Control Panel\\Desktop"`)
    pub path: String,
    /// Nome do valor dentro da chave (ex: `"JPEGImportQuality"`)
    pub key: String,
    /// Conteúdo anterior — `null` se a chave não existia antes do tweak
    pub value: Option<Value>,
    /// Tipo do valor no registro: `"DWORD"`, `"STRING"`, `"QWORD"`, `"BINARY"`, etc.
    pub value_type: String,
}

/// Entrada de backup de um tweak específico
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub category: TweakCategory,
    pub description: String,
    pub original_value: OriginalValue,
    /// Timestamp ISO 8601 UTC de criação do backup
    pub backed_up_at: String,
    /// Valor que foi escrito pelo tweak (para referência e auditoria)
    pub applied_value: Value,
    pub status: BackupStatus,
}

/// Estrutura completa do arquivo `backups.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub version: String,
    pub created_at: String,
    pub last_modified: String,
    pub backups: HashMap<String, BackupEntry>,
}

impl BackupFile {
    fn new() -> Self {
        let now = now_utc();
        Self {
            version: "1.0".to_string(),
            created_at: now.clone(),
            last_modified: now,
            backups: HashMap::new(),
        }
    }
}

// ─── Estado global (thread-safe) ─────────────────────────────────────────────

/// Cache em memória do arquivo de backups, protegido por `Mutex`.
/// `OnceLock` garante inicialização lazy e única — o arquivo é lido do disco
/// apenas na primeira chamada a qualquer função pública deste módulo.
static STATE: OnceLock<Mutex<BackupFile>> = OnceLock::new();

/// Retorna referência ao Mutex global, inicializando a partir do disco se necessário.
fn get_state() -> &'static Mutex<BackupFile> {
    STATE.get_or_init(|| {
        let data = load_from_disk().unwrap_or_else(|_| BackupFile::new());
        Mutex::new(data)
    })
}

// ─── I/O de arquivo ───────────────────────────────────────────────────────────

/// Retorna o caminho absoluto de `backups.json`, criando o diretório se necessário.
/// Localização: `%APPDATA%\FrameGuard\backups.json`
fn backup_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "Variável de ambiente APPDATA não encontrada".to_string())?;

    let dir = PathBuf::from(appdata).join("FrameGuard");

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Não foi possível criar o diretório de backups: {}", e))?;
    }

    Ok(dir.join("backups.json"))
}

/// Carrega `backups.json` do disco para memória.
/// Retorna um `BackupFile` vazio e válido se o arquivo ainda não existir.
fn load_from_disk() -> Result<BackupFile, String> {
    let path = backup_path()?;

    if !path.exists() {
        return Ok(BackupFile::new());
    }

    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Erro ao ler backups.json: {}", e))?;

    serde_json::from_str(&contents)
        .map_err(|e| format!("Arquivo backups.json inválido ou corrompido: {}", e))
}

/// Persiste o estado em memória no `backups.json` do disco.
/// Usa `to_string_pretty` para que o arquivo seja legível por humanos.
fn save_to_disk(state: &BackupFile) -> Result<(), String> {
    let path = backup_path()?;

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Erro ao serializar backups: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Erro ao salvar backups.json: {}", e))
}

// ─── Utilitário de timestamp ──────────────────────────────────────────────────

/// Retorna o instante atual em ISO 8601 UTC. Ex: `"2025-01-15T10:30:45Z"`
fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ─── API pública ──────────────────────────────────────────────────────────────

/// Salva o estado original de um tweak antes de aplicá-lo.
///
/// Deve ser chamado **antes** de qualquer modificação no sistema para garantir
/// que o valor original esteja preservado para uma reversão futura.
///
/// Se já existir um backup com status `Applied` para o mesmo `tweak_id`,
/// atualiza apenas o timestamp e o `applied_value`, preservando o
/// `original_value` verdadeiro. Isso permite reaplicar tweaks que o Windows
/// reverteu silenciosamente sem perder a referência para reversão.
///
/// # Parâmetros
/// - `tweak_id`: identificador único do tweak (ex: `"disable_vbs"`)
/// - `category`: tipo do tweak, define como a reversão deve ocorrer
/// - `description`: texto legível descrevendo o que o tweak altera
/// - `original_value`: estado atual do sistema antes de qualquer mudança
/// - `applied_value`: valor que será escrito pelo tweak (para auditoria)
pub fn backup_before_apply(
    tweak_id: &str,
    category: TweakCategory,
    description: &str,
    original_value: OriginalValue,
    applied_value: Value,
) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    // Protege o original verdadeiro: não sobrescreve backup ativo existente.
    // Se o tweak já tem backup Applied, apenas atualiza o timestamp —
    // isso permite reaplicar tweaks que o Windows reverteu silenciosamente.
    if let Some(existing) = state.backups.get(tweak_id) {
        if existing.status == BackupStatus::Applied {
            let mut updated = existing.clone();
            updated.backed_up_at = now_utc();
            updated.applied_value = applied_value;
            state.backups.insert(tweak_id.to_string(), updated);
            state.last_modified = now_utc();
            save_to_disk(&state)?;
            return Ok(());
        }
    }

    let entry = BackupEntry {
        category,
        description: description.to_string(),
        original_value,
        backed_up_at: now_utc(),
        applied_value,
        status: BackupStatus::Applied,
    };

    state.backups.insert(tweak_id.to_string(), entry);
    state.last_modified = now_utc();
    save_to_disk(&state)
}

/// Recupera o valor original de um tweak para uso na reversão.
///
/// Marca automaticamente o backup como `Reverted` e persiste no disco.
/// O chamador é responsável por de fato escrever o valor original de volta
/// ao sistema (registro, DISM, etc.) após receber o retorno.
///
/// Retorna erro se o backup não existir ou já tiver sido revertido anteriormente.
///
/// # Retorna
/// O `OriginalValue` salvo, contendo path, key e conteúdo original da chave.
pub fn restore_from_backup(tweak_id: &str) -> Result<OriginalValue, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    let entry = state
        .backups
        .get_mut(tweak_id)
        .ok_or_else(|| format!("Backup para '{}' não encontrado", tweak_id))?;

    if entry.status == BackupStatus::Reverted {
        return Err(format!(
            "Tweak '{}' já foi revertido — backup já utilizado",
            tweak_id
        ));
    }

    let original = entry.original_value.clone();
    entry.status = BackupStatus::Reverted;
    state.last_modified = now_utc();
    save_to_disk(&state)?;

    Ok(original)
}

/// Verifica se existe um backup para o tweak informado e retorna o status atual.
///
/// # Retorna
/// - `Ok(None)` — nenhum backup registrado para este `tweak_id`
/// - `Ok(Some(Applied))` — backup existe e o tweak está ativo no sistema
/// - `Ok(Some(Reverted))` — backup existe, mas o tweak já foi revertido
#[allow(dead_code)]
pub fn get_backup_status(tweak_id: &str) -> Result<Option<BackupStatus>, String> {
    let state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    Ok(state.backups.get(tweak_id).map(|e| e.status.clone()))
}

/// Retorna todos os backups registrados, para listagem na UI ou auditoria.
///
/// Retorna um clone do `HashMap` interno para evitar manter o lock durante
/// processamento subsequente pelo chamador.
pub fn get_all_backups() -> Result<HashMap<String, BackupEntry>, String> {
    let state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    Ok(state.backups.clone())
}

/// Substitui completamente o estado de backups pelo conteúdo fornecido.
///
/// Usado pela importação de configurações (`import_config` no modo `"replace"`).
/// Persiste no disco e atualiza o cache em memória atomicamente, garantindo
/// consistência imediata sem necessidade de reiniciar o aplicativo.
pub fn replace_all_backups(new_state: BackupFile) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    save_to_disk(&new_state)?;
    *state = new_state;
    Ok(())
}

/// Mescla entradas de backup com o estado atual, substituindo as existentes.
///
/// Usado pela importação de configurações (`import_config` no modo `"merge"`).
/// Todas as entradas do mapa fornecido são inseridas — se a chave já existir,
/// a entrada atual é sobrescrita pela importada.
///
/// # Retorna
/// A quantidade de entradas inseridas ou atualizadas.
pub fn merge_backups(entries: HashMap<String, BackupEntry>) -> Result<usize, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    let count = entries.len();

    for (key, entry) in entries {
        state.backups.insert(key, entry);
    }

    if count > 0 {
        state.last_modified = now_utc();
        save_to_disk(&state)?;
    }

    Ok(count)
}

/// Remove permanentemente o backup de um tweak específico do arquivo.
///
/// Use após confirmar que a reversão foi bem-sucedida e o backup não é mais
/// necessário, ou para limpeza de registros obsoletos.
/// Retorna erro se o `tweak_id` não existir nos backups.
#[allow(dead_code)]
pub fn delete_backup(tweak_id: &str) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de backups".to_string())?;

    state
        .backups
        .remove(tweak_id)
        .ok_or_else(|| format!("Backup para '{}' não encontrado para remoção", tweak_id))?;

    state.last_modified = now_utc();
    save_to_disk(&state)
}
