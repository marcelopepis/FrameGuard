import { useState, useEffect } from 'react';
import { Cpu, Layers, Database, Globe, ShieldCheck, ShieldOff } from 'lucide-react';
import { getSystemInfo, getSystemSummary, getSystemUsage } from '../services/systemInfo';
import type { SystemInfo, SystemSummary } from '../services/systemInfo';
import styles from './Dashboard.module.css';

export default function Dashboard() {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [summary, setSummary] = useState<SystemSummary | null>(null);

  useEffect(() => {
    getSystemInfo()
      .then(setInfo)
      .catch((err) => console.error('[FrameGuard] get_system_info falhou:', err));

    getSystemSummary()
      .then(setSummary)
      .catch((err) => console.error('[FrameGuard] get_system_summary falhou:', err));
  }, []);

  // Polling de CPU e RAM a cada 2 segundos
  useEffect(() => {
    const id = setInterval(() => {
      getSystemUsage().then(u => {
        setInfo(prev => {
          if (!prev) return prev;
          const ram_used_gb = Math.round(u.ram_usage_percent / 100 * prev.ram_total_gb * 10) / 10;
          return { ...prev, cpu_usage_percent: u.cpu_usage_percent, ram_usage_percent: u.ram_usage_percent, ram_used_gb };
        });
      }).catch(() => {});
    }, 2000);
    return () => clearInterval(id);
  }, []);

  const loading = <span className={styles.loading}>Carregando…</span>;

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
          <p className={styles.cardValue}>{info ? info.cpu_name : loading}</p>
          <p className={styles.cardDetail}>
            {info ? (
              <>
                <span className={styles.highlight}>{info.cpu_usage_percent.toFixed(0)}%</span>
                {' '}de uso atual
              </>
            ) : (
              <>
                <span className={styles.highlight}>—</span>
                {' '}de uso atual
              </>
            )}
          </p>
        </div>

        {/* Placa de Vídeo */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Layers size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Placa de Vídeo</span>
          </div>
          <p className={styles.cardValue}>{info ? info.gpu_name : loading}</p>
          <p className={styles.cardDetail}>
            {info ? (
              <span className={styles.highlight}>
                {info.gpu_vram_gb > 0 ? `${info.gpu_vram_gb} GB VRAM` : 'GPU dedicada'}
              </span>
            ) : '—'}
          </p>
        </div>

        {/* Memória RAM */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Database size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Memória RAM</span>
          </div>
          <p className={styles.cardValue}>{info ? `${info.ram_used_gb} GB` : loading}</p>
          <p className={styles.cardDetail}>de {info ? info.ram_total_gb : '—'} GB total</p>
          <div className={styles.progressTrack}>
            <div
              className={styles.progressFill}
              style={{ width: `${info ? info.ram_usage_percent : 0}%` }}
            />
          </div>
          <p className={styles.progressLabel}>
            {info ? `${info.ram_usage_percent.toFixed(0)}% em uso` : '—'}
          </p>
        </div>

        {/* Sistema — dados reais via Tauri */}
        <div className={styles.card}>
          <div className={styles.cardHeader}>
            <div className={styles.iconWrap}>
              <Globe size={17} strokeWidth={2} />
            </div>
            <span className={styles.cardLabel}>Sistema</span>
          </div>
          <p className={styles.cardValue}>
            {summary ? summary.os_version : loading}
          </p>
          <p className={styles.cardDetail}>
            {summary ? summary.hostname : '—'}
          </p>
          {summary && (
            <AdminTag elevated={summary.is_elevated} />
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
              optimized={info?.game_mode_enabled ?? false}
              goodLabel="Ativo"
              badLabel="Inativo"
              loading={!info}
              tooltip="Prioriza CPU e GPU para o jogo em execução, reduzindo interferência de processos em segundo plano. Recomendado: Ativo."
            />
            <StatusBadge
              label="HAGS"
              optimized={info?.hags_enabled ?? false}
              goodLabel="Ativo"
              badLabel="Inativo"
              loading={!info}
              tooltip="Hardware-Accelerated GPU Scheduling: a GPU gerencia sua própria memória, reduzindo latência e carga da CPU. Recomendado: Ativo."
            />
            <StatusBadge
              label="VBS"
              optimized={!(info?.vbs_enabled ?? true)}
              goodLabel="Desabilitado"
              badLabel="Habilitado"
              loading={!info}
              tooltip="Virtualization Based Security protege o Windows via virtualização, mas pode reduzir performance em games em até 10–15%. Recomendado: Desabilitado para gaming."
            />
            <StatusBadge
              label="Game DVR"
              optimized={info?.game_dvr_disabled ?? false}
              goodLabel="Desabilitado"
              badLabel="Habilitado"
              loading={!info}
              tooltip="Gravação em segundo plano do Game DVR consome GPU (encoder) e CPU mesmo quando você não está gravando. Recomendado: Desabilitado."
            />
          </div>
          <div className={styles.badges}>
            <StatusBadge
              label="Power Plan"
              optimized={info?.ultimate_performance ?? false}
              goodLabel="Ultimate"
              badLabel="Outro plano"
              loading={!info}
              tooltip="O plano Ultimate Performance mantém o processador em frequência máxima, eliminando latência de boost. Recomendado: Ultimate Performance."
            />
            <StatusBadge
              label="Timer Res"
              optimized={info?.timer_resolution_optimized ?? false}
              goodLabel="1 ms"
              badLabel="Padrão"
              loading={!info}
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
