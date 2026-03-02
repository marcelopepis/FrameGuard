// Hook compartilhado para execução de planos.
//
// Encapsula toda a lógica de execução (listener de progresso, invoke,
// estado de cada item, resumo final) para reutilizar entre Dashboard
// e a página de Planos sem duplicar código.

import { useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isRestorePointEnabled } from '../utils/restorePoint';

// ── Tipos ────────────────────────────────────────────────────────────────────

export interface PlanItem {
  tweak_id: string;
  order: number;
  enabled: boolean;
}

export interface Plan {
  id: string;
  name: string;
  description: string;
  created_at: string;
  last_executed: string | null;
  items: PlanItem[];
  builtin: boolean;
  builtin_version: number | null;
}

interface HealthCheckData {
  id: string;
  name: string;
  status: 'success' | 'warning' | 'error';
  message: string;
  details?: string;
  duration_seconds: number;
  space_freed_mb?: number;
}

interface ItemResult {
  tweak_id: string;
  status: 'completed' | 'failed' | 'skipped';
  error: string | null;
  result_data: HealthCheckData | null;
}

interface PlanProgress {
  plan_id: string;
  current_item: string;
  current_item_index: number;
  total_items: number;
  item_status: 'running' | 'completed' | 'failed' | 'skipped';
  item_result: ItemResult | null;
  overall_progress_percent: number;
}

export interface PlanExecutionSummary {
  plan_id: string;
  plan_name: string;
  duration_seconds: number;
  total_items: number;
  completed_count: number;
  failed_count: number;
  skipped_count: number;
  results: ItemResult[];
}

export type ItemStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface ItemExecState {
  status: ItemStatus;
  message?: string;
  details?: string;
  error?: string;
  hcStatus?: 'success' | 'warning' | 'error';
}

export interface RestorePointStatus {
  status: 'created' | 'skipped' | 'disabled' | 'failed';
  message: string;
}

export interface ExecState {
  running: boolean;
  items: Record<string, ItemExecState>;
  progress: number;
  summary: PlanExecutionSummary | null;
  fatalError: string | null;
  restorePoint: RestorePointStatus | null;
}

// ── Hook ────────────────────────────────────────────────────────────────────

export function usePlanExecution() {
  const [executingPlan, setExecutingPlan] = useState<Plan | null>(null);
  const [execState, setExecState] = useState<ExecState | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  const execute = useCallback(async (plan: Plan, onDone?: () => void) => {
    // Inicializa estado: todos os itens como 'pending'
    const initialItems: Record<string, ItemExecState> = {};
    plan.items.forEach(item => {
      initialItems[item.tweak_id] = { status: 'pending' };
    });

    setExecutingPlan(plan);
    setExecState({ running: true, items: initialItems, progress: 0, summary: null, fatalError: null, restorePoint: null });

    // Listener para status do ponto de restauração (emitido pelo backend antes de executar)
    const unlistenRp = await listen<RestorePointStatus>('restore_point_status', (event) => {
      setExecState(prev => prev ? { ...prev, restorePoint: event.payload } : prev);
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

      setExecState(prev => prev ? {
        ...prev,
        progress: p.overall_progress_percent,
        items: { ...prev.items, [p.current_item]: newItemState },
      } : prev);
    });

    unlistenRef.current = unlisten;

    try {
      const summary = await invoke<PlanExecutionSummary>('execute_plan', {
        planId: plan.id,
        shouldCreateRestorePoint: isRestorePointEnabled(),
      });
      setExecState(prev => prev
        ? { ...prev, running: false, summary, progress: 100 }
        : prev,
      );
      onDone?.();
    } catch (err) {
      setExecState(prev => prev
        ? { ...prev, running: false, fatalError: String(err) }
        : prev,
      );
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
