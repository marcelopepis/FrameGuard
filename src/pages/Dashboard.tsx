import { useState, useEffect } from 'react';
import { Cpu, Layers, Database, Globe, ShieldCheck, ShieldOff } from 'lucide-react';
import { getSystemInfo, getSystemSummary } from '../services/systemInfo';
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
                {info.cpu_cores > 0 ? `${info.cpu_cores} núcleos` : 'Placa de vídeo'}
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
        <div className={styles.badges}>
          <StatusBadge label="Game Mode" active={info?.game_mode_enabled ?? false} loading={!info} />
          <StatusBadge label="HAGS"      active={info?.hags_enabled ?? false}       loading={!info} />
          <StatusBadge label="VBS"       active={info?.vbs_enabled ?? false}        loading={!info} />
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
  active: boolean;
  loading?: boolean;
}

function StatusBadge({ label, active, loading }: StatusBadgeProps) {
  return (
    <div className={`${styles.badge} ${loading ? '' : active ? styles.badgeOn : styles.badgeOff}`}>
      <span className={`${styles.dot} ${loading ? '' : active ? styles.dotOn : styles.dotOff}`} />
      <span className={styles.badgeLabel}>{label}</span>
      <span className={styles.badgeStatus}>
        {loading ? '…' : active ? 'Ativo' : 'Inativo'}
      </span>
    </div>
  );
}
