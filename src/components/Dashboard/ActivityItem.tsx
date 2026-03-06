import { CheckCircle2, XCircle, AlertTriangle, Play, Zap, Undo2 } from 'lucide-react';
import type { ActivityEntry } from '../../hooks/useDashboardData';
import styles from '../../pages/Dashboard.module.css';

// ── Helpers de tempo relativo ────────────────────────────────────────────────

export function timeAgo(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diffSec = Math.floor((now - then) / 1000);

  if (diffSec < 60) return 'agora mesmo';
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

export { formatDuration };

// ── Componente ───────────────────────────────────────────────────────────────

export function ActivityItem({ entry }: { entry: ActivityEntry }) {
  const isPlan = entry.activity_type === 'plan_execution';
  const isRevert = entry.activity_type === 'tweak_reverted';

  const TypeIcon = isPlan ? Play : isRevert ? Undo2 : Zap;

  const resultIcon =
    entry.result === 'success' ? (
      <CheckCircle2 size={14} strokeWidth={2.5} className={styles.actResultSuccess} />
    ) : entry.result === 'partial' ? (
      <AlertTriangle size={14} strokeWidth={2.5} className={styles.actResultPartial} />
    ) : (
      <XCircle size={14} strokeWidth={2.5} className={styles.actResultFailed} />
    );

  let detail = '';
  if (isPlan && entry.completed_count !== null) {
    const parts: string[] = [];
    if (entry.completed_count > 0)
      parts.push(`${entry.completed_count} concluído${entry.completed_count !== 1 ? 's' : ''}`);
    if (entry.failed_count && entry.failed_count > 0)
      parts.push(`${entry.failed_count} ${entry.failed_count === 1 ? 'falhou' : 'falharam'}`);
    if (entry.duration_seconds > 0) parts.push(formatDuration(entry.duration_seconds));
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
