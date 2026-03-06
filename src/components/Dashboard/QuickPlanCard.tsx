import { Play, Zap, HeartPulse, Gamepad2, Wrench, Lock } from 'lucide-react';
import type { Plan } from '../../hooks';
import { timeAgo } from './ActivityItem';
import styles from '../../pages/Dashboard.module.css';

const PLAN_ICONS: Record<string, typeof HeartPulse> = {
  Saúde: HeartPulse,
  Gaming: Gamepad2,
  Manutenção: Wrench,
  Privacidade: Lock,
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
  isCompatible: (tweakId: string) => boolean;
}

export function QuickPlanCard({ plan, onView, onRun, disabled, isCompatible }: QuickPlanCardProps) {
  const Icon = guessPlanIcon(plan.name);
  const enabledCount = plan.items.filter((i) => i.enabled && isCompatible(i.tweak_id)).length;
  const lastExec = plan.last_executed ? timeAgo(plan.last_executed) : 'nunca';

  return (
    <div className={styles.quickPlanCard} onClick={onView} role="button" tabIndex={0}>
      <div className={styles.quickPlanIcon}>
        <Icon size={16} strokeWidth={2} />
      </div>
      <div className={styles.quickPlanInfo}>
        <span className={styles.quickPlanName}>{plan.name}</span>
        <div className={styles.quickPlanMeta}>
          <span className={styles.quickPlanLastExec}>Última: {lastExec}</span>
          {plan.recommended_frequency && (
            <span className={styles.quickPlanFreq}>{plan.recommended_frequency}</span>
          )}
        </div>
      </div>
      <button
        className={styles.quickPlanRun}
        onClick={(e) => {
          e.stopPropagation();
          onRun();
        }}
        disabled={disabled || enabledCount === 0}
        title="Executar plano"
      >
        <Play size={12} strokeWidth={2.5} fill="currentColor" />
      </button>
    </div>
  );
}
