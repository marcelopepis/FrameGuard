import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import {
  Cpu, Layers, Database, Globe, ShieldCheck, ShieldOff,
  CheckCircle2, XCircle, AlertTriangle, Play, Zap, Undo2, Clock,
  HeartPulse, Gamepad2, Wrench, Lock,
  Loader2, X, ChevronDown, ChevronUp, MinusCircle,
} from 'lucide-react';
import { getStaticHwInfo, getGpuInfo, getSystemStatus, getSystemSummary, getSystemUsage } from '../services/systemInfo';
import type { StaticHwInfo, GpuInfo, SystemStatus, SystemSummary } from '../services/systemInfo';
import { usePlanExecution } from '../hooks';
import type { Plan, ExecState, ItemStatus } from '../hooks';
import { useToast } from '../contexts/ToastContext';
import styles from './Dashboard.module.css';

// ── Tipos de atividade ────────────────────────────────────────────────────────

interface ActivityEntry {
  timestamp: string;
  activity_type: 'plan_execution' | 'tweak_applied' | 'tweak_reverted';
  name: string;
  result: 'success' | 'partial' | 'failed';
  duration_seconds: number;
  completed_count: number | null;
  failed_count: number | null;
  skipped_count: number | null;
}

// ── Helpers de tempo relativo ────────────────────────────────────────────────

function timeAgo(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffSec = Math.floor((now - then) / 1000);

  if (diffSec < 60)   return 'agora mesmo';
  if (diffSec < 3600) return `há ${Math.floor(diffSec / 60)} min`;
  if (diffSec < 86400) return `há ${Math.floor(diffSec / 3600)}h`;
  if (diffSec < 172800) return 'ontem';
  return `há ${Math.floor(diffSec / 86400)} dias`;
}

function formatDuration(seconds: number): string {
  if (seconds === 0) return '';
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
}

// ── Componente principal ────────────────────────────────────────────────────

export default function Dashboard() {
  const [hw, setHw] = useState<StaticHwInfo | null>(null);
  const [gpu, setGpu] = useState<GpuInfo | null>(null);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [summary, setSummary] = useState<SystemSummary | null>(null);
  const [cpuPercent, setCpuPercent] = useState<number | null>(null);
  const [ramPercent, setRamPercent] = useState<number | null>(null);
  const [ramUsedGb, setRamUsedGb] = useState<number | null>(null);
  const [activity, setActivity] = useState<ActivityEntry[] | null>(null);
  const [builtinPlans, setBuiltinPlans] = useState<Plan[] | null>(null);

  const navigate = useNavigate();
  const { showToast } = useToast();
  const { executingPlan, execState, execute, closeModal, cleanup } = usePlanExecution();

  // Loading progressivo: cada chamada renderiza sua seção assim que os dados chegam
  // (os skeletons tratam o estado null de cada seção independentemente)
  useEffect(() => {
    getSystemSummary().then(setSummary).catch(() => {});
    invoke<ActivityEntry[]>('get_recent_activity', { limit: 5 }).then(setActivity).catch(() => {});
    invoke<Plan[]>('get_all_plans')
      .then(plans => setBuiltinPlans(plans.filter(p => p.builtin)))
      .catch(() => {});
    getStaticHwInfo().then(setHw).catch(() => {});
    getGpuInfo().then(setGpu).catch(() => {});
    getSystemStatus().then(setStatus).catch(() => {});

    return () => { cleanup(); };
  }, [cleanup]);

  // Refresh periódico de atividade e status (custo negligível: ~50 ms cada)
  // Necessário porque o Dashboard é always-mounted (keep-alive via display:none)
  useEffect(() => {
    const id = setInterval(() => {
      invoke<ActivityEntry[]>('get_recent_activity', { limit: 5 })
        .then(setActivity).catch(() => {});
      getSystemStatus().then(setStatus).catch(() => {});
    }, 10_000);
    return () => clearInterval(id);
  }, []);

  // Polling de CPU e RAM a cada 2 segundos
  useEffect(() => {
    const id = setInterval(() => {
      getSystemUsage().then(u => {
        setCpuPercent(u.cpu_usage_percent);
        setRamPercent(u.ram_usage_percent);
        if (hw) {
          setRamUsedGb(Math.round(u.ram_usage_percent / 100 * hw.ram_total_gb * 10) / 10);
        }
      }).catch(() => {});
    }, 2000);
    return () => clearInterval(id);
  }, [hw]);

  return (
    <div className={styles.page}>
      {/* Header */}
      <header className={styles.header}>
        <h1 className={styles.title}>Dashboard</h1>
        <p className={styles.subtitle}>Visão geral do seu sistema</p>
      </header>

      {/* Grid 2x2 de informações */}
      <div className={styles.grid}>
        {/* Processador */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Cpu size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Processador</span>
          </div>
          {hw ? (
            <>
              <p className={styles.cardValue}>{hw.cpu_name}</p>
              <p className={styles.cardDetail}>
                <span className={styles.highlight}>
                  {cpuPercent !== null ? `${cpuPercent.toFixed(0)}%` : '—'}
                </span>
                {' '}de uso atual
              </p>
            </>
          ) : (
            <>
              <div className={styles.skeleton} style={{ width: '75%' }} />
              <div className={styles.skeleton} style={{ width: '40%', height: 14 }} />
            </>
          )}
        </div>

        {/* Placa de Vídeo */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Layers size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Placa de Vídeo</span>
          </div>
          {gpu ? (
            <>
              <p className={styles.cardValue}>{gpu.gpu_name}</p>
              <p className={styles.cardDetail}>
                <span className={styles.highlight}>
                  {gpu.gpu_vram_gb > 0 ? `${gpu.gpu_vram_gb} GB VRAM` : 'GPU dedicada'}
                </span>
              </p>
            </>
          ) : (
            <>
              <div className={styles.skeleton} style={{ width: '65%' }} />
              <div className={styles.skeleton} style={{ width: '35%', height: 14 }} />
            </>
          )}
        </div>

        {/* Memória RAM */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Database size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Memória RAM</span>
          </div>
          {hw ? (
            <>
              <p className={styles.cardValue}>{ramUsedGb !== null ? `${ramUsedGb} GB` : '— GB'}</p>
              <p className={styles.cardDetail}>de {hw.ram_total_gb} GB total</p>
              <div className={styles.progressTrack}>
                <div
                  className={styles.progressFill}
                  style={{ width: `${ramPercent ?? 0}%` }}
                />
              </div>
              <p className={styles.progressLabel}>
                {ramPercent !== null ? `${ramPercent.toFixed(0)}% em uso` : '—'}
              </p>
            </>
          ) : (
            <>
              <div className={styles.skeleton} style={{ width: '30%' }} />
              <div className={styles.skeleton} style={{ width: '50%', height: 14 }} />
              <div className={styles.skeleton} style={{ width: '100%', height: 4, marginTop: 6 }} />
            </>
          )}
        </div>

        {/* Sistema */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Globe size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Sistema</span>
          </div>
          {summary ? (
            <>
              <p className={styles.cardValue}>{summary.os_version}</p>
              <p className={styles.cardDetail}>{summary.hostname}</p>
              <AdminTag elevated={summary.is_elevated} />
            </>
          ) : (
            <>
              <div className={styles.skeleton} style={{ width: '60%' }} />
              <div className={styles.skeleton} style={{ width: '35%', height: 14 }} />
            </>
          )}
        </div>
      </div>

      {/* Status Rápido */}
      <section className={styles.statusSection}>
        <h2 className={styles.sectionTitle}>Status Rápido</h2>
        <div className={styles.badgeRows}>
          <div className={styles.badges}>
            <StatusBadge
              label="Game Mode"
              optimized={status?.game_mode_enabled ?? false}
              goodLabel="Ativo"
              badLabel="Inativo"
              loading={!status}
              tooltip="Prioriza CPU e GPU para o jogo em execução, reduzindo interferência de processos em segundo plano. Recomendado: Ativo."
            />
            <StatusBadge
              label="HAGS"
              optimized={status?.hags_enabled ?? false}
              goodLabel="Ativo"
              badLabel="Inativo"
              loading={!status}
              tooltip="Hardware-Accelerated GPU Scheduling: a GPU gerencia sua própria memória, reduzindo latência e carga da CPU. Recomendado: Ativo."
            />
            <StatusBadge
              label="VBS"
              optimized={!(status?.vbs_enabled ?? true)}
              goodLabel="Desabilitado"
              badLabel="Habilitado"
              loading={!status}
              tooltip="Virtualization Based Security protege o Windows via virtualização, mas pode reduzir performance em games em até 10–15%. Recomendado: Desabilitado para gaming."
            />
            <GameDvrBadge
              dvrStatus={status?.game_dvr_status ?? 'available'}
              loading={!status}
            />
          </div>
          <div className={styles.badges}>
            <PowerPlanBadge
              tier={status?.power_plan_tier ?? 'other'}
              name={status?.power_plan_name ?? ''}
              loading={!status}
            />
            <StatusBadge
              label="Timer Res"
              optimized={status?.timer_resolution_optimized ?? false}
              goodLabel="1 ms"
              badLabel="Padrão"
              loading={!status}
              tooltip="Timer resolution de 1 ms (vs 15,6 ms padrão) melhora frame pacing e reduz input lag em monitores 144Hz+. Recomendado: Otimizado."
            />
          </div>
        </div>
      </section>

      {/* Planos Rápidos */}
      <section className={styles.quickPlansSection}>
        <h2 className={styles.sectionTitle}>Planos Rápidos</h2>
        <div className={styles.quickPlansGrid}>
          {builtinPlans === null ? (
            // Skeleton loading
            <>
              {[1, 2, 3, 4].map(i => (
                <div key={i} className={styles.quickPlanCard}>
                  <div className={styles.skeleton} style={{ width: 28, height: 28, borderRadius: 6, flexShrink: 0 }} />
                  <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <div className={styles.skeleton} style={{ width: '65%' }} />
                    <div className={styles.skeleton} style={{ width: '35%', height: 12 }} />
                  </div>
                </div>
              ))}
            </>
          ) : builtinPlans.map(plan => (
            <QuickPlanCard
              key={plan.id}
              plan={plan}
              onView={() => navigate(`/plans?viewPlan=${plan.id}`)}
              onRun={() => {
                execute(plan, () => {
                  showToast('success', 'Plano concluído', plan.name);
                  // Recarrega atividade recente
                  invoke<ActivityEntry[]>('get_recent_activity', { limit: 5 })
                    .then(setActivity).catch(() => {});
                });
              }}
              disabled={execState?.running ?? false}
            />
          ))}
        </div>
      </section>

      {/* Modal de execução (planos rápidos) */}
      {executingPlan && execState && (
        <QuickExecModal
          plan={executingPlan}
          state={execState}
          onClose={closeModal}
        />
      )}

      {/* Atividade Recente */}
      <section className={styles.activitySection}>
        <h2 className={styles.sectionTitle}>Atividade Recente</h2>
        {activity === null ? (
          <div className={styles.activityList}>
            {[1, 2, 3].map(i => (
              <div key={i} className={styles.activityItem}>
                <div className={styles.skeleton} style={{ width: 15, height: 15, borderRadius: '50%', flexShrink: 0 }} />
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
                  <div className={styles.skeleton} style={{ width: '60%' }} />
                  <div className={styles.skeleton} style={{ width: '35%', height: 12 }} />
                </div>
              </div>
            ))}
          </div>
        ) : activity.length === 0 ? (
          <div className={styles.activityEmpty}>
            <Clock size={16} strokeWidth={2} />
            <span>Nenhuma atividade registrada ainda</span>
          </div>
        ) : (
          <div className={styles.activityList}>
            {activity.map((entry, idx) => (
              <ActivityItem key={idx} entry={entry} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

// ─── ActivityItem ──────────────────────────────────────────

function ActivityItem({ entry }: { entry: ActivityEntry }) {
  const isPlan = entry.activity_type === 'plan_execution';
  const isRevert = entry.activity_type === 'tweak_reverted';

  // Ícone de tipo
  const TypeIcon = isPlan ? Play : isRevert ? Undo2 : Zap;

  // Ícone e classe de resultado
  const resultIcon = entry.result === 'success'
    ? <CheckCircle2 size={14} strokeWidth={2.5} className={styles.actResultSuccess} />
    : entry.result === 'partial'
      ? <AlertTriangle size={14} strokeWidth={2.5} className={styles.actResultPartial} />
      : <XCircle size={14} strokeWidth={2.5} className={styles.actResultFailed} />;

  // Resumo curto
  let detail = '';
  if (isPlan && entry.completed_count !== null) {
    const parts: string[] = [];
    if (entry.completed_count > 0)
      parts.push(`${entry.completed_count} concluído${entry.completed_count !== 1 ? 's' : ''}`);
    if (entry.failed_count && entry.failed_count > 0)
      parts.push(`${entry.failed_count} ${entry.failed_count === 1 ? 'falhou' : 'falharam'}`);
    if (entry.duration_seconds > 0)
      parts.push(formatDuration(entry.duration_seconds));
    detail = parts.join(' · ');
  } else if (isRevert) {
    detail = 'Revertido';
  } else {
    detail = entry.result === 'success' ? 'Aplicado' : 'Falhou';
  }

  return (
    <div className={styles.activityItem}>
      {resultIcon}
      <div className={styles.activityInfo}>
        <div className={styles.activityNameRow}>
          <TypeIcon size={12} strokeWidth={2} className={styles.activityTypeIcon} />
          <span className={styles.activityName}>{entry.name}</span>
        </div>
        {detail && <span className={styles.activityDetail}>{detail}</span>}
      </div>
      <span className={styles.activityTime}>{timeAgo(entry.timestamp)}</span>
    </div>
  );
}

// ─── QuickPlanCard ─────────────────────────────────────────

const PLAN_ICONS: Record<string, typeof HeartPulse> = {
  'Saúde': HeartPulse,
  'Gaming': Gamepad2,
  'Manutenção': Wrench,
  'Privacidade': Lock,
};

function guessPlanIcon(name: string) {
  for (const [keyword, Icon] of Object.entries(PLAN_ICONS)) {
    if (name.toLowerCase().includes(keyword.toLowerCase())) return Icon;
  }
  return Zap;
}

interface QuickPlanCardProps {
  plan: Plan;
  onView: () => void;
  onRun: () => void;
  disabled: boolean;
}

function QuickPlanCard({ plan, onView, onRun, disabled }: QuickPlanCardProps) {
  const Icon = guessPlanIcon(plan.name);
  const enabledCount = plan.items.filter(i => i.enabled).length;

  return (
    <div className={styles.quickPlanCard} onClick={onView} role="button" tabIndex={0}>
      <div className={styles.quickPlanIcon}>
        <Icon size={16} strokeWidth={2} />
      </div>
      <div className={styles.quickPlanInfo}>
        <span className={styles.quickPlanName}>{plan.name}</span>
        <span className={styles.quickPlanCount}>
          {enabledCount} {enabledCount === 1 ? 'item' : 'itens'}
        </span>
      </div>
      <button
        className={styles.quickPlanRun}
        onClick={(e) => { e.stopPropagation(); onRun(); }}
        disabled={disabled || enabledCount === 0}
        title="Executar plano"
      >
        <Play size={12} strokeWidth={2.5} fill="currentColor" />
      </button>
    </div>
  );
}

// ─── QuickExecModal (modal de execução compacto) ───────────

// Mapa de tweaks para nomes legíveis
const TWEAK_NAMES: Record<string, string> = {
  disable_wallpaper_compression: 'Desabilitar Compressão de Wallpaper',
  disable_reserved_storage: 'Recuperar Armazenamento Reservado',
  disable_delivery_optimization: 'Desabilitar Delivery Optimization',
  enable_hags: 'HAGS',
  enable_game_mode: 'Game Mode',
  disable_vbs: 'Desabilitar VBS',
  disable_game_dvr: 'Desabilitar Game DVR',
  disable_xbox_overlay: 'Desabilitar Xbox Overlay',
  enable_msi_mode_gpu: 'MSI Mode GPU',
  disable_mpo: 'Desabilitar MPO',
  disable_nvidia_telemetry: 'Desabilitar Telemetria NVIDIA',
  enable_timer_resolution: 'Timer 1 ms',
  disable_mouse_acceleration: 'Desabilitar Aceleração do Mouse',
  disable_fullscreen_optimizations: 'Desabilitar Fullscreen Optimizations',
  enable_ultimate_performance: 'Ultimate Performance',
  disable_power_throttling: 'Desabilitar Power Throttling',
  disable_hibernation: 'Desabilitar Hibernação',
  disable_ntfs_last_access: 'Desabilitar Timestamp NTFS',
  disable_nagle: 'Desabilitar Nagle',
  disable_sticky_keys: 'Desabilitar Teclas de Aderência',
  disable_bing_search: 'Desabilitar Bing no Menu Iniciar',
  disable_telemetry_registry: 'Desabilitar Telemetria Windows',
  disable_copilot: 'Desabilitar Copilot / Cortana',
  disable_content_delivery: 'Desabilitar Content Delivery',
  disable_background_apps: 'Desabilitar Apps em Background',
  flush_dns: 'Flush DNS',
  temp_cleanup: 'Limpeza de Temporários',
  dism_checkhealth: 'DISM — CheckHealth',
  dism_scanhealth: 'DISM — ScanHealth',
  dism_restorehealth: 'DISM — RestoreHealth',
  dism_cleanup: 'DISM — ComponentCleanup',
  sfc_scannow: 'SFC — System File Checker',
  chkdsk: 'Check Disk (C:)',
  ssd_trim: 'TRIM de SSDs',
};

function QuickExecModal({ plan, state, onClose }: { plan: Plan; state: ExecState; onClose: () => void }) {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const sortedItems = [...plan.items].sort((a, b) => a.order - b.order);
  const isDone = !state.running;

  function toggleExpand(id: string) {
    setExpandedItems(prev => {
      const s = new Set(prev);
      if (s.has(id)) s.delete(id); else s.add(id);
      return s;
    });
  }

  return (
    <div className={styles.execOverlay}>
      <div className={styles.execModal} onClick={e => e.stopPropagation()}>
        {/* Cabeçalho */}
        <div className={styles.execHeader}>
          <div className={styles.execTitleGroup}>
            <h2 className={styles.execTitle}>{plan.name}</h2>
            <p className={styles.execSubtitle}>
              {state.running
                ? `Executando… ${state.progress}%`
                : state.fatalError
                  ? 'Execução interrompida com erro'
                  : state.summary
                    ? `Concluído em ${formatDuration(state.summary.duration_seconds)}`
                    : 'Concluído'}
            </p>
          </div>
          <button
            className={styles.execClose}
            onClick={onClose}
            disabled={state.running}
            title={state.running ? 'Aguarde a conclusão' : 'Fechar'}
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* Barra de progresso */}
        <div className={styles.execProgress}>
          <div
            className={`${styles.execProgressFill} ${isDone && !state.fatalError ? styles.execProgressDone : ''}`}
            style={{ width: `${state.progress}%` }}
          />
        </div>

        {/* Lista de itens */}
        <div className={styles.execItemList}>
          {sortedItems.map(item => {
            const itemState = state.items[item.tweak_id] ?? { status: 'pending' as ItemStatus };
            const isExpanded = expandedItems.has(item.tweak_id);
            const hasDetails = !!(itemState.message || itemState.details || itemState.error);

            return (
              <div key={item.tweak_id} className={`${styles.execItemRow} ${styles[`execSt_${itemState.status}`] ?? ''}`}>
                <div className={styles.execItemMain}>
                  <ExecStatusIcon status={itemState.status} hcStatus={itemState.hcStatus} />
                  <div className={styles.execItemText}>
                    <span className={styles.execItemName}>
                      {TWEAK_NAMES[item.tweak_id] ?? item.tweak_id}
                    </span>
                    {itemState.message && !isExpanded && (
                      <span className={styles.execItemMsg}>{itemState.message}</span>
                    )}
                  </div>
                  {hasDetails && (
                    <button className={styles.execExpandBtn} onClick={() => toggleExpand(item.tweak_id)}>
                      {isExpanded
                        ? <ChevronUp size={13} strokeWidth={2} />
                        : <ChevronDown size={13} strokeWidth={2} />}
                    </button>
                  )}
                </div>
                {isExpanded && hasDetails && (
                  <div className={styles.execItemDetails}>
                    {itemState.error && <p className={styles.execError}>{itemState.error}</p>}
                    {itemState.message && <p className={styles.execMsg}>{itemState.message}</p>}
                    {itemState.details && <pre className={styles.execLog}>{itemState.details}</pre>}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Resumo final */}
        {isDone && state.summary && (
          <div className={styles.execFooter}>
            <div className={styles.execStats}>
              <span className={styles.execStatSuccess}>
                <CheckCircle2 size={13} strokeWidth={2.5} />
                {state.summary.completed_count}
              </span>
              {state.summary.failed_count > 0 && (
                <span className={styles.execStatFailed}>
                  <XCircle size={13} strokeWidth={2.5} />
                  {state.summary.failed_count}
                </span>
              )}
              {state.summary.skipped_count > 0 && (
                <span className={styles.execStatSkipped}>
                  <MinusCircle size={13} strokeWidth={2.5} />
                  {state.summary.skipped_count}
                </span>
              )}
            </div>
            <button className={styles.execCloseBtn} onClick={onClose}>Fechar</button>
          </div>
        )}

        {isDone && state.fatalError && (
          <div className={styles.execFooter}>
            <p className={styles.execError}>{state.fatalError}</p>
            <button className={styles.execCloseBtn} onClick={onClose}>Fechar</button>
          </div>
        )}
      </div>
    </div>
  );
}

function ExecStatusIcon({ status, hcStatus }: { status: ItemStatus; hcStatus?: 'success' | 'warning' | 'error' }) {
  if (status === 'pending')  return <Clock       size={14} strokeWidth={2} className={styles.stPending}   />;
  if (status === 'running')  return <Loader2     size={14} strokeWidth={2} className={styles.stRunning}   />;
  if (status === 'failed')   return <XCircle     size={14} strokeWidth={2} className={styles.stFailed}    />;
  if (status === 'skipped')  return <MinusCircle size={14} strokeWidth={2} className={styles.stSkipped}   />;
  if (hcStatus === 'warning') return <CheckCircle2 size={14} strokeWidth={2} className={styles.stWarning} />;
  if (hcStatus === 'error')   return <CheckCircle2 size={14} strokeWidth={2} className={styles.stFailed}  />;
  return <CheckCircle2 size={14} strokeWidth={2} className={styles.stCompleted} />;
}

// ─── AdminTag ──────────────────────────────────────────────

function AdminTag({ elevated }: { elevated: boolean }) {
  const Icon = elevated ? ShieldCheck : ShieldOff;
  return (
    <div className={`${styles.adminTag} ${elevated ? styles.adminOn : styles.adminOff}`}>
      <Icon size={11} strokeWidth={2.5} />
      <span>{elevated ? 'Administrador' : 'Sem elevação'}</span>
    </div>
  );
}

// ─── StatusBadge ───────────────────────────────────────────

interface StatusBadgeProps {
  label: string;
  optimized: boolean;
  goodLabel: string;
  badLabel: string;
  loading?: boolean;
  tooltip?: string;
}

function StatusBadge({ label, optimized, goodLabel, badLabel, loading, tooltip }: StatusBadgeProps) {
  return (
    <div className={`${styles.badge} ${loading ? '' : optimized ? styles.badgeOn : styles.badgeOff}`}>
      <span className={`${styles.dot} ${loading ? '' : optimized ? styles.dotOn : styles.dotOff}`} />
      <span className={styles.badgeLabel}>{label}</span>
      <span className={styles.badgeStatus}>
        {loading ? '…' : optimized ? goodLabel : badLabel}
      </span>
      {tooltip && <div className={styles.badgeTooltip}>{tooltip}</div>}
    </div>
  );
}

// ─── PowerPlanBadge (3 tiers) ─────────────────────────────

interface PowerPlanBadgeProps {
  tier: string;
  name: string;
  loading?: boolean;
}

// ─── GameDvrBadge (3 estados) ─────────────────────────────

interface GameDvrBadgeProps {
  dvrStatus: string;
  loading?: boolean;
}

function GameDvrBadge({ dvrStatus, loading }: GameDvrBadgeProps) {
  const badgeStyle = loading
    ? ''
    : dvrStatus === 'disabled'
      ? styles.badgeOn
      : dvrStatus === 'available'
        ? styles.badgeOff
        : styles.badgeError;

  const dotStyle = loading
    ? ''
    : dvrStatus === 'disabled'
      ? styles.dotOn
      : dvrStatus === 'available'
        ? styles.dotOff
        : styles.dotError;

  const statusText = loading
    ? '…'
    : dvrStatus === 'disabled'
      ? 'Desabilitado'
      : dvrStatus === 'available'
        ? 'Ativo (sem gravação)'
        : 'Gravação ativa';

  const tooltip = dvrStatus === 'disabled'
    ? 'O Game DVR está desabilitado. Encoder de GPU e buffer circular inativos.'
    : dvrStatus === 'available'
      ? 'O Game DVR está disponível mas a gravação em background está desligada. Sem impacto em performance.'
      : 'Gravação em background está ligada. Pode impactar FPS.';

  return (
    <div className={`${styles.badge} ${badgeStyle}`}>
      <span className={`${styles.dot} ${dotStyle}`} />
      <span className={styles.badgeLabel}>Game DVR</span>
      <span className={styles.badgeStatus}>{statusText}</span>
      <div className={styles.badgeTooltip}>{tooltip}</div>
    </div>
  );
}

function PowerPlanBadge({ tier, name, loading }: PowerPlanBadgeProps) {
  const tooltip = "O plano Ultimate Performance mantém o processador em frequência máxima, eliminando latência de boost. Recomendado: Ultimate Performance.";

  const badgeStyle = loading
    ? ''
    : tier === 'ultimate'
      ? styles.badgeOn
      : tier === 'high'
        ? styles.badgeOff
        : styles.badgeError;

  const dotStyle = loading
    ? ''
    : tier === 'ultimate'
      ? styles.dotOn
      : tier === 'high'
        ? styles.dotOff
        : styles.dotError;

  const statusText = loading
    ? '…'
    : tier === 'ultimate'
      ? name
      : tier === 'high'
        ? `${name} (bom, mas Ultimate é melhor)`
        : name;

  return (
    <div className={`${styles.badge} ${badgeStyle}`}>
      <span className={`${styles.dot} ${dotStyle}`} />
      <span className={styles.badgeLabel}>Power Plan</span>
      <span className={styles.badgeStatus}>{statusText}</span>
      <div className={styles.badgeTooltip}>{tooltip}</div>
    </div>
  );
}
