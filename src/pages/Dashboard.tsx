import { useState, useEffect } from 'react';
import { Cpu, Layers, Database, Globe, ShieldCheck, ShieldOff } from 'lucide-react';
import { getSystemSummary } from '../services/systemInfo';
import type { SystemSummary } from '../services/systemInfo';
import styles from './Dashboard.module.css';

// Dados mockados para CPU, GPU, RAM — serão substituídos por invoke() depois
const MOCK = {
  cpu: {
    name: 'Intel Core i9-13900K',
    usage: 42,
  },
  gpu: {
    name: 'NVIDIA GeForce RTX 4080',
    tempC: 65,
  },
  ram: {
    totalGb: 32,
    usedGb: 14.2,
    usagePercent: 44,
  },
  status: {
    gameMode: true,
    hags: true,
    vbs: false,
  },
};

export default function Dashboard() {
  const { cpu, gpu, ram, status } = MOCK;

  // Dados reais do backend Rust
  const [summary, setSummary] = useState<SystemSummary | null>(null);

  useEffect(() => {
    getSystemSummary()
      .then(setSummary)
      .catch((err) => console.error('[FrameGuard] get_system_summary falhou:', err));
  }, []);

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
          <p className={styles.cardValue}>{cpu.name}</p>
          <p className={styles.cardDetail}>
            <span className={styles.highlight}>{cpu.usage}%</span>
            {' '}de uso atual
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
          <p className={styles.cardValue}>{gpu.name}</p>
          <p className={styles.cardDetail}>
            <span className={styles.highlight}>{gpu.tempC}°C</span>
            {' '}temperatura
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
          <p className={styles.cardValue}>{ram.usedGb} GB</p>
          <p className={styles.cardDetail}>de {ram.totalGb} GB total</p>
          <div className={styles.progressTrack}>
            <div
              className={styles.progressFill}
              style={{ width: `${ram.usagePercent}%` }}
            />
          </div>
          <p className={styles.progressLabel}>{ram.usagePercent}% em uso</p>
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
            {summary ? summary.os_version : <span className={styles.loading}>Carregando…</span>}
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
          <StatusBadge label="Game Mode" active={status.gameMode} />
          <StatusBadge label="HAGS"      active={status.hags} />
          <StatusBadge label="VBS"       active={status.vbs} />
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
}

function StatusBadge({ label, active }: StatusBadgeProps) {
  return (
    <div className={`${styles.badge} ${active ? styles.badgeOn : styles.badgeOff}`}>
      <span className={`${styles.dot} ${active ? styles.dotOn : styles.dotOff}`} />
      <span className={styles.badgeLabel}>{label}</span>
      <span className={styles.badgeStatus}>{active ? 'Ativo' : 'Inativo'}</span>
    </div>
  );
}
