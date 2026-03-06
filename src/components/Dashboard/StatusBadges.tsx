import { ShieldCheck, ShieldOff } from 'lucide-react';
import styles from '../../pages/Dashboard.module.css';

// ─── AdminTag ──────────────────────────────────────────────

export function AdminTag({ elevated }: { elevated: boolean }) {
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

export function StatusBadge({
  label,
  optimized,
  goodLabel,
  badLabel,
  loading,
  tooltip,
}: StatusBadgeProps) {
  return (
    <div
      className={`${styles.badge} ${loading ? '' : optimized ? styles.badgeOn : styles.badgeOff}`}
    >
      <span
        className={`${styles.dot} ${loading ? '' : optimized ? styles.dotOn : styles.dotOff}`}
      />
      <span className={styles.badgeLabel}>{label}</span>
      <span className={styles.badgeStatus}>{loading ? '…' : optimized ? goodLabel : badLabel}</span>
      {tooltip && <div className={styles.badgeTooltip}>{tooltip}</div>}
    </div>
  );
}

// ─── GameDvrBadge (3 estados) ─────────────────────────────

export function GameDvrBadge({ dvrStatus, loading }: { dvrStatus: string; loading?: boolean }) {
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

  const tooltip =
    dvrStatus === 'disabled'
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

// ─── PowerPlanBadge (3 tiers) ─────────────────────────────

export function PowerPlanBadge({
  tier,
  name,
  loading,
}: {
  tier: string;
  name: string;
  loading?: boolean;
}) {
  const tooltip =
    'O plano Ultimate Performance mantém o processador em frequência máxima, eliminando latência de boost. Recomendado: Ultimate Performance.';

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
