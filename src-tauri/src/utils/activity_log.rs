//! Log de Atividade Recente do FrameGuard.
//!
//! Registra ações executadas pelo app (planos e tweaks individuais)
//! em `%APPDATA%\FrameGuard\activity_log.json` para exibição na
//! seção "Atividade Recente" do Dashboard.
//!
//! Mantém no máximo `MAX_ENTRIES` registros (FIFO) para evitar
//! crescimento indefinido do arquivo.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::{fs, path::PathBuf};

// ─── Constantes ──────────────────────────────────────────────────────────────

/// Máximo de entradas no log. Entradas mais antigas são descartadas (FIFO).
const MAX_ENTRIES: usize = 100;

// ─── Tipos públicos ─────────────────────────────────────────────────────────

/// Tipo de atividade registrada.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    /// Execução completa de um plano de execução
    PlanExecution,
    /// Aplicação individual de um tweak/otimização
    TweakApplied,
    /// Reversão individual de um tweak/otimização
    TweakReverted,
}

/// Resultado geral da atividade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityResult {
    /// Todos os itens concluídos com sucesso
    Success,
    /// Plano concluído com alguns itens falhando
    Partial,
    /// Ação falhou completamente
    Failed,
}

/// Uma entrada no log de atividade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Timestamp ISO 8601 UTC da atividade
    pub timestamp: String,
    /// Tipo da atividade
    pub activity_type: ActivityType,
    /// Nome legível (ex: "Saúde Completa", "Desabilitar VBS")
    pub name: String,
    /// Resultado geral
    pub result: ActivityResult,
    /// Duração em segundos (0 para tweaks instantâneos)
    pub duration_seconds: u64,
    /// Contadores de itens (apenas para planos; `None` para tweaks)
    pub completed_count: Option<usize>,
    pub failed_count: Option<usize>,
    pub skipped_count: Option<usize>,
}

/// Estrutura raiz de `activity_log.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivityLog {
    /// Entradas ordenadas da mais recente para a mais antiga
    entries: Vec<ActivityEntry>,
}

impl ActivityLog {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

// ─── Estado global (thread-safe) ────────────────────────────────────────────

static STATE: OnceLock<Mutex<ActivityLog>> = OnceLock::new();

fn get_state() -> &'static Mutex<ActivityLog> {
    STATE.get_or_init(|| {
        let data = load_from_disk().unwrap_or_else(|_| ActivityLog::new());
        Mutex::new(data)
    })
}

// ─── I/O de arquivo ─────────────────────────────────────────────────────────

fn log_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "Variável de ambiente APPDATA não encontrada".to_string())?;

    let dir = PathBuf::from(appdata).join("FrameGuard");

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Não foi possível criar o diretório FrameGuard: {}", e))?;
    }

    Ok(dir.join("activity_log.json"))
}

fn load_from_disk() -> Result<ActivityLog, String> {
    let path = log_path()?;

    if !path.exists() {
        return Ok(ActivityLog::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Erro ao ler activity_log.json: {}", e))?;

    serde_json::from_str(&contents)
        .map_err(|e| format!("Arquivo activity_log.json inválido: {}", e))
}

fn save_to_disk(state: &ActivityLog) -> Result<(), String> {
    let path = log_path()?;

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Erro ao serializar activity log: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Erro ao salvar activity_log.json: {}", e))
}

// ─── API pública ────────────────────────────────────────────────────────────

/// Registra uma nova atividade no log.
///
/// A entrada é inserida no início (mais recente primeiro).
/// Se o log exceder `MAX_ENTRIES`, as entradas mais antigas são descartadas.
pub fn log_activity(entry: ActivityEntry) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no activity log".to_string())?;

    state.entries.insert(0, entry);

    // Mantém o tamanho dentro do limite
    state.entries.truncate(MAX_ENTRIES);

    save_to_disk(&state)
}

/// Retorna as `limit` atividades mais recentes.
pub fn get_recent(limit: u32) -> Result<Vec<ActivityEntry>, String> {
    let state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no activity log".to_string())?;

    let n = (limit as usize).min(state.entries.len());
    Ok(state.entries[..n].to_vec())
}

// ─── Helpers de criação ─────────────────────────────────────────────────────

/// Cria uma entrada de log para execução de plano (chamado pelo executor de planos).
pub fn plan_execution_entry(
    plan_name: &str,
    duration_seconds: u64,
    completed: usize,
    failed: usize,
    skipped: usize,
) -> ActivityEntry {
    let result = if failed == 0 {
        ActivityResult::Success
    } else if completed > 0 {
        ActivityResult::Partial
    } else {
        ActivityResult::Failed
    };

    ActivityEntry {
        timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        activity_type: ActivityType::PlanExecution,
        name: plan_name.to_string(),
        result,
        duration_seconds,
        completed_count: Some(completed),
        failed_count: Some(failed),
        skipped_count: Some(skipped),
    }
}

/// Cria uma entrada de log para tweak individual (apply ou revert).
pub fn tweak_entry(
    name: &str,
    applied: bool,
    success: bool,
) -> ActivityEntry {
    ActivityEntry {
        timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        activity_type: if applied {
            ActivityType::TweakApplied
        } else {
            ActivityType::TweakReverted
        },
        name: name.to_string(),
        result: if success {
            ActivityResult::Success
        } else {
            ActivityResult::Failed
        },
        duration_seconds: 0,
        completed_count: None,
        failed_count: None,
        skipped_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── plan_execution_entry ────────────────────────────────────────────────

    #[test]
    fn all_completed_is_success() {
        let entry = plan_execution_entry("Manutenção", 30, 5, 0, 0);
        assert!(matches!(entry.result, ActivityResult::Success));
        assert!(matches!(entry.activity_type, ActivityType::PlanExecution));
        assert_eq!(entry.name, "Manutenção");
        assert_eq!(entry.duration_seconds, 30);
        assert_eq!(entry.completed_count, Some(5));
        assert_eq!(entry.failed_count, Some(0));
    }

    #[test]
    fn some_failed_is_partial() {
        let entry = plan_execution_entry("Gaming", 60, 3, 2, 0);
        assert!(matches!(entry.result, ActivityResult::Partial));
    }

    #[test]
    fn all_failed_is_failed() {
        let entry = plan_execution_entry("Privacidade", 10, 0, 4, 0);
        assert!(matches!(entry.result, ActivityResult::Failed));
    }

    #[test]
    fn zero_completed_with_skipped_is_failed() {
        let entry = plan_execution_entry("Teste", 5, 0, 1, 3);
        assert!(matches!(entry.result, ActivityResult::Failed));
    }

    // ── tweak_entry ─────────────────────────────────────────────────────────

    #[test]
    fn apply_success() {
        let entry = tweak_entry("Desabilitar VBS", true, true);
        assert!(matches!(entry.activity_type, ActivityType::TweakApplied));
        assert!(matches!(entry.result, ActivityResult::Success));
        assert_eq!(entry.name, "Desabilitar VBS");
        assert_eq!(entry.duration_seconds, 0);
        assert!(entry.completed_count.is_none());
    }

    #[test]
    fn revert_failure() {
        let entry = tweak_entry("Desabilitar VBS", false, false);
        assert!(matches!(entry.activity_type, ActivityType::TweakReverted));
        assert!(matches!(entry.result, ActivityResult::Failed));
    }

    #[test]
    fn timestamp_is_iso_format() {
        let entry = tweak_entry("Test", true, true);
        // Formato esperado: YYYY-MM-DDTHH:MM:SSZ
        assert!(entry.timestamp.ends_with('Z'));
        assert_eq!(entry.timestamp.len(), 20);
        assert_eq!(&entry.timestamp[4..5], "-");
        assert_eq!(&entry.timestamp[10..11], "T");
    }
}
