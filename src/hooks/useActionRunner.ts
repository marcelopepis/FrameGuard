// Hook compartilhado para execução de ações com streaming de saída em tempo real.
//
// Encapsula toda a lógica de execução comum às páginas de Saúde do Sistema e
// Limpeza: buffer de linhas pendentes com flush a cada 80ms, listen/unlisten de
// eventos Tauri, persistência no localStorage e integração com GlobalRunningProvider.

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ActionMeta, ActionState, CommandEvent, HealthCheckResult, LogLine } from '../types/health';
import { useGlobalRunning } from '../contexts/RunningContext';

/** Extrai percentual de linhas como " 42.0%" → 42. Usado pelo DISM. */
function extractProgress(text: string): number | null {
  const m = text.trim().match(/^(\d+(?:\.\d+)?)%$/);
  return m ? Math.min(100, parseFloat(m[1])) : null;
}

function makeActionState(id: string, lsKey: (id: string) => string): ActionState {
  let lastResult: HealthCheckResult | null = null;
  try {
    const saved = localStorage.getItem(lsKey(id));
    if (saved) lastResult = JSON.parse(saved) as HealthCheckResult;
  } catch { /* ignora */ }
  return { running: false, log: [], progress: null, lastResult, showLog: false, showDetails: false };
}

/**
 * @param actions  Lista de ações da página.
 * @param lsKeyPrefix  Prefixo do localStorage, ex: "frameguard:health" ou "frameguard:cleanup".
 */
export function useActionRunner(actions: ActionMeta[], lsKeyPrefix: string) {
  const lsKey = (id: string) => `${lsKeyPrefix}:${id}`;

  const [states, setStates] = useState<Record<string, ActionState>>(() => {
    const s: Record<string, ActionState> = {};
    for (const a of actions) s[a.id] = makeActionState(a.id, lsKey);
    return s;
  });

  const { isRunning, startTask, endTask } = useGlobalRunning();

  async function handleRun(meta: ActionMeta) {
    startTask(meta.id);
    setStates(prev => ({
      ...prev,
      [meta.id]: { ...prev[meta.id], running: true, log: [], progress: null, showLog: true },
    }));

    // Buffer local: acumula linhas entre flushes para evitar centenas de re-renders
    // por segundo durante comandos longos (causa UI freeze sem isso).
    let pendingLines: LogLine[] = [];
    let pendingProgress: number | null = null;

    // Flush a cada 80ms → máximo ~12 re-renders/s independente do volume de output
    const flushTimer = setInterval(() => {
      if (pendingLines.length === 0 && pendingProgress === null) return;
      const lines = pendingLines.splice(0);
      const p = pendingProgress;
      pendingProgress = null;
      setStates(prev => {
        const cur = prev[meta.id];
        // Limita a 500 linhas para não sobrecarregar o DOM
        const nextLog = lines.length > 0 ? [...cur.log, ...lines].slice(-500) : cur.log;
        return {
          ...prev,
          [meta.id]: { ...cur, log: nextLog, progress: p !== null ? p : cur.progress },
        };
      });
    }, 80);

    const unlisten = await listen<CommandEvent>(meta.eventChannel, event => {
      const { event_type, data } = event.payload;
      pendingLines.push({ type: event_type, text: data });
      if (event_type === 'stdout') {
        const pct = extractProgress(data);
        if (pct !== null) pendingProgress = pct;
      }
    });

    try {
      const result = await invoke<HealthCheckResult>(meta.command, meta.invokeArgs ?? {});
      clearInterval(flushTimer);
      const remaining = pendingLines.splice(0);
      try { localStorage.setItem(lsKey(meta.id), JSON.stringify(result)); } catch { /* ignora */ }
      setStates(prev => {
        const cur = prev[meta.id];
        const nextLog = remaining.length > 0 ? [...cur.log, ...remaining].slice(-500) : cur.log;
        return {
          ...prev,
          [meta.id]: { ...cur, running: false, progress: null, lastResult: result, showLog: true, log: nextLog },
        };
      });
      // Registra atividade (warning = parcialmente bem-sucedido, conta como sucesso)
      invoke('log_tweak_activity', {
        name: meta.name,
        applied: true,
        success: result.status === 'success' || result.status === 'warning',
      }).catch(() => {});
    } catch (e) {
      clearInterval(flushTimer);
      const remaining = pendingLines.splice(0);
      setStates(prev => {
        const cur = prev[meta.id];
        const nextLog = [...cur.log, ...remaining, { type: 'error', text: String(e) }].slice(-500);
        return { ...prev, [meta.id]: { ...cur, running: false, progress: null, log: nextLog } };
      });
      invoke('log_tweak_activity', { name: meta.name, applied: true, success: false }).catch(() => {});
    } finally {
      endTask(meta.id);
      unlisten();
    }
  }

  function toggleLog(id: string) {
    setStates(prev => ({ ...prev, [id]: { ...prev[id], showLog: !prev[id].showLog } }));
  }

  function toggleDetails(id: string) {
    setStates(prev => ({ ...prev, [id]: { ...prev[id], showDetails: !prev[id].showDetails } }));
  }

  return { states, handleRun, toggleLog, toggleDetails, isRunning };
}
