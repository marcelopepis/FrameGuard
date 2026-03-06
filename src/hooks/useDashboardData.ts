import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  getStaticHwInfo,
  getGpuInfo,
  getSystemStatus,
  getSystemSummary,
  getSystemUsage,
} from '../services/systemInfo';
import type { StaticHwInfo, GpuInfo, SystemStatus, SystemSummary } from '../services/systemInfo';
import type { Plan } from './usePlanExecution';

/** Entrada do log de atividade recente retornada pelo backend. */
export interface ActivityEntry {
  /** Timestamp ISO 8601 UTC do momento da atividade */
  timestamp: string;
  /** Tipo de atividade registrada */
  activity_type: 'plan_execution' | 'tweak_applied' | 'tweak_reverted';
  /** Nome legível do plano ou tweak executado */
  name: string;
  /** Resultado final: `"success"` (tudo OK), `"partial"` (falhas parciais), `"failed"` */
  result: 'success' | 'partial' | 'failed';
  /** Duração total em segundos */
  duration_seconds: number;
  /** Itens concluídos com sucesso (apenas para planos) */
  completed_count: number | null;
  /** Itens que falharam (apenas para planos) */
  failed_count: number | null;
  /** Itens ignorados (apenas para planos) */
  skipped_count: number | null;
}

/** Dados agregados do Dashboard, carregados progressivamente. */
export interface DashboardData {
  /** Informações estáticas de hardware (CPU, RAM, disco) — `null` enquanto carrega */
  hw: StaticHwInfo | null;
  /** Informações da GPU (nome, VRAM, driver) — `null` enquanto carrega */
  gpu: GpuInfo | null;
  /** Status geral do sistema (temperatura, saúde) — `null` enquanto carrega */
  status: SystemStatus | null;
  /** Resumo textual do sistema para exibição rápida — `null` enquanto carrega */
  summary: SystemSummary | null;
  /** Uso atual de CPU em percentual (0–100) — `null` antes da primeira medição */
  cpuPercent: number | null;
  /** Uso atual de RAM em percentual (0–100) — `null` antes da primeira medição */
  ramPercent: number | null;
  /** RAM utilizada em GB — `null` antes da primeira medição */
  ramUsedGb: number | null;
  /** Últimas 5 entradas do log de atividade — `null` enquanto carrega */
  activity: ActivityEntry[] | null;
  /** Planos built-in filtrados para exibição na seção de Quick Plans — `null` enquanto carrega */
  builtinPlans: Plan[] | null;
  /** Recarrega a lista de atividade recente (após executar um plano, por ex.) */
  refreshActivity: () => void;
}

/**
 * Encapsula todos os estados e fetches do Dashboard.
 *
 * - Carrega hardware info, GPU, status, summary, atividade e planos built-in no mount
 * - Faz polling de CPU/RAM a cada 3s (inicia após HW carregar)
 * - Faz polling de atividade e status a cada 10s
 *
 * @param cleanupPlanExec - Função de limpeza do `usePlanExecution` (chamada no unmount)
 * @returns Objeto `DashboardData` com todos os estados e `refreshActivity`
 *
 * @example
 * ```tsx
 * const { hw, gpu, cpuPercent, ramPercent, activity } = useDashboardData(cleanup);
 * ```
 */
export function useDashboardData(cleanupPlanExec: () => void): DashboardData {
  const [hw, setHw] = useState<StaticHwInfo | null>(null);
  const [gpu, setGpu] = useState<GpuInfo | null>(null);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [summary, setSummary] = useState<SystemSummary | null>(null);
  const [cpuPercent, setCpuPercent] = useState<number | null>(null);
  const [ramPercent, setRamPercent] = useState<number | null>(null);
  const [ramUsedGb, setRamUsedGb] = useState<number | null>(null);
  const [activity, setActivity] = useState<ActivityEntry[] | null>(null);
  const [builtinPlans, setBuiltinPlans] = useState<Plan[] | null>(null);

  const refreshActivity = useCallback(() => {
    invoke<ActivityEntry[]>('get_recent_activity', { limit: 5 })
      .then(setActivity)
      .catch(() => {});
  }, []);

  // Loading progressivo: cada chamada renderiza sua seção assim que os dados chegam
  useEffect(() => {
    getSystemSummary()
      .then(setSummary)
      .catch(() => {});
    refreshActivity();
    invoke<Plan[]>('get_all_plans')
      .then((plans) => setBuiltinPlans(plans.filter((p) => p.builtin)))
      .catch(() => {});
    getStaticHwInfo()
      .then(setHw)
      .catch(() => {});
    getGpuInfo()
      .then(setGpu)
      .catch(() => {});
    getSystemStatus()
      .then(setStatus)
      .catch(() => {});

    return () => {
      cleanupPlanExec();
    };
  }, [cleanupPlanExec, refreshActivity]);

  // Refresh periódico de atividade e status (custo negligível: ~50 ms cada)
  useEffect(() => {
    const id = setInterval(() => {
      refreshActivity();
      getSystemStatus()
        .then(setStatus)
        .catch(() => {});
    }, 10_000);
    return () => clearInterval(id);
  }, [refreshActivity]);

  // Polling de CPU e RAM — só inicia depois que dados de HW carregarem
  useEffect(() => {
    if (!hw) return;

    // Primeira medição imediata
    getSystemUsage()
      .then((u) => {
        setCpuPercent(u.cpu_usage_percent);
        setRamPercent(u.ram_usage_percent);
        setRamUsedGb(Math.round((u.ram_usage_percent / 100) * hw.ram_total_gb * 10) / 10);
      })
      .catch(() => {});

    const id = setInterval(() => {
      getSystemUsage()
        .then((u) => {
          setCpuPercent(u.cpu_usage_percent);
          setRamPercent(u.ram_usage_percent);
          setRamUsedGb(Math.round((u.ram_usage_percent / 100) * hw.ram_total_gb * 10) / 10);
        })
        .catch(() => {});
    }, 3000);
    return () => clearInterval(id);
  }, [hw]);

  return {
    hw,
    gpu,
    status,
    summary,
    cpuPercent,
    ramPercent,
    ramUsedGb,
    activity,
    builtinPlans,
    refreshActivity,
  };
}
