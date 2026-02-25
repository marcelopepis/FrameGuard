import { useState, useEffect } from 'react';
import { Cpu, Layers, Database, Globe, ShieldCheck, ShieldOff } from 'lucide-react';
import { getStaticHwInfo, getSystemStatus, getSystemSummary, getSystemUsage } from '../services/systemInfo';
import type { StaticHwInfo, SystemStatus, SystemSummary } from '../services/systemInfo';
import styles from './Dashboard.module.css';

export default function Dashboard() {
  const [hw, setHw] = useState<StaticHwInfo | null>(null);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [summary, setSummary] = useState<SystemSummary | null>(null);
  const [cpuPercent, setCpuPercent] = useState<number | null>(null);
  const [ramPercent, setRamPercent] = useState<number | null>(null);
  const [ramUsedGb, setRamUsedGb] = useState<number | null>(null);

  // Fetch paralelo: hardware + status + summary
  useEffect(() => {
    Promise.all([
      getStaticHwInfo(),
      getSystemStatus(),
      getSystemSummary(),
    ]).then(([hwInfo, sysStatus, sysSummary]) => {
      setHw(hwInfo);
      setStatus(sysStatus);
      setSummary(sysSummary);
    }).catch((err) => console.error('[FrameGuard] dashboard init falhou:', err));
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
          {hw ? (
            <>
              <p className={styles.cardValue}>{hw.gpu_name}</p>
              <p className={styles.cardDetail}>
                <span className={styles.highlight}>
                  {hw.gpu_vram_gb > 0 ? `${hw.gpu_vram_gb} GB VRAM` : 'GPU dedicada'}
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
            <StatusBadge
              label="Game DVR"
              optimized={status?.game_dvr_disabled ?? false}
              goodLabel="Desabilitado"
              badLabel="Habilitado"
              loading={!status}
              tooltip="Gravação em segundo plano do Game DVR consome GPU (encoder) e CPU mesmo quando você não está gravando. Recomendado: Desabilitado."
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
    </div>
  );
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
