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
    /// `true` para planos oficiais do FrameGuard (não editáveis/removíveis pelo usuário)
    #[serde(default)]
    pub builtin: bool,
    /// Versão do plano built-in; `None` para planos do usuário.
    /// Usado para atualizar planos oficiais em atualizações do app sem exigir
    /// que o usuário delete o `plans.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin_version: Option<u32>,
    /// Descrição longa / mini-tutorial do plano (apenas built-in)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_description: Option<String>,
    /// Frequência recomendada de execução (ex: "1x por mês")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_frequency: Option<String>,
    /// Público-alvo (ex: "Para todos", "Para gamers")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_audience: Option<String>,
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
/// Na primeira inicialização, injeta planos built-in que ainda não existam.
fn get_state() -> &'static Mutex<PlansFile> {
    STATE.get_or_init(|| {
        let mut data = load_from_disk().unwrap_or_else(|_| PlansFile::new());
        seed_builtin_plans(&mut data);
        Mutex::new(data)
    })
}

// ─── Planos built-in ─────────────────────────────────────────────────────────

/// IDs determinísticos para planos built-in (permite detectar se já existem).
const BUILTIN_MANUTENCAO_BASICA: &str = "builtin_manutencao_basica";
const BUILTIN_SAUDE_COMPLETA: &str = "builtin_saude_completa";
const BUILTIN_OTIMIZACAO_GAMING: &str = "builtin_otimizacao_gaming";
const BUILTIN_PRIVACIDADE_DEBLOAT: &str = "builtin_privacidade_debloat";

/// Versão atual das definições de planos built-in.
///
/// Incrementar sempre que a definição de qualquer plano built-in mudar
/// (ordem de itens, adição/remoção de tweaks, nome, descrição).
/// Na inicialização, planos com `builtin_version < CURRENT_BUILTIN_VERSION`
/// são atualizados automaticamente (preservando `last_executed`).
///
/// Histórico:
///   1 — versão inicial (planos criados sem campo builtin_version)
///   2 — reordenação do Saúde Completa (limpeza antes de scans)
///   3 — adicionado disable_nvidia_telemetry ao gaming plan (auto-skip por hardware)
///   4 — adicionado long_description, recommended_frequency e target_audience
const CURRENT_BUILTIN_VERSION: u32 = 4;

/// Injeta planos built-in que ainda não existam no estado, e atualiza
/// planos com versão inferior à `CURRENT_BUILTIN_VERSION`.
///
/// Na atualização, `last_executed` é preservado para que o usuário
/// mantenha o histórico de quando executou o plano.
fn seed_builtin_plans(state: &mut PlansFile) {
    let now = now_utc();
    let mut modified = false;

    // Helper: cria PlanItem com order sequencial
    fn items(ids: &[&str]) -> Vec<PlanItem> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| PlanItem {
                tweak_id: id.to_string(),
                order: i as u32,
                enabled: true,
            })
            .collect()
    }

    /// Definição canônica de um plano built-in para inserção/atualização.
    struct BuiltinDef {
        id: &'static str,
        name: &'static str,
        description: &'static str,
        tweak_ids: &'static [&'static str],
        long_description: &'static str,
        recommended_frequency: &'static str,
        target_audience: &'static str,
    }

    let definitions = [
        BuiltinDef {
            id: BUILTIN_MANUTENCAO_BASICA,
            name: "Manutenção Básica",
            description: "Limpeza rápida: flush DNS, temporários e TRIM de SSDs",
            tweak_ids: &["flush_dns", "temp_cleanup", "ssd_trim"],
            long_description: "Este plano executa uma limpeza rápida do seu sistema:\n\n\
                1. Flush DNS — limpa o cache de resolução de nomes para resolver problemas de conexão\n\
                2. Limpeza de temporários — remove arquivos temporários do Windows e cache de apps\n\
                3. TRIM de SSDs — otimiza os SSDs para manter a velocidade de gravação\n\n\
                Recomendado: Execute semanalmente ou quando sentir que o sistema está lento. Execução rápida (~1-2 minutos).",
            recommended_frequency: "Semanalmente ou quando notar lentidão",
            target_audience: "Para todos",
        },
        BuiltinDef {
            id: BUILTIN_SAUDE_COMPLETA,
            name: "Saúde Completa",
            description: "Limpeza + verificação completa: DISM, SFC, Check Disk e TRIM",
            tweak_ids: &[
                // Limpeza primeiro — menos arquivos = scans mais rápidos
                "temp_cleanup",
                "flush_dns",
                "ssd_trim",
                // Verificação e reparo
                "dism_checkhealth",
                "dism_scanhealth",
                "dism_restorehealth",
                "dism_cleanup",
                "sfc_scannow",
                "chkdsk",
            ],
            long_description: "Este plano executa uma verificação completa do seu sistema:\n\n\
                1. Limpeza de temporários — remove arquivos temp e cache\n\
                2. Flush DNS — limpa cache de resolução DNS\n\
                3. TRIM de SSDs — otimiza a velocidade dos SSDs\n\
                4. DISM CheckHealth — verificação rápida de integridade do component store\n\
                5. DISM ScanHealth — scan profundo do component store\n\
                6. DISM RestoreHealth — repara componentes corrompidos via Windows Update\n\
                7. DISM ComponentCleanup — remove versões antigas de componentes do Windows\n\
                8. SFC ScanNow — verifica e repara arquivos protegidos do sistema\n\
                9. Check Disk — verifica erros no sistema de arquivos do disco\n\n\
                Recomendado: Execute uma vez por mês, ou quando sentir que o Windows está lento ou instável. A execução completa pode levar 10-20 minutos.",
            recommended_frequency: "1x por mês ou quando o Windows parecer instável",
            target_audience: "Para todos",
        },
        BuiltinDef {
            id: BUILTIN_OTIMIZACAO_GAMING,
            name: "Otimização Gaming",
            description: "Tweaks essenciais para máximo desempenho em jogos",
            tweak_ids: &[
                "enable_hags",
                "enable_game_mode",
                "disable_vbs",
                "disable_game_dvr",
                "enable_timer_resolution",
                "disable_mouse_acceleration",
                "enable_ultimate_performance",
                "disable_nvidia_telemetry", // auto-skip em hardware não-NVIDIA
            ],
            long_description: "Configura tweaks essenciais para máximo desempenho em jogos:\n\n\
                1. HAGS — transfere scheduling de GPU para o hardware, reduzindo latência\n\
                2. Game Mode — prioriza CPU e GPU para o jogo em execução\n\
                3. VBS desabilitado — remove virtualização que pode custar 5-10% de FPS\n\
                4. Game DVR desabilitado — remove gravação em background que consome GPU\n\
                5. Timer 1ms — melhora frame pacing e input lag em monitores 144Hz+\n\
                6. Aceleração do mouse — desabilita para movimentação 1:1 (raw input)\n\
                7. Ultimate Performance — plano de energia sem throttling de CPU\n\
                8. Telemetria NVIDIA — remove coleta de dados da NVIDIA (auto-ignorado em AMD/Intel)\n\n\
                Recomendado: Execute após instalar ou reinstalar o Windows e drivers de GPU. Reinicie o PC após executar.",
            recommended_frequency: "1x após instalar drivers ou reinstalar o Windows",
            target_audience: "Para gamers",
        },
        BuiltinDef {
            id: BUILTIN_PRIVACIDADE_DEBLOAT,
            name: "Privacidade e Debloat",
            description: "Remove telemetria, bloatware e integração com serviços Microsoft",
            tweak_ids: &[
                "disable_telemetry_registry",
                "disable_copilot",
                "disable_content_delivery",
                "disable_background_apps",
                "disable_bing_search",
            ],
            long_description: "Remove telemetria e integração com serviços Microsoft:\n\n\
                1. Telemetria Windows — reduz coleta de dados para o mínimo (3 chaves de registro)\n\
                2. Copilot / Cortana — desabilita assistentes IA e voz integrados\n\
                3. Content Delivery — bloqueia instalação silenciosa de apps \"sugeridos\"\n\
                4. Background Apps — impede apps UWP de executar em segundo plano\n\
                5. Bing no Menu Iniciar — remove resultados web da pesquisa do Menu Iniciar\n\n\
                Recomendado: Execute uma vez após instalar o Windows. Após grandes updates (como 24H2), re-execute pois a Microsoft pode reativar configurações.",
            recommended_frequency: "1x após instalar o Windows; re-executar após updates",
            target_audience: "Para todos",
        },
    ];

    for def in &definitions {
        let needs_update = match state.plans.get(def.id) {
            // Plano não existe — precisa inserir
            None => true,
            // Plano existe mas com versão antiga (ou sem versão = v1) — precisa atualizar
            Some(existing) => {
                existing.builtin_version.unwrap_or(1) < CURRENT_BUILTIN_VERSION
            }
        };

        if needs_update {
            // Preserva last_executed do plano anterior (se existia)
            let last_executed = state
                .plans
                .get(def.id)
                .and_then(|p| p.last_executed.clone());

            // Preserva created_at original (se existia), senão usa agora
            let created_at = state
                .plans
                .get(def.id)
                .map(|p| p.created_at.clone())
                .unwrap_or_else(|| now.clone());

            state.plans.insert(
                def.id.to_string(),
                Plan {
                    id: def.id.to_string(),
                    name: def.name.to_string(),
                    description: def.description.to_string(),
                    created_at,
                    last_executed,
                    items: items(def.tweak_ids),
                    builtin: true,
                    builtin_version: Some(CURRENT_BUILTIN_VERSION),
                    long_description: Some(def.long_description.to_string()),
                    recommended_frequency: Some(def.recommended_frequency.to_string()),
                    target_audience: Some(def.target_audience.to_string()),
                },
            );
            modified = true;
        }
    }

    // Migração: garante que planos com IDs builtin_ tenham `builtin: true`
    // (para plans.json salvos antes da introdução do campo)
    for plan in state.plans.values_mut() {
        if plan.id.starts_with("builtin_") && !plan.builtin {
            plan.builtin = true;
            modified = true;
        }
    }

    if modified {
        state.last_modified = now;
        // Persiste no disco — erro é silenciado (planos ficam em memória de qualquer forma)
        let _ = save_to_disk(state);
    }
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
        builtin: false,
        builtin_version: None,
        long_description: None,
        recommended_frequency: None,
        target_audience: None,
    };

    state.plans.insert(plan.id.clone(), plan.clone());
    state.last_modified = now_utc();
    save_to_disk(&state)?;

    Ok(plan)
}

/// Atualiza nome, descrição e itens de um plano existente.
///
/// Planos oficiais (`builtin == true`) não podem ser editados — retorna erro
/// instruindo o usuário a duplicar o plano.
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

    if plan.builtin {
        return Err(
            "Planos oficiais não podem ser editados. Use 'Duplicar e personalizar' para criar uma versão customizada.".to_string()
        );
    }

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
/// Planos oficiais (`builtin == true`) não podem ser removidos.
/// Retorna erro se o `plan_id` não existir.
pub fn delete_plan(plan_id: &str) -> Result<(), String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    // Verifica se é builtin antes de remover
    if let Some(plan) = state.plans.get(plan_id) {
        if plan.builtin {
            return Err("Planos oficiais não podem ser removidos.".to_string());
        }
    }

    state
        .plans
        .remove(plan_id)
        .ok_or_else(|| format!("Plano '{}' não encontrado para remoção", plan_id))?;

    state.last_modified = now_utc();
    save_to_disk(&state)
}

/// Duplica um plano existente com novo UUID, `builtin: false` e nome com sufixo " (Cópia)".
///
/// Permite ao usuário personalizar planos oficiais sem alterar o original.
/// Retorna o plano duplicado já persistido em disco.
pub fn duplicate_plan(plan_id: &str) -> Result<Plan, String> {
    let mut state = get_state()
        .lock()
        .map_err(|_| "Falha ao adquirir lock no estado de planos".to_string())?;

    let source = state
        .plans
        .get(plan_id)
        .ok_or_else(|| format!("Plano '{}' não encontrado para duplicação", plan_id))?
        .clone();

    let duplicate = Plan {
        id: Uuid::new_v4().to_string(),
        name: format!("{} (Cópia)", source.name),
        description: source.description,
        created_at: now_utc(),
        last_executed: None,
        items: source.items,
        builtin: false,
        builtin_version: None,
        long_description: source.long_description,
        recommended_frequency: source.recommended_frequency,
        target_audience: source.target_audience,
    };

    state.plans.insert(duplicate.id.clone(), duplicate.clone());
    state.last_modified = now_utc();
    save_to_disk(&state)?;

    Ok(duplicate)
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
