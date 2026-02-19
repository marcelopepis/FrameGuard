//! Gerenciamento de Planos de Execução do FrameGuard.
//!
//! Persiste os planos em `%APPDATA%\FrameGuard\plans.json`.
//! Cada plano é uma lista ordenada de tweaks/ações que o usuário
//! monta para execução com um clique.
//!
//! O estado em memória é protegido por `OnceLock<Mutex<PlansFile>>`,
//! garantindo inicialização lazy e acesso thread-safe — mesmo padrão
//! de `backup.rs`.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::{fs, path::PathBuf};
use uuid::Uuid;

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Um item dentro de um plano: aponta para um tweak/ação e define sua posição.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    /// ID do tweak ou ação (ex: `"temp_cleanup"`, `"dism_checkhealth"`)
    pub tweak_id: String,
    /// Posição na sequência de execução — menor = executa primeiro
    pub order: u32,
    /// `false` pula este item durante a execução sem removê-lo do plano
    pub enabled: bool,
}

/// Um Plano de Execução: coleção nomeada de itens para rodar em sequência.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// UUID v4 gerado na criação — identificador imutável do plano
    pub id: String,
    /// Nome legível definido pelo usuário (ex: `"Manutenção Semanal"`)
    pub name: String,
    /// Descrição opcional do objetivo do plano
    pub description: String,
    /// Timestamp ISO 8601 UTC de criação
    pub created_at: String,
    /// Timestamp ISO 8601 UTC da última execução completa; `null` se nunca executado
    pub last_executed: Option<String>,
    /// Lista de itens ordenados por `order` ascendente
    pub items: Vec<PlanItem>,
}

/// Estrutura raiz de `plans.json` — envolve todos os planos com metadados de versão.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlansFile {
    pub version: String,
    pub created_at: String,
    pub last_modified: String,
    /// Planos indexados por ID para busca O(1) na persistência
    pub plans: HashMap<String, Plan>,
}

impl PlansFile {
    fn new() -> Self {
        let now = now_utc();
        Self {
            version: "1.0".to_string(),
            created_at: now.clone(),
            last_modified: now,
            plans: HashMap::new(),
        }
    }
}

// ─── Estado global (thread-safe) ─────────────────────────────────────────────

/// Cache em memória de `plans.json`, inicializado na primeira chamada.
static STATE: OnceLock<Mutex<PlansFile>> = OnceLock::new();

/// Retorna referência ao Mutex global, carregando o arquivo do disco se necessário.
fn get_state() -> &'static Mutex<PlansFile> {
    STATE.get_or_init(|| {
        let data = load_from_disk().unwrap_or_else(|_| PlansFile::new());
        Mutex::new(data)
    })
}

// ─── I/O de arquivo ───────────────────────────────────────────────────────────

/// Retorna o caminho absoluto de `plans.json`, criando o diretório se necessário.
fn plans_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "Variável de ambiente APPDATA não encontrada".to_string())?;

    let dir = PathBuf::from(appdata).join("FrameGuard");

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Não foi possível criar o diretório FrameGuard: {}", e))?;
    }

    Ok(dir.join("plans.json"))
}

/// Carrega `plans.json` do disco. Retorna arquivo vazio e válido se ainda não existir.
fn load_from_disk() -> Result<PlansFile, String> {
    let path = plans_path()?;

    if !path.exists() {
        return Ok(PlansFile::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Erro ao ler plans.json: {}", e))?;

    serde_json::from_str(&contents)
        .map_err(|e| format!("Arquivo plans.json inválido ou corrompido: {}", e))
}

/// Persiste o estado em memória em `plans.json` (formato indentado, legível por humanos).
fn save_to_disk(state: &PlansFile) -> Result<(), String> {
    let path = plans_path()?;

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Erro ao serializar planos: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Erro ao salvar plans.json: {}", e))
}

// ─── Utilitário ───────────────────────────────────────────────────────────────

/// Retorna o instante atual em ISO 8601 UTC.
fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ─── API pública ──────────────────────────────────────────────────────────────

/// Cria um novo plano com UUID v4 gerado automaticamente e o persiste no disco.
///
/// Os itens são armazenados como fornecidos — a ordenação por `order` é
/// responsabilidade do chamador (feita no executor de planos em `plans.rs`).
///
/// # Retorna
/// O `Plan` criado com `id` e `created_at` preenchidos.
pub fn create_plan(
    name: &str,
    description: &str,
    items: Vec<PlanItem>,
) -> Result<Plan, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let plan = Plan {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: description.to_string(),
        created_at: now_utc(),
        last_executed: None,
        items,
    };

    state.plans.insert(plan.id.clone(), plan.clone());
    state.last_modified = now_utc();
    save_to_disk(&state)?;

    Ok(plan)
}

/// Atualiza nome, descrição e itens de um plano existente.
///
/// Preserva `created_at`, `last_executed` e o `id` original.
/// Retorna erro se o `plan_id` não existir.
pub fn update_plan(
    plan_id: &str,
    name: &str,
    description: &str,
    items: Vec<PlanItem>,
) -> Result<Plan, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let plan = state
        .plans
        .get_mut(plan_id)
        .ok_or_else(|| format!("Plano '{}' não encontrado", plan_id))?;

    plan.name = name.to_string();
    plan.description = description.to_string();
    plan.items = items;

    let updated = plan.clone();

    state.last_modified = now_utc();
    save_to_disk(&state)?;

    Ok(updated)
}

/// Remove permanentemente um plano do arquivo.
///
/// Retorna erro se o `plan_id` não existir.
pub fn delete_plan(plan_id: &str) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    state
        .plans
        .remove(plan_id)
        .ok_or_else(|| format!("Plano '{}' não encontrado para remoção", plan_id))?;

    state.last_modified = now_utc();
    save_to_disk(&state)
}

/// Retorna uma cópia de um plano específico pelo seu ID.
///
/// Retorna erro se o `plan_id` não existir.
pub fn get_plan(plan_id: &str) -> Result<Plan, String> {
    let state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    state
        .plans
        .get(plan_id)
        .cloned()
        .ok_or_else(|| format!("Plano '{}' não encontrado", plan_id))
}

/// Retorna todos os planos ordenados por `created_at` ascendente (mais antigos primeiro).
///
/// Retorna `Vec` vazio se nenhum plano estiver cadastrado.
pub fn get_all_plans() -> Result<Vec<Plan>, String> {
    let state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let mut plans: Vec<Plan> = state.plans.values().cloned().collect();

    // Ordenação estável por data de criação — mais antigos aparecem primeiro
    plans.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    Ok(plans)
}

/// Substitui completamente o estado de planos pelo conteúdo fornecido.
///
/// Usado pela importação de configurações (`import_config` no modo `"replace"`).
/// Persiste no disco e atualiza o cache em memória atomicamente, garantindo
/// consistência imediata sem necessidade de reiniciar o aplicativo.
pub fn replace_all_plans(new_state: PlansFile) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    save_to_disk(&new_state)?;
    *state = new_state;
    Ok(())
}

/// Mescla planos importados com o estado atual, preservando os existentes.
///
/// Usado pela importação de configurações (`import_config` no modo `"merge"`).
/// Apenas planos cujos IDs **não existem** na base atual são adicionados —
/// planos com o mesmo ID são preservados sem modificação.
///
/// # Retorna
/// A quantidade de planos efetivamente adicionados.
pub fn merge_plans(new_plans: Vec<Plan>) -> Result<usize, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let mut added = 0;

    for plan in new_plans {
        if !state.plans.contains_key(&plan.id) {
            state.plans.insert(plan.id.clone(), plan);
            added += 1;
        }
    }

    if added > 0 {
        state.last_modified = now_utc();
        save_to_disk(&state)?;
    }

    Ok(added)
}

/// Registra o timestamp da execução mais recente de um plano.
///
/// Chamado automaticamente por `execute_plan` ao término de cada execução.
/// Retorna erro se o `plan_id` não existir.
pub fn mark_executed(plan_id: &str) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let plan = state
        .plans
        .get_mut(plan_id)
        .ok_or_else(|| format!("Plano '{}' não encontrado para atualizar execução", plan_id))?;

    plan.last_executed = Some(now_utc());
    state.last_modified = now_utc();
    save_to_disk(&state)
}
