/**
 * Hook compartilhado para execução de planos.
 *
 * Encapsula toda a lógica de execução (listener de progresso, invoke,
 * estado de cada item, resumo final) para reutilizar entre Dashboard
 * e a página de Planos sem duplicar código.
 *
 * @module usePlanExecution
 */

import { useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isRestorePointEnabled } from '../utils/restorePoint';

// ── Tipos ────────────────────────────────────────────────────────────────────

/** Item individual dentro de um plano de execução. */
export interface PlanItem {
  /** ID do tweak/ação a executar */
  tweak_id: string;
  /** Ordem de execução (ascendente) */
  order: number;
  /** Se `false`, o item é ignorado durante a execução */
  enabled: boolean;
}

/** Plano de execução completo retornado pelo backend. */
export interface Plan {
  /** UUID v4 do plano */
  id: string;
  /** Nome legível do plano */
  name: string;
  /** Descrição curta do objetivo */
  description: string;
  /** Timestamp ISO 8601 de criação */
  created_at: string;
  /** Timestamp da última execução, ou `null` se nunca executado */
  last_executed: string | null;
  /** Lista de itens do plano */
  items: PlanItem[];
  /** `true` para planos oficiais (prefixo `builtin_`) */
  builtin: boolean;
  /** Versão do plano built-in (usado para auto-update) */
  builtin_version: number | null;
  /** Descrição longa exibida no modal de detalhes (apenas planos built-in) */
  long_description?: string | null;
  /** Frequência de execução recomendada (ex: `"Semanal"`) */
  recommended_frequency?: string | null;
  /** Público-alvo do plano (ex: `"Todos os usuários"`) */
  target_audience?: string | null;
}

/** Dados de resultado de uma ação de saúde (DISM, SFC, etc.) dentro de um plano. */
interface HealthCheckData {
  /** ID da ação */
  id: string;
  /** Nome legível da ação */
  name: string;
  /** Resultado: sucesso, aviso ou erro */
  status: 'success' | 'warning' | 'error';
  /** Mensagem de resultado legível */
  message: string;
  /** Detalhes técnicos adicionais */
  details?: string;
  /** Duração da execução em segundos */
  duration_seconds: number;
  /** Espaço em disco liberado em MB (se aplicável) */
  space_freed_mb?: number;
}

/** Resultado normalizado de um item após execução dentro de um plano. */
interface ItemResult {
  /** ID do tweak executado */
  tweak_id: string;
  /** Estado final do item */
  status: 'completed' | 'failed' | 'skipped';
  /** Mensagem de erro quando `status === "failed"` */
  error: string | null;
  /** Dados do HealthCheck (para ações de saúde) ou `null` */
  result_data: HealthCheckData | null;
}

/** Payload do evento `"plan_progress"` emitido pelo backend durante a execução. */
interface PlanProgress {
  /** ID do plano em execução */
  plan_id: string;
  /** `tweak_id` do item sendo processado */
  current_item: string;
  /** Índice 0-based do item atual */
  current_item_index: number;
  /** Total de itens no plano */
  total_items: number;
  /** Estado atual do item */
  item_status: 'running' | 'completed' | 'failed' | 'skipped';
  /** Resultado do item (preenchido em `"completed"` e `"failed"`) */
  item_result: ItemResult | null;
  /** Percentual de conclusão geral (0–100) */
  overall_progress_percent: number;
}

/** Resumo completo retornado ao término de `execute_plan`. */
export interface PlanExecutionSummary {
  /** ID do plano executado */
  plan_id: string;
  /** Nome do plano no momento da execução */
  plan_name: string;
  /** Duração total em segundos */
  duration_seconds: number;
  /** Total de itens no plano */
  total_items: number;
  /** Itens concluídos com sucesso */
  completed_count: number;
  /** Itens que falharam */
  failed_count: number;
  /** Itens ignorados (desabilitados ou hardware incompatível) */
  skipped_count: number;
  /** Resultado individual de cada item, na ordem de execução */
  results: ItemResult[];
}

/** Estados possíveis de um item durante a execução de um plano. */
export type ItemStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

/** Estado de execução de um item individual no modal de progresso. */
export interface ItemExecState {
  /** Estado atual do item */
  status: ItemStatus;
  /** Mensagem de resultado legível (ex: `"Nenhum problema encontrado"`) */
  message?: string;
  /** Detalhes técnicos adicionais */
  details?: string;
  /** Mensagem de erro quando `status === "failed"` */
  error?: string;
  /** Status do HealthCheck para itens de saúde (determina ícone/cor) */
  hcStatus?: 'success' | 'warning' | 'error';
}

/** Status do ponto de restauração criado antes da execução do plano. */
export interface RestorePointStatus {
  /** Resultado da tentativa de criação */
  status: 'created' | 'skipped' | 'disabled' | 'failed';
  /** Mensagem descritiva para exibição em toast */
  message: string;
}

/** Estado geral da execução de um plano (usado pelo modal de progresso). */
export interface ExecState {
  /** `true` enquanto o plano está em execução */
  running: boolean;
  /** Estado de cada item indexado por `tweak_id` */
  items: Record<string, ItemExecState>;
  /** Percentual de progresso geral (0–100) */
  progress: number;
  /** Resumo final após conclusão — `null` durante execução */
  summary: PlanExecutionSummary | null;
  /** Erro fatal que impediu a execução — `null` se OK */
  fatalError: string | null;
  /** Status do ponto de restauração — `null` se não solicitado */
  restorePoint: RestorePointStatus | null;
}

// ── Hook ────────────────────────────────────────────────────────────────────

/**
 * Hook que gerencia a execução de planos com progresso em tempo real.
 *
 * Registra listeners para eventos `"plan_progress"` e `"restore_point_status"`
 * antes de invocar `execute_plan`, atualizando o estado de cada item conforme
 * os eventos chegam do backend.
 *
 * @returns Objeto com:
 *   - `executingPlan` — plano atualmente em execução (ou `null`)
 *   - `execState` — estado completo da execução (itens, progresso, resumo)
 *   - `execute(plan, onDone?)` — inicia a execução de um plano
 *   - `closeModal()` — fecha o modal de execução (bloqueado durante execução)
 *   - `cleanup()` — remove listeners pendentes (chamar no unmount)
 *
 * @example
 * ```tsx
 * const { executingPlan, execState, execute, closeModal, cleanup } = usePlanExecution();
 * // No unmount: cleanup()
 * ```
 */
export function usePlanExecution() {
  const [executingPlan, setExecutingPlan] = useState<Plan | null>(null);
  const [execState, setExecState] = useState<ExecState | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  const execute = useCallback(async (plan: Plan, onDone?: () => void) => {
    // Inicializa estado: todos os itens como 'pending'
    const initialItems: Record<string, ItemExecState> = {};
    plan.items.forEach((item) => {
      initialItems[item.tweak_id] = { status: 'pending' };
    });

    setExecutingPlan(plan);
    setExecState({
      running: true,
      items: initialItems,
      progress: 0,
      summary: null,
      fatalError: null,
      restorePoint: null,
    });

    // Listener para status do ponto de restauração (emitido pelo backend antes de executar)
    const unlistenRp = await listen<RestorePointStatus>('restore_point_status', (event) => {
      setExecState((prev) => (prev ? { ...prev, restorePoint: event.payload } : prev));
    });

    // Registra listener ANTES do invoke (para não perder eventos iniciais)
    const unlisten = await listen<PlanProgress>('plan_progress', (event) => {
      const p = event.payload;

      const newItemState: ItemExecState = { status: p.item_status };

      if (p.item_result?.error) {
        newItemState.error = p.item_result.error;
      }
      if (p.item_result?.result_data) {
        const rd = p.item_result.result_data;
        newItemState.message = rd.message;
        newItemState.details = rd.details;
        newItemState.hcStatus = rd.status;
      }

      setExecState((prev) =>
        prev
          ? {
              ...prev,
              progress: p.overall_progress_percent,
              items: { ...prev.items, [p.current_item]: newItemState },
            }
          : prev,
      );
    });

    unlistenRef.current = unlisten;

    try {
      const summary = await invoke<PlanExecutionSummary>('execute_plan', {
        planId: plan.id,
        shouldCreateRestorePoint: isRestorePointEnabled(),
      });
      setExecState((prev) => (prev ? { ...prev, running: false, summary, progress: 100 } : prev));
      onDone?.();
    } catch (err) {
      setExecState((prev) => (prev ? { ...prev, running: false, fatalError: String(err) } : prev));
    } finally {
      unlisten();
      unlistenRp();
      unlistenRef.current = null;
    }
  }, []);

  const closeModal = useCallback(() => {
    if (execState?.running) return;
    setExecutingPlan(null);
    setExecState(null);
  }, [execState?.running]);

  const cleanup = useCallback(() => {
    unlistenRef.current?.();
  }, []);

  return { executingPlan, execState, execute, closeModal, cleanup };
}
