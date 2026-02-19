//! Comandos Tauri para gerenciamento e execução de Planos de Execução.
//!
//! Um Plano de Execução é uma lista personalizada de tweaks/ações que o usuário
//! monta para rodar em sequência com um clique. Exemplo: "Manutenção Semanal"
//! com DISM CheckHealth → SFC → TRIM → Limpeza de Temporários.
//!
//! ## Fluxo de execução (`execute_plan`)
//! Para cada item habilitado (ordenado por `order` ascendente):
//!   1. Emite evento `"plan_progress"` com `item_status: "running"`
//!   2. Chama o tweak/ação correspondente diretamente (sem passar pela IPC Tauri)
//!   3. Emite evento `"plan_progress"` com `item_status: "completed"` ou `"failed"`
//!   4. Continua para o próximo item (falhas não interrompem o plano)
//!
//! Itens com `enabled: false` recebem evento `"skipped"` e são contabilizados
//! no resumo final, mas não são executados.

use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use tauri::Emitter;

use crate::utils::plan_manager::{self, PlanItem};

// ─── Tipos de evento e resultado ─────────────────────────────────────────────

/// Payload dos eventos `"plan_progress"` emitidos durante `execute_plan`.
///
/// O frontend deve registrar `listen("plan_progress", handler)` para
/// acompanhar o progresso item a item.
#[derive(Debug, Clone, Serialize)]
pub struct PlanProgress {
    /// ID do plano em execução
    pub plan_id: String,
    /// `tweak_id` do item sendo processado no momento
    pub current_item: String,
    /// Índice 0-based do item atual na lista de itens totais
    pub current_item_index: usize,
    /// Total de itens no plano (habilitados + desabilitados)
    pub total_items: usize,
    /// Estado do item: `"running"` | `"completed"` | `"failed"` | `"skipped"`
    pub item_status: String,
    /// Resultado serializado do tweak; `null` para eventos `"running"` e `"skipped"`
    pub item_result: Option<serde_json::Value>,
    /// Percentual de conclusão baseado em itens processados: `(processados * 100) / total`
    pub overall_progress_percent: u32,
}

/// Resultado normalizado de um item individual dentro da execução do plano.
#[derive(Debug, Clone, Serialize)]
pub struct ItemResult {
    /// ID do tweak executado
    pub tweak_id: String,
    /// `"completed"` | `"failed"` | `"skipped"`
    pub status: String,
    /// Mensagem de erro quando `status == "failed"`; `null` caso contrário
    pub error: Option<String>,
    /// Output do tweak serializado como JSON; `null` para itens skipped ou tweaks sem retorno
    pub result_data: Option<serde_json::Value>,
}

/// Resumo completo retornado ao término de `execute_plan`.
#[derive(Debug, Serialize)]
pub struct PlanExecutionSummary {
    /// ID do plano executado
    pub plan_id: String,
    /// Nome do plano no momento da execução
    pub plan_name: String,
    /// Timestamp ISO 8601 UTC do início da execução
    pub started_at: String,
    /// Timestamp ISO 8601 UTC do término da execução
    pub completed_at: String,
    /// Duração total em segundos
    pub duration_seconds: u64,
    /// Total de itens no plano (habilitados + desabilitados)
    pub total_items: usize,
    /// Itens que foram executados com sucesso ou warning
    pub completed_count: usize,
    /// Itens que falharam durante a execução
    pub failed_count: usize,
    /// Itens pulados por estarem com `enabled: false`
    pub skipped_count: usize,
    /// Resultado individual de cada item, na ordem de execução
    pub results: Vec<ItemResult>,
}

// ─── Utilitário de timestamp ──────────────────────────────────────────────────

fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ─── Dispatcher de tweaks ─────────────────────────────────────────────────────

/// Executa um único tweak/ação pelo seu `tweak_id` e retorna o resultado normalizado.
///
/// Este dispatcher centraliza o mapeamento de IDs textuais para chamadas de função,
/// capturando todos os erros internamente para que o executor de planos possa
/// continuar para o próximo item sem interrupção.
///
/// ## Tweaks suportados
/// **Saúde do Sistema** (retornam `HealthCheckResult`):
/// - `"dism_cleanup"`, `"dism_checkhealth"`, `"dism_scanhealth"`, `"dism_restorehealth"`
/// - `"sfc_scannow"`, `"chkdsk"`, `"ssd_trim"`, `"flush_dns"`, `"temp_cleanup"`
///
/// **Otimizações** (retornam `()` em sucesso):
/// - `"disable_wallpaper_compression"`, `"revert_wallpaper_compression"`
/// - `"disable_reserved_storage"`, `"enable_reserved_storage"`
/// - `"disable_delivery_optimization"`, `"revert_delivery_optimization"`
fn execute_single_tweak(app: &tauri::AppHandle, tweak_id: &str) -> ItemResult {
    use super::{health_check, optimizations};

    // Tenta executar o tweak e converte o resultado para Option<serde_json::Value>
    // Ok(Some(json)) = sucesso com dados retornados (ex: HealthCheckResult)
    // Ok(None)       = sucesso sem dados (ex: tweaks de otimização que retornam ())
    // Err(msg)       = falha — item marcado como "failed" no resumo
    let outcome: Result<Option<serde_json::Value>, String> = match tweak_id {
        // ── Saúde: DISM ──────────────────────────────────────────────────────
        "dism_cleanup" => health_check::run_dism_cleanup(app.clone())
            .map(|r| Some(to_json(r))),

        "dism_checkhealth" => health_check::run_dism_checkhealth(app.clone())
            .map(|r| Some(to_json(r))),

        "dism_scanhealth" => health_check::run_dism_scanhealth(app.clone())
            .map(|r| Some(to_json(r))),

        "dism_restorehealth" => health_check::run_dism_restorehealth(app.clone())
            .map(|r| Some(to_json(r))),

        // ── Saúde: Verificações ──────────────────────────────────────────────
        "sfc_scannow" => health_check::run_sfc(app.clone())
            .map(|r| Some(to_json(r))),

        // chkdsk sem drive_letter especificado → padrão C:
        "chkdsk" => health_check::run_chkdsk(app.clone(), None)
            .map(|r| Some(to_json(r))),

        "ssd_trim" => health_check::run_ssd_trim(app.clone())
            .map(|r| Some(to_json(r))),

        // ── Saúde: Manutenção ────────────────────────────────────────────────
        "flush_dns" => health_check::flush_dns(app.clone())
            .map(|r| Some(to_json(r))),

        "temp_cleanup" => health_check::run_temp_cleanup(app.clone())
            .map(|r| Some(to_json(r))),

        // ── Otimizações: Wallpaper ────────────────────────────────────────────
        "disable_wallpaper_compression" => optimizations::disable_wallpaper_compression()
            .map(|_| None),

        "revert_wallpaper_compression" => optimizations::revert_wallpaper_compression()
            .map(|_| None),

        // ── Otimizações: Armazenamento Reservado ─────────────────────────────
        "disable_reserved_storage" => optimizations::disable_reserved_storage(app.clone())
            .map(|_| None),

        "enable_reserved_storage" => optimizations::enable_reserved_storage(app.clone())
            .map(|_| None),

        // ── Otimizações: Delivery Optimization ──────────────────────────────
        "disable_delivery_optimization" => optimizations::disable_delivery_optimization()
            .map(|_| None),

        "revert_delivery_optimization" => optimizations::revert_delivery_optimization()
            .map(|_| None),

        // ── Desconhecido ─────────────────────────────────────────────────────
        other => Err(format!(
            "Tweak '{}' não reconhecido pelo executor de planos. \
             Verifique se o ID está correto e o tweak está registrado.",
            other
        )),
    };

    match outcome {
        Ok(data) => ItemResult {
            tweak_id: tweak_id.to_string(),
            status: "completed".to_string(),
            error: None,
            result_data: data,
        },
        Err(e) => ItemResult {
            tweak_id: tweak_id.to_string(),
            status: "failed".to_string(),
            error: Some(e),
            result_data: None,
        },
    }
}

/// Serializa qualquer tipo `Serialize` em `serde_json::Value`.
/// Em caso de falha de serialização (não deve ocorrer na prática), retorna um objeto de erro.
fn to_json<T: serde::Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(&value).unwrap_or_else(|e| {
        json!({ "serialization_error": format!("Falha ao serializar resultado: {}", e) })
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Comandos Tauri
// ═══════════════════════════════════════════════════════════════════════════════

/// Cria um novo Plano de Execução e o persiste em `plans.json`.
///
/// O ID do plano é gerado automaticamente (UUID v4). Os itens são armazenados
/// como fornecidos pelo frontend — a ordem de execução é determinada pelo campo
/// `order` de cada item.
///
/// # Parâmetros
/// - `name`: nome legível do plano (ex: `"Manutenção Semanal"`)
/// - `description`: descrição opcional do objetivo do plano
/// - `items`: lista de itens com `tweak_id`, `order` e `enabled`
///
/// # Retorna
/// O `Plan` criado completo, incluindo o `id` e `created_at` gerados.
#[tauri::command]
pub fn create_plan(
    name: String,
    description: String,
    items: Vec<PlanItem>,
) -> Result<plan_manager::Plan, String> {
    plan_manager::create_plan(&name, &description, items)
}

/// Atualiza nome, descrição e itens de um plano existente.
///
/// Preserva o `id`, `created_at` e `last_executed` originais.
/// Retorna erro se o `plan_id` não existir.
#[tauri::command]
pub fn update_plan(
    plan_id: String,
    name: String,
    description: String,
    items: Vec<PlanItem>,
) -> Result<plan_manager::Plan, String> {
    plan_manager::update_plan(&plan_id, &name, &description, items)
}

/// Remove permanentemente um plano de `plans.json`.
///
/// Retorna erro se o `plan_id` não existir.
#[tauri::command]
pub fn delete_plan(plan_id: String) -> Result<(), String> {
    plan_manager::delete_plan(&plan_id)
}

/// Retorna os dados completos de um plano específico pelo seu ID.
///
/// Retorna erro se o `plan_id` não existir.
#[tauri::command]
pub fn get_plan(plan_id: String) -> Result<plan_manager::Plan, String> {
    plan_manager::get_plan(&plan_id)
}

/// Retorna todos os planos cadastrados, ordenados por `created_at` ascendente.
///
/// Retorna lista vazia se nenhum plano existir — nunca retorna erro por ausência de dados.
#[tauri::command]
pub fn get_all_plans() -> Result<Vec<plan_manager::Plan>, String> {
    plan_manager::get_all_plans()
}

/// Executa todos os itens habilitados de um plano em sequência, emitindo
/// eventos de progresso `"plan_progress"` em tempo real para o frontend.
///
/// ## Comportamento detalhado
/// 1. Carrega o plano pelo ID — retorna erro se não encontrado
/// 2. Ordena os itens por `order` ascendente
/// 3. Para cada item:
///    - `enabled: false` → emite `"skipped"`, avança para o próximo
///    - `enabled: true`  → emite `"running"`, executa, emite `"completed"` ou `"failed"`
/// 4. Falhas individuais **não interrompem** o plano — o executor continua sempre
/// 5. Ao final: registra `last_executed` no plano e retorna `PlanExecutionSummary`
///
/// ## Eventos emitidos (nome: `"plan_progress"`, payload: `PlanProgress`)
/// O frontend deve registrar: `listen("plan_progress", handler)`
///
/// # Parâmetros
/// - `plan_id`: ID do plano a executar
///
/// # Retorna
/// `PlanExecutionSummary` com contagens e resultados individuais de cada item.
#[tauri::command]
pub fn execute_plan(
    app_handle: tauri::AppHandle,
    plan_id: String,
) -> Result<PlanExecutionSummary, String> {
    let started_at = now_utc();
    let start_instant = std::time::Instant::now();

    // Carrega o plano — falha aqui é um erro real (ID inexistente)
    let plan = plan_manager::get_plan(&plan_id)?;

    // Ordena itens por `order` ascendente para garantir sequência correta
    let mut items = plan.items.clone();
    items.sort_by_key(|item| item.order);

    let total_items = items.len();
    let mut results: Vec<ItemResult> = Vec::with_capacity(total_items);
    let mut completed_count: usize = 0;
    let mut failed_count: usize = 0;
    let mut skipped_count: usize = 0;
    let mut processed_count: usize = 0; // itens que passaram pelo executor (não skipped)

    for (index, item) in items.iter().enumerate() {
        // Progresso percentual baseado em itens já processados (antes deste)
        let progress_before = percent(processed_count, total_items);

        if !item.enabled {
            // Item desabilitado: emite "skipped" e avança sem executar
            emit_progress(
                &app_handle,
                &plan_id,
                &item.tweak_id,
                index,
                total_items,
                "skipped",
                None,
                progress_before,
            );

            results.push(ItemResult {
                tweak_id: item.tweak_id.clone(),
                status: "skipped".to_string(),
                error: None,
                result_data: None,
            });
            skipped_count += 1;
            continue;
        }

        // Emite "running" antes de iniciar o tweak
        emit_progress(
            &app_handle,
            &plan_id,
            &item.tweak_id,
            index,
            total_items,
            "running",
            None,
            progress_before,
        );

        // Executa o tweak — falhas são capturadas internamente pelo dispatcher
        let result = execute_single_tweak(&app_handle, &item.tweak_id);
        processed_count += 1;

        // Progresso percentual após processar este item
        let progress_after = percent(processed_count, total_items);
        let result_json = to_json(&result);

        // Emite "completed" ou "failed" com o resultado do tweak
        emit_progress(
            &app_handle,
            &plan_id,
            &item.tweak_id,
            index,
            total_items,
            &result.status,
            Some(result_json),
            progress_after,
        );

        match result.status.as_str() {
            "completed" => completed_count += 1,
            "failed" => failed_count += 1,
            _ => {}
        }

        results.push(result);
    }

    // Registra o timestamp de execução no plano — erro não crítico (log e continua)
    if let Err(e) = plan_manager::mark_executed(&plan_id) {
        eprintln!(
            "[FrameGuard] Aviso: não foi possível registrar last_executed para '{}': {}",
            plan_id, e
        );
    }

    let duration_seconds = start_instant.elapsed().as_secs();

    Ok(PlanExecutionSummary {
        plan_id: plan.id,
        plan_name: plan.name,
        started_at,
        completed_at: now_utc(),
        duration_seconds,
        total_items,
        completed_count,
        failed_count,
        skipped_count,
        results,
    })
}

// ─── Helpers internos ─────────────────────────────────────────────────────────

/// Calcula o percentual de progresso: `(done * 100) / total`, retorna 0 se `total == 0`.
fn percent(done: usize, total: usize) -> u32 {
    if total == 0 {
        return 100;
    }
    ((done * 100) / total) as u32
}

/// Emite um evento `"plan_progress"` para o frontend.
/// Falhas de emissão são silenciadas para não interromper a execução.
#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &tauri::AppHandle,
    plan_id: &str,
    current_item: &str,
    current_item_index: usize,
    total_items: usize,
    item_status: &str,
    item_result: Option<serde_json::Value>,
    overall_progress_percent: u32,
) {
    let payload = PlanProgress {
        plan_id: plan_id.to_string(),
        current_item: current_item.to_string(),
        current_item_index,
        total_items,
        item_status: item_status.to_string(),
        item_result,
        overall_progress_percent,
    };
    let _ = app.emit("plan_progress", payload);
}
