// Página de Saúde do Sistema do FrameGuard.
//
// Agrupa as ações de manutenção e verificação do Windows em três seções:
// DISM (integridade do Component Store), Verificação de Disco e Manutenção.
// Cada ação exibe saída em tempo real via eventos Tauri com auto-scroll.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  ShieldCheck, Search, Wrench, Package,
  FileCheck, HardDrive, Zap,
  Globe, ChevronDown, ChevronUp, Loader2,
  CheckCircle2, XCircle, AlertTriangle,
  Play, RefreshCw,
} from 'lucide-react';
import type { LucideProps } from 'lucide-react';
import styles from './SystemHealth.module.css';

// ── Tipos ──────────────────────────────────────────────────────────────────────

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
  requiresInternet?: boolean;
  requiresRestart?: boolean;
  category: string;
}

interface Section {
  id: string;
  title: string;
  subtitle: string;
}

// ── Metadados estáticos das ações ──────────────────────────────────────────────

const SECTIONS: Section[] = [
  {
    id: 'dism',
    title: 'DISM — Component Store',
    subtitle: 'Integridade e reparo do repositório de componentes do Windows',
  },
  {
    id: 'verificacao',
    title: 'Verificação de Disco',
    subtitle: 'Integridade do sistema de arquivos e otimização de SSDs',
  },
];

const ACTIONS: ActionMeta[] = [
  // ── DISM ────────────────────────────────────────────────────────────────────
  {
    id: 'dism_checkhealth',
    name: 'DISM CheckHealth',
    Icon: ShieldCheck,
    description: 'Verificação rápida de integridade do Component Store. Consulta apenas metadados locais — sem downloads, sem reparos.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /CheckHealth

Consulta somente os metadados do Component Store (WinSxS), sem examinar os arquivos reais. É a verificação mais rápida e ideal para diagnóstico inicial.

Saídas possíveis:
• "No component store corruption detected" → Saudável
• "The component store is repairable"       → Corrupção detectada, use RestoreHealth
• "The component store is corrupted"        → Corrupção grave, reparo urgente`,
    estimatedDuration: '< 30 segundos',
    eventChannel: 'dism_checkhealth_progress',
    command: 'run_dism_checkhealth',
    category: 'dism',
  },
  {
    id: 'dism_scanhealth',
    name: 'DISM ScanHealth',
    Icon: Search,
    description: 'Varredura profunda do Component Store. Examina todos os arquivos em busca de corrupção, sem realizar reparos.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /ScanHealth

Mais abrangente que o CheckHealth: verifica os arquivos reais do WinSxS comparando com os manifestos do sistema. Pode levar vários minutos.

Não realiza reparos — apenas documenta os problemas. Se detectar corrupção, execute o RestoreHealth a seguir.`,
    estimatedDuration: '2–15 minutos',
    eventChannel: 'dism_scanhealth_progress',
    command: 'run_dism_scanhealth',
    category: 'dism',
  },
  {
    id: 'dism_restorehealth',
    name: 'DISM RestoreHealth',
    Icon: Wrench,
    description: 'Repara o Component Store baixando arquivos limpos do Windows Update. Substitui componentes corrompidos por versões íntegras.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /RestoreHealth

Baixa versões íntegras dos componentes corrompidos diretamente dos servidores da Microsoft via Windows Update, substituindo os arquivos danificados.

Recomendação: execute ScanHealth antes para confirmar a corrupção. Após o RestoreHealth, execute SFC /scannow para reparar arquivos de sistema individuais.

Requer conexão ativa com a internet.`,
    estimatedDuration: '5–30 minutos',
    eventChannel: 'dism_restorehealth_progress',
    command: 'run_dism_restorehealth',
    requiresInternet: true,
    category: 'dism',
  },
  {
    id: 'dism_cleanup',
    name: 'DISM StartComponentCleanup',
    Icon: Package,
    description: 'Remove componentes obsoletos de atualizações anteriores da pasta WinSxS, liberando espaço em disco.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /StartComponentCleanup

O Windows mantém cópias antigas dos componentes do sistema para permitir rollback de atualizações. Com o tempo, esse acúmulo pode ocupar vários GB em C:\\Windows\\WinSxS.

O StartComponentCleanup remove versões que não são mais necessárias, reduzindo o tamanho do Component Store.

O Windows 10/11 faz isso automaticamente via agendamento — este comando força a limpeza imediata.`,
    estimatedDuration: '1–10 minutos',
    eventChannel: 'dism_cleanup_progress',
    command: 'run_dism_cleanup',
    category: 'dism',
  },

  // ── Verificação ─────────────────────────────────────────────────────────────
  {
    id: 'sfc_scannow',
    name: 'SFC /scannow',
    Icon: FileCheck,
    description: 'Verifica e repara arquivos protegidos do Windows usando o cache local. Não requer conexão com a internet.',
    technicalDetails:
`Executa: sfc.exe /scannow (System File Checker)

Verifica a integridade de todos os arquivos protegidos do sistema e repara automaticamente os corrompidos usando o cache local (C:\\Windows\\System32\\dllcache).

Diferença entre SFC e DISM RestoreHealth:
• SFC usa cache local — mais rápido, sem internet, mas limitado ao cache disponível
• DISM usa Windows Update — mais abrangente, requer internet

Recomendação: execute DISM RestoreHealth primeiro para reconstruir o cache, depois SFC para reparar arquivos individuais.

O log completo fica em: C:\\Windows\\Logs\\CBS\\CBS.log`,
    estimatedDuration: '10–30 minutos',
    eventChannel: 'sfc_progress',
    command: 'run_sfc',
    category: 'verificacao',
  },
  {
    id: 'chkdsk',
    name: 'Check Disk (C:)',
    Icon: HardDrive,
    description: 'Verifica e corrige erros lógicos e físicos no disco C:. Se o disco estiver em uso, agenda a verificação para o próximo boot.',
    technicalDetails:
`Executa: chkdsk.exe C: /r

O flag /r implica /f (corrigir erros) e adiciona verificação de setores físicos defeituosos.

Comportamento no disco do sistema (C: em uso):
• O volume está bloqueado pelo Windows — chkdsk não consegue acessá-lo diretamente
• Uma confirmação "Y" é enviada automaticamente para agendar no próximo boot
• A verificação ocorre antes do Windows iniciar na próxima reinicialização

Exit codes: 0=sem erros, 1=erros corrigidos, 2=limpeza sugerida, 3=falha grave.`,
    estimatedDuration: 'Agendamento imediato / varia no boot',
    eventChannel: 'chkdsk_progress',
    command: 'run_chkdsk',
    invokeArgs: { driveLetter: null },
    requiresRestart: true,
    category: 'verificacao',
  },
  {
    id: 'ssd_trim',
    name: 'TRIM de SSDs',
    Icon: Zap,
    description: 'Executa TRIM em todos os SSDs detectados para manter performance de escrita e prolongar a vida útil do dispositivo.',
    technicalDetails:
`Usa PowerShell: Get-PhysicalDisk (SSD) + Optimize-Volume -ReTrim

O TRIM instrui o SSD a apagar internamente blocos marcados como não utilizados pelo sistema de arquivos. Sem TRIM, blocos "sujos" se acumulam e degradam a performance de escrita progressivamente.

O Windows executa TRIM automaticamente via Scheduled Tasks, mas executar manualmente garante que todos os SSDs estejam otimizados agora.

Apenas SSDs são processados — HDDs são detectados e ignorados automaticamente.`,
    estimatedDuration: '30 segundos–2 minutos',
    eventChannel: 'trim_progress',
    command: 'run_ssd_trim',
    category: 'verificacao',
  },

];

// ── Utilitários ────────────────────────────────────────────────────────────────

/** Extrai percentual de linhas DISM como " 42.0%" → 42 */
function extractProgress(text: string): number | null {
  const m = text.trim().match(/^(\d+(?:\.\d+)?)%$/);
  return m ? Math.min(100, parseFloat(m[1])) : null;
}

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

const LS_KEY = (id: string) => `frameguard:health:${id}`;

function makeActionState(id: string): ActionState {
  let lastResult: HealthCheckResult | null = null;
  try {
    const saved = localStorage.getItem(LS_KEY(id));
    if (saved) lastResult = JSON.parse(saved) as HealthCheckResult;
  } catch { /* ignora */ }
  return { running: false, log: [], progress: null, lastResult, showLog: false, showDetails: false };
}

// ── Subcomponente ActionCard ────────────────────────────────────────────────────

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

          {/* Saiba mais */}
          <button className={styles.btnDetails} onClick={onToggleDetails}>
            {state.showDetails ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            {state.showDetails ? 'Menos detalhes' : 'Saiba mais'}
          </button>

          {state.showDetails && (
            <div className={styles.detailsPanel}>
              <pre className={styles.detailsText}>{meta.technicalDetails}</pre>
            </div>
          )}

          {/* Badges e duração estimada */}
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
            <span className={styles.metaDuration}>
              ⏱ {meta.estimatedDuration}
            </span>
          </div>
        </div>
      </div>

      {/* ── Divisor ── */}
      <div className={styles.cardDivider} />

      {/* ── Rodapé: resultado + botões ── */}
      <div className={styles.cardBottom}>
        <div className={styles.resultArea}>
          {state.running ? (
            /* Estado de execução com barra de progresso */
            <div className={styles.runningState}>
              <div className={styles.progressTrack}>
                {state.progress !== null ? (
                  <div
                    className={styles.progressFill}
                    style={{ width: `${state.progress}%` }}
                  />
                ) : (
                  <div className={styles.progressIndeterminate} />
                )}
              </div>
              <span className={styles.progressLabel}>
                {state.progress !== null ? `${state.progress.toFixed(0)}%` : 'Executando...'}
              </span>
            </div>
          ) : result ? (
            /* Resultado da última execução */
            <div className={`${styles.resultRow} ${statusClass}`}>
              {StatusIcon && <StatusIcon size={14} className={styles.resultIcon} />}
              <div className={styles.resultContent}>
                <span className={styles.resultMessage}>{result.message}</span>
                <div className={styles.resultMeta}>
                  <span className={styles.resultDuration}>
                    {formatDuration(result.duration_seconds)}
                  </span>
                  {result.space_freed_mb !== null && result.space_freed_mb > 0 && (
                    <span className={styles.resultSpace}>
                      {formatSpaceFreed(result.space_freed_mb)} liberados
                    </span>
                  )}
                  <span className={styles.resultTimestamp}>
                    {formatDate(result.timestamp)}
                  </span>
                </div>
              </div>
            </div>
          ) : (
            /* Estado inicial — nunca executado */
            <span className={styles.idleState}>Nunca executado</span>
          )}
        </div>

        {/* Controles direitos */}
        <div className={styles.cardControls}>
          {/* Botão "Ver log" (visível após execução) */}
          {result && !state.running && (
            <button className={styles.btnLog} onClick={onToggleLog}>
              {state.showLog ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              {state.showLog ? 'Ocultar log' : 'Ver log'}
            </button>
          )}

          {/* Botão Executar */}
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

      {/* ── Área de log (tempo real ou histórico) ── */}
      {(state.running || state.showLog) && state.log.length > 0 && (
        <div className={styles.logArea} ref={logRef}>
          {state.log.map((line, i) => (
            <div
              key={i}
              className={`${styles.logLine} ${
                line.type === 'stderr'    ? styles.logStderr  :
                line.type === 'started'  ? styles.logSystem  :
                line.type === 'completed'? styles.logSystem  :
                line.type === 'error'    ? styles.logError   :
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

// ── Componente principal ───────────────────────────────────────────────────────

export default function SystemHealth() {
  const [states, setStates] = useState<Record<string, ActionState>>(() => {
    const s: Record<string, ActionState> = {};
    for (const a of ACTIONS) s[a.id] = makeActionState(a.id);
    return s;
  });

  const isAnyRunning = Object.values(states).some(a => a.running);

  function updateAction(id: string, updates: Partial<ActionState>) {
    setStates(prev => ({
      ...prev,
      [id]: { ...prev[id], ...updates },
    }));
  }

  async function handleRun(meta: ActionMeta) {
    updateAction(meta.id, { running: true, log: [], progress: null, showLog: true });

    // Buffer local: acumula linhas entre flushes para evitar centenas de re-renders
    // por segundo durante comandos longos como DISM ScanHealth (causa UI freeze).
    let pendingLines: LogLine[] = [];
    let pendingProgress: number | null = null;

    // Flush a cada 80 ms → máximo ~12 re-renders/s independente do volume de output
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
    setStates(prev => ({
      ...prev,
      [id]: { ...prev[id], showLog: !prev[id].showLog },
    }));
  }

  function toggleDetails(id: string) {
    setStates(prev => ({
      ...prev,
      [id]: { ...prev[id], showDetails: !prev[id].showDetails },
    }));
  }

  // ── Render ──

  return (
    <div className={styles.page}>

      {/* ── Header ── */}
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Saúde do Sistema</h1>
          <p className={styles.subtitle}>Manutenção e integridade do Windows</p>
        </div>
      </div>

      {/* ── Seções de ações ── */}
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
