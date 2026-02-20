// Página de Limpeza do FrameGuard.
//
// Concentra as ações de limpeza e manutenção pontual do sistema:
// Flush DNS e Limpeza de Temporários. Cada ação exibe saída em
// tempo real via eventos Tauri com auto-scroll e persiste o
// resultado da última execução via localStorage.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Globe, Trash2,
  ChevronDown, ChevronUp, Loader2,
  CheckCircle2, XCircle, AlertTriangle,
  Play,
} from 'lucide-react';
import type { LucideProps } from 'lucide-react';
import styles from './SystemHealth.module.css';

// ── Tipos ───────────────────────────────────────────────────────────────────

interface CommandEvent {
  event_type: 'started' | 'stdout' | 'stderr' | 'completed' | 'error';
  data: string;
  timestamp: string;
}

interface HealthCheckResult {
  id: string;
  name: string;
  status: 'success' | 'warning' | 'error';
  message: string;
  details: string;
  duration_seconds: number;
  space_freed_mb: number | null;
  timestamp: string;
}

interface LogLine {
  type: string;
  text: string;
}

interface ActionState {
  running: boolean;
  log: LogLine[];
  progress: number | null;
  lastResult: HealthCheckResult | null;
  showLog: boolean;
  showDetails: boolean;
}

interface ActionMeta {
  id: string;
  name: string;
  Icon: React.ComponentType<LucideProps>;
  description: string;
  technicalDetails: string;
  estimatedDuration: string;
  eventChannel: string;
  command: string;
  invokeArgs?: Record<string, unknown>;
  category: string;
}

interface Section {
  id: string;
  title: string;
  subtitle: string;
}

// ── Metadados das ações ─────────────────────────────────────────────────────

const SECTIONS: Section[] = [
  {
    id: 'limpeza',
    title: 'Limpeza e Manutenção',
    subtitle: 'Limpeza rápida de cache e arquivos temporários do sistema',
  },
];

const ACTIONS: ActionMeta[] = [
  {
    id: 'flush_dns',
    name: 'Flush DNS',
    Icon: Globe,
    description: 'Limpa o cache DNS local. Resolve problemas de conectividade causados por entradas desatualizadas ou corrompidas.',
    technicalDetails:
`Executa: ipconfig.exe /flushdns

O cache DNS local armazena resoluções de nomes recentes (ex: "google.com → 142.250.x.x") para acelerar conexões. Pode ficar desatualizado após mudanças de DNS ou conter entradas corrompidas que causam falhas de conexão.

O flush força o Windows a consultar os servidores DNS configurados na próxima requisição, garantindo endereços atualizados.

Útil para: sites que não carregam após mudança de DNS, troca de provedor, ou alterações no arquivo hosts.`,
    estimatedDuration: '< 1 segundo',
    eventChannel: 'dns_flush_progress',
    command: 'flush_dns',
    category: 'limpeza',
  },
  {
    id: 'temp_cleanup',
    name: 'Limpeza de Temporários',
    Icon: Trash2,
    description: 'Remove arquivos temporários de %TEMP%, Windows\\Temp e do cache do Windows Update. Arquivos em uso são ignorados.',
    technicalDetails:
`Remove arquivos de três locais:

• %TEMP%                                   — Temporários do usuário atual (instaladores, extrações, caches)
• C:\\Windows\\Temp                         — Temporários do sistema e serviços Windows
• C:\\Windows\\SoftwareDistribution\\Download — Cache do Windows Update (atualizações já instaladas)

Arquivos em uso são pulados silenciosamente. A pasta SoftwareDistribution\\Download é recriada automaticamente pelo Windows Update quando necessário.

O espaço liberado é calculado com precisão comparando o tamanho antes e depois da remoção.`,
    estimatedDuration: '30 segundos–3 minutos',
    eventChannel: 'temp_cleanup_progress',
    command: 'run_temp_cleanup',
    category: 'limpeza',
  },
];

// ── Utilitários ─────────────────────────────────────────────────────────────

const LS_KEY = (id: string) => `frameguard:cleanup:${id}`;

function formatDuration(secs: number): string {
  if (secs < 1) return '< 1s';
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  });
}

function formatSpaceFreed(mb: number): string {
  if (mb < 1024) return `${mb} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

function makeActionState(id: string): ActionState {
  let lastResult: HealthCheckResult | null = null;
  try {
    const saved = localStorage.getItem(LS_KEY(id));
    if (saved) lastResult = JSON.parse(saved) as HealthCheckResult;
  } catch { /* ignora */ }
  return { running: false, log: [], progress: null, lastResult, showLog: false, showDetails: false };
}

// ── ActionCard ───────────────────────────────────────────────────────────────

interface ActionCardProps {
  meta: ActionMeta;
  state: ActionState;
  onRun: () => void;
  onToggleLog: () => void;
  onToggleDetails: () => void;
  disabled?: boolean;
}

function ActionCard({ meta, state, onRun, onToggleLog, onToggleDetails, disabled }: ActionCardProps) {
  const logRef = useRef<HTMLDivElement>(null);

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

      {/* ── Topo ── */}
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
            <span className={styles.metaDuration}>⏱ {meta.estimatedDuration}</span>
          </div>
        </div>
      </div>

      {/* ── Divisor ── */}
      <div className={styles.cardDivider} />

      {/* ── Rodapé ── */}
      <div className={styles.cardBottom}>
        <div className={styles.resultArea}>
          {state.running ? (
            <div className={styles.runningState}>
              <div className={styles.progressTrack}>
                <div className={styles.progressIndeterminate} />
              </div>
              <span className={styles.progressLabel}>Executando...</span>
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
            <button className={styles.btnRun} onClick={onRun} disabled={disabled}>
              <Play size={13} />
              Executar
            </button>
          )}
        </div>
      </div>

      {/* ── Log ── */}
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

// ── Componente principal ────────────────────────────────────────────────────

export default function Cleanup() {
  const [states, setStates] = useState<Record<string, ActionState>>(() => {
    const s: Record<string, ActionState> = {};
    for (const a of ACTIONS) s[a.id] = makeActionState(a.id);
    return s;
  });

  const isAnyRunning = Object.values(states).some(a => a.running);

  function updateAction(id: string, updates: Partial<ActionState>) {
    setStates(prev => ({ ...prev, [id]: { ...prev[id], ...updates } }));
  }
  // mantém updateAction referenciado para evitar warning de linter
  void updateAction;

  async function handleRun(meta: ActionMeta) {
    setStates(prev => ({
      ...prev,
      [meta.id]: { ...prev[meta.id], running: true, log: [], progress: null, showLog: true },
    }));

    let pendingLines: LogLine[] = [];

    const flushTimer = setInterval(() => {
      if (pendingLines.length === 0) return;
      const lines = pendingLines.splice(0);
      setStates(prev => {
        const cur = prev[meta.id];
        const nextLog = [...cur.log, ...lines].slice(-500);
        return { ...prev, [meta.id]: { ...cur, log: nextLog } };
      });
    }, 80);

    const unlisten = await listen<CommandEvent>(meta.eventChannel, event => {
      const { event_type, data } = event.payload;
      pendingLines.push({ type: event_type, text: data });
    });

    try {
      const result = await invoke<HealthCheckResult>(meta.command, meta.invokeArgs ?? {});
      clearInterval(flushTimer);
      const remaining = pendingLines.splice(0);
      try { localStorage.setItem(LS_KEY(meta.id), JSON.stringify(result)); } catch { /* ignora */ }
      setStates(prev => {
        const cur = prev[meta.id];
        const nextLog = remaining.length > 0 ? [...cur.log, ...remaining].slice(-500) : cur.log;
        return {
          ...prev,
          [meta.id]: { ...cur, running: false, progress: null, lastResult: result, showLog: true, log: nextLog },
        };
      });
    } catch (e) {
      clearInterval(flushTimer);
      const remaining = pendingLines.splice(0);
      setStates(prev => {
        const cur = prev[meta.id];
        const nextLog = [...cur.log, ...remaining, { type: 'error', text: String(e) }].slice(-500);
        return { ...prev, [meta.id]: { ...cur, running: false, progress: null, log: nextLog } };
      });
    } finally {
      unlisten();
    }
  }

  function toggleLog(id: string) {
    setStates(prev => ({ ...prev, [id]: { ...prev[id], showLog: !prev[id].showLog } }));
  }

  function toggleDetails(id: string) {
    setStates(prev => ({ ...prev, [id]: { ...prev[id], showDetails: !prev[id].showDetails } }));
  }

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Limpeza</h1>
          <p className={styles.subtitle}>Limpeza e manutenção do sistema</p>
        </div>
      </div>

      <div className={styles.sections}>
        {SECTIONS.map(section => {
          const sectionActions = ACTIONS.filter(a => a.category === section.id);
          return (
            <div key={section.id} className={styles.section}>
              <div className={styles.sectionHeader}>
                <span className={styles.sectionTitle}>{section.title}</span>
                <span className={styles.sectionSubtitle}>{section.subtitle}</span>
              </div>

              <div className={styles.actionList}>
                {sectionActions.map(meta => (
                  <ActionCard
                    key={meta.id}
                    meta={meta}
                    state={states[meta.id]}
                    onRun={() => handleRun(meta)}
                    onToggleLog={() => toggleLog(meta.id)}
                    onToggleDetails={() => toggleDetails(meta.id)}
                    disabled={isAnyRunning && !states[meta.id].running}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
