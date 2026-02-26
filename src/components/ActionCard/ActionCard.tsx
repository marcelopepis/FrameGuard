// Componente ActionCard compartilhado entre as páginas de Saúde do Sistema e Limpeza.
//
// Recebe os metadados e o estado da ação via props e renderiza o card completo:
// ícone, descrição, detalhes técnicos expansíveis, barra de progresso, log em
// tempo real e botão de execução.

import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Globe, RefreshCw,
  ChevronDown, ChevronUp, Loader2,
  CheckCircle2, XCircle, AlertTriangle, Play,
  Lock, X as XIcon,
} from 'lucide-react';
import styles from './ActionCard.module.css';
import type { ActionMeta, ActionState, LockingProcessInfo } from '../../types/health';
import { formatDate, formatDuration, formatSpaceFreed } from '../../utils/formatters';

interface ActionCardProps {
  meta: ActionMeta;
  state: ActionState;
  onRun: () => void;
  onToggleLog: () => void;
  onToggleDetails: () => void;
  disabled?: boolean;
}

export function ActionCard({ meta, state, onRun, onToggleLog, onToggleDetails, disabled }: ActionCardProps) {
  const logRef = useRef<HTMLDivElement>(null);

  // Auto-scroll para a última linha conforme o log cresce
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [state.log]);

  const { Icon } = meta;
  const result = state.lastResult;

  const StatusIcon = result
    ? { success: CheckCircle2, warning: AlertTriangle, error: XCircle }[result.status]
    : null;

  const statusClass = result
    ? { success: styles.statusSuccess, warning: styles.statusWarning, error: styles.statusError }[result.status]
    : '';

  return (
    <div className={`${styles.actionCard} ${state.running ? styles.actionCardRunning : ''}`}>

      {/* ── Topo: ícone + info ── */}
      <div className={styles.cardTop}>
        <div className={styles.cardIcon}>
          <Icon size={16} />
        </div>

        <div className={styles.cardInfo}>
          <div className={styles.cardName}>{meta.name}</div>
          <p className={styles.cardDesc}>{meta.description}</p>

          <button className={styles.btnDetails} onClick={onToggleDetails}>
            {state.showDetails ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            {state.showDetails ? 'Menos detalhes' : 'Saiba mais'}
          </button>

          {state.showDetails && (
            <div className={styles.detailsPanel}>
              <pre className={styles.detailsText}>{meta.technicalDetails}</pre>
            </div>
          )}

          <div className={styles.cardMeta}>
            {meta.requiresInternet && (
              <span className={styles.metaBadgeInternet}>
                <Globe size={10} /> Requer internet
              </span>
            )}
            {meta.requiresRestart && (
              <span className={styles.metaBadgeRestart}>
                <RefreshCw size={10} /> Pode reinicializar
              </span>
            )}
            <span className={styles.metaDuration}>⏱ {meta.estimatedDuration}</span>
          </div>
        </div>
      </div>

      {/* ── Divisor ── */}
      <div className={styles.cardDivider} />

      {/* ── Rodapé: resultado + botões ── */}
      <div className={styles.cardBottom}>
        <div className={styles.resultArea}>
          {state.running ? (
            <div className={styles.runningState}>
              <div className={styles.progressTrack}>
                {state.progress !== null ? (
                  <div className={styles.progressFill} style={{ width: `${state.progress}%` }} />
                ) : (
                  <div className={styles.progressIndeterminate} />
                )}
              </div>
              <span className={styles.progressLabel}>
                {state.progress !== null ? `${state.progress.toFixed(0)}%` : 'Executando...'}
              </span>
            </div>
          ) : result ? (
            <div className={`${styles.resultRow} ${statusClass}`}>
              {StatusIcon && <StatusIcon size={14} className={styles.resultIcon} />}
              <div className={styles.resultContent}>
                <span className={styles.resultMessage}>{result.message}</span>
                <div className={styles.resultMeta}>
                  <span className={styles.resultDuration}>{formatDuration(result.duration_seconds)}</span>
                  {result.space_freed_mb !== null && result.space_freed_mb > 0 && (
                    <span className={styles.resultSpace}>
                      {formatSpaceFreed(result.space_freed_mb)} liberados
                    </span>
                  )}
                  <span className={styles.resultTimestamp}>{formatDate(result.timestamp)}</span>
                </div>
              </div>
            </div>
          ) : (
            <span className={styles.idleState}>Nunca executado</span>
          )}
        </div>

        <div className={styles.cardControls}>
          {result && !state.running && (
            <button className={styles.btnLog} onClick={onToggleLog}>
              {state.showLog ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              {state.showLog ? 'Ocultar log' : 'Ver log'}
            </button>
          )}

          {state.running ? (
            <div className={styles.runningBadge}>
              <Loader2 size={13} className={styles.spinner} />
              <span>Executando</span>
            </div>
          ) : (
            <div className={styles.btnRunWrap}>
              <button className={styles.btnRun} onClick={!disabled ? onRun : undefined} disabled={disabled}>
                <Play size={13} />
                Executar
              </button>
              {disabled && (
                <div className={styles.btnRunTip}>
                  Outro comando em execução.<br />Aguarde a conclusão.
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* ── Processos travando arquivos (apenas temp_cleanup) ── */}
      {!state.running && result?.locking_processes && result.locking_processes.length > 0 && (
        <LockingProcessesPanel
          processes={result.locking_processes}
          onRetry={onRun}
          disabled={disabled}
        />
      )}

      {/* ── Área de log (tempo real ou histórico) ── */}
      {(state.running || state.showLog) && state.log.length > 0 && (
        <div className={styles.logArea} ref={logRef}>
          {state.log.map((line, i) => (
            <div
              key={i}
              className={`${styles.logLine} ${
                line.type === 'stderr'     ? styles.logStderr  :
                line.type === 'started'   ? styles.logSystem  :
                line.type === 'completed' ? styles.logSystem  :
                line.type === 'error'     ? styles.logError   :
                styles.logStdout
              }`}
            >
              {line.type === 'started' ? `$ ${line.text}` : line.text}
            </div>
          ))}
          {state.running && (
            <div className={`${styles.logLine} ${styles.logCursor}`}>▋</div>
          )}
        </div>
      )}
    </div>
  );
}

// ─── Painel de processos que travam arquivos ────────────────────────────────

function LockingProcessesPanel({
  processes, onRetry, disabled,
}: {
  processes: LockingProcessInfo[];
  onRetry: () => void;
  disabled?: boolean;
}) {
  const [killing, setKilling] = useState<Set<number>>(new Set());
  const [killed, setKilled] = useState<Set<number>>(new Set());
  const [errors, setErrors] = useState<Map<number, string>>(new Map());

  async function handleKill(pid: number) {
    setKilling(prev => new Set(prev).add(pid));
    setErrors(prev => { const m = new Map(prev); m.delete(pid); return m; });

    try {
      await invoke<string>('kill_process', { pid });
      setKilled(prev => new Set(prev).add(pid));
    } catch (e) {
      setErrors(prev => new Map(prev).set(pid, String(e)));
    } finally {
      setKilling(prev => { const s = new Set(prev); s.delete(pid); return s; });
    }
  }

  const allKilled = processes.every(p => killed.has(p.pid));

  return (
    <div className={styles.lockPanel}>
      <div className={styles.lockHeader}>
        <Lock size={13} strokeWidth={2.5} />
        <span>Processos travando arquivos</span>
      </div>

      <div className={styles.lockList}>
        {processes.map(p => (
          <div key={p.pid} className={`${styles.lockRow} ${killed.has(p.pid) ? styles.lockRowKilled : ''}`}>
            <div className={styles.lockInfo}>
              <span className={styles.lockName}>{p.name}</span>
              <span className={styles.lockMeta}>PID {p.pid} · {p.file_count} arquivo{p.file_count !== 1 ? 's' : ''}</span>
            </div>

            {killed.has(p.pid) ? (
              <span className={styles.lockKilledBadge}>
                <CheckCircle2 size={11} strokeWidth={2.5} />
                Encerrado
              </span>
            ) : killing.has(p.pid) ? (
              <span className={styles.lockKillingBadge}>
                <Loader2 size={11} className={styles.spinner} />
                Encerrando...
              </span>
            ) : (
              <button
                className={styles.lockKillBtn}
                onClick={() => handleKill(p.pid)}
                title={`Encerrar ${p.name}`}
              >
                <XIcon size={11} strokeWidth={2.5} />
                Encerrar
              </button>
            )}

            {errors.has(p.pid) && (
              <span className={styles.lockError}>{errors.get(p.pid)}</span>
            )}
          </div>
        ))}
      </div>

      {allKilled && (
        <button
          className={styles.lockRetryBtn}
          onClick={onRetry}
          disabled={disabled}
        >
          <Play size={12} strokeWidth={2.5} />
          Executar limpeza novamente
        </button>
      )}
    </div>
  );
}
