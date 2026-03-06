import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Cpu, Layers, Database, Globe, Clock } from 'lucide-react';
import { usePlanExecution, useHardwareFilter } from '../hooks';
import { useDashboardData } from '../hooks/useDashboardData';
import { useToast } from '../contexts/ToastContext';
import { showRestorePointToast } from '../utils/restorePoint';
import {
  ActivityItem,
  QuickPlanCard,
  QuickExecModal,
  AdminTag,
  StatusBadge,
  GameDvrBadge,
  PowerPlanBadge,
} from '../components/Dashboard';
import styles from './Dashboard.module.css';

export default function Dashboard() {
  const navigate = useNavigate();
  const { showToast } = useToast();
  const { executingPlan, execState, execute, closeModal, cleanup } = usePlanExecution();
  const { isCompatible } = useHardwareFilter();
  const data = useDashboardData(cleanup);
  const {
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
  } = data;

  // Toast para status do ponto de restauração durante execução de plano
  useEffect(() => {
    if (execState?.restorePoint) {
      showRestorePointToast(execState.restorePoint, showToast);
    }
  }, [execState?.restorePoint, showToast]);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>Dashboard</h1>
        <p className={styles.subtitle}>Visão geral do seu sistema</p>
      </header>

      {/* Grid 2x2 de informações */}
      <div className={styles.grid}>
        <HwCard
          icon={<Cpu size={17} strokeWidth={2} />}
          label="Processador"
          value={hw?.cpu_name}
          detail={cpuPercent !== null ? `${cpuPercent.toFixed(0)}% de uso atual` : undefined}
          loading={!hw}
        />

        <HwCard
          icon={<Layers size={17} strokeWidth={2} />}
          label="Placa de Vídeo"
          value={gpu?.gpu_name}
          detail={
            gpu ? (gpu.gpu_vram_gb > 0 ? `${gpu.gpu_vram_gb} GB VRAM` : 'GPU dedicada') : undefined
          }
          loading={!gpu}
        />

        {/* RAM — tem barra de progresso, então fica inline */}
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
                <div className={styles.progressFill} style={{ width: `${ramPercent ?? 0}%` }} />
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
            <GameDvrBadge dvrStatus={status?.game_dvr_status ?? 'available'} loading={!status} />
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
          {builtinPlans === null
            ? [1, 2, 3, 4].map((i) => (
                <div key={i} className={styles.quickPlanCard}>
                  <div
                    className={styles.skeleton}
                    style={{ width: 28, height: 28, borderRadius: 6, flexShrink: 0 }}
                  />
                  <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <div className={styles.skeleton} style={{ width: '65%' }} />
                    <div className={styles.skeleton} style={{ width: '35%', height: 12 }} />
                  </div>
                </div>
              ))
            : builtinPlans.map((plan) => (
                <QuickPlanCard
                  key={plan.id}
                  plan={plan}
                  onView={() => navigate(`/plans?viewPlan=${plan.id}`)}
                  onRun={() =>
                    execute(plan, () => {
                      showToast('success', 'Plano concluído', plan.name);
                      refreshActivity();
                    })
                  }
                  disabled={execState?.running ?? false}
                  isCompatible={isCompatible}
                />
              ))}
        </div>
        {builtinPlans !== null && (
          <button className={styles.viewAllLink} onClick={() => navigate('/plans')}>
            Ver todos os planos
          </button>
        )}
      </section>

      {/* Modal de execução (planos rápidos) */}
      {executingPlan && execState && (
        <QuickExecModal plan={executingPlan} state={execState} onClose={closeModal} />
      )}

      {/* Atividade Recente */}
      <section className={styles.activitySection}>
        <h2 className={styles.sectionTitle}>Atividade Recente</h2>
        {activity === null ? (
          <div className={styles.activityList}>
            {[1, 2, 3].map((i) => (
              <div key={i} className={styles.activityItem}>
                <div
                  className={styles.skeleton}
                  style={{ width: 15, height: 15, borderRadius: '50%', flexShrink: 0 }}
                />
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

// ── Helper para cards de hardware simples (CPU, GPU) ──────────────────────────

function HwCard({
  icon,
  label,
  value,
  detail,
  loading,
}: {
  icon: React.ReactNode;
  label: string;
  value?: string;
  detail?: string;
  loading: boolean;
}) {
  return (
    <div className={styles.card}>
      <div className={styles.cardHeader}>
        <div className={styles.iconWrap}>{icon}</div>
        <span className={styles.cardLabel}>{label}</span>
      </div>
      {!loading && value ? (
        <>
          <p className={styles.cardValue}>{value}</p>
          {detail && (
            <p className={styles.cardDetail}>
              <span className={styles.highlight}>{detail}</span>
            </p>
          )}
        </>
      ) : (
        <>
          <div className={styles.skeleton} style={{ width: '75%' }} />
          <div className={styles.skeleton} style={{ width: '40%', height: 14 }} />
        </>
      )}
    </div>
  );
}
