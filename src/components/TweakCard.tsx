// Componente TweakCard compartilhado entre as páginas Optimizations e Privacy.
//
// Responsável exclusivamente pela UI do card individual de tweak:
// nome, descrição, botões Aplicar/Reverter/Restaurar Padrão, badges e log DISM.
// Toda a lógica de invocação Tauri fica nas páginas pai.

import React, { useEffect, useRef } from 'react';
import {
  ChevronDown, ChevronUp, Loader2,
  RotateCcw, Play,
  ShieldCheck, ShieldAlert, ShieldQuestion,
} from 'lucide-react';
import styles from '../pages/Optimizations.module.css';

// ── Tipos exportados ──────────────────────────────────────────────────────────

export interface TweakInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  is_applied: boolean;
  requires_restart: boolean;
  last_applied: string | null;
  has_backup: boolean;
  risk_level: 'low' | 'medium' | 'high';
  evidence_level: 'proven' | 'plausible' | 'unproven';
  default_value_description: string;
}

export interface CardState {
  loading: boolean;
  loadingAction: 'applying' | 'reverting' | 'restoring' | null;
  showDetails: boolean;
  dismLog: string[];
}

export interface TweakCardProps {
  tweak: TweakInfo;
  state: CardState;
  onApply: () => void;
  onRevert: () => void;
  onRestoreDefault: () => void;
  onToggleDetails: () => void;
  globalDisabled?: boolean;
  /** Texto técnico detalhado exibido ao expandir "Saiba mais". */
  technicalDetail?: string;
  /**
   * Indica que este tweak suporta o fluxo "Restaurar Padrão Windows" quando
   * o sistema já está no estado aplicado mas sem backup do FrameGuard
   * (ex: configurado externamente via DISM ou ferramenta de terceiros).
   */
  isBackupBased?: boolean;
}

// ── Constantes ─────────────────────────────────────────────────────────────────

export const RISK_LABEL: Record<string, string> = {
  low:    'Baixo Risco',
  medium: 'Risco Médio',
  high:   'Alto Risco',
};

export const EVIDENCE_META: Record<TweakInfo['evidence_level'], {
  label: string;
  tooltip: string;
  icon: React.ElementType;
  className: string;
}> = {
  proven: {
    label:     'Comprovado',
    tooltip:   'Benchmarks documentados confirmam o benefício deste tweak',
    icon:      ShieldCheck,
    className: styles.evidenceProven,
  },
  plausible: {
    label:     'Plausível',
    tooltip:   'Raciocínio técnico sólido, mas sem benchmarks rigorosos publicados',
    icon:      ShieldQuestion,
    className: styles.evidencePlausible,
  },
  unproven: {
    label:     'Não comprovado',
    tooltip:   'Amplamente compartilhado na comunidade, sem evidência formal',
    icon:      ShieldAlert,
    className: styles.evidenceUnproven,
  },
};

// ── Utilitários exportados ────────────────────────────────────────────────────

export function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  });
}

export function makeCardState(): CardState {
  return {
    loading: false,
    loadingAction: null,
    showDetails: false,
    dismLog: [],
  };
}

// ── EvidenceBadge ─────────────────────────────────────────────────────────────

export function EvidenceBadge({ level }: { level: TweakInfo['evidence_level'] }) {
  const meta = EVIDENCE_META[level];
  const Icon = meta.icon;
  return (
    <span className={styles.evidenceBadgeWrap}>
      <span className={`${styles.evidenceBadge} ${meta.className}`}>
        <Icon size={11} />
        {meta.label}
      </span>
      <span className={styles.evidenceTip}>{meta.tooltip}</span>
    </span>
  );
}

// ── TweakCard ─────────────────────────────────────────────────────────────────

export function TweakCard({
  tweak, state,
  onApply, onRevert, onRestoreDefault,
  onToggleDetails,
  globalDisabled,
  technicalDetail,
  isBackupBased = false,
}: TweakCardProps) {
  const dismLogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (dismLogRef.current) {
      dismLogRef.current.scrollTop = dismLogRef.current.scrollHeight;
    }
  }, [state.dismLog]);

  const riskClass = {
    low:    styles.riskLow,
    medium: styles.riskMedium,
    high:   styles.riskHigh,
  }[tweak.risk_level];

  // Tweak está no estado aplicado mas sem backup do FrameGuard (configurado externamente).
  const isExternal = isBackupBased && tweak.is_applied && !tweak.has_backup;

  // ── Badge de status ──
  const statusLabel = !tweak.is_applied
    ? 'Inativo'
    : tweak.has_backup
      ? 'Aplicado pelo FrameGuard'
      : isExternal
        ? 'Aplicado (externo)'
        : 'Ativo';

  const statusMod = tweak.is_applied ? styles.statusActive : styles.statusInactive;
  const dotMod    = tweak.is_applied ? styles.dotActive    : styles.dotInactive;

  // ── Botão de ação ──
  function ActionButton() {
    if (state.loading) {
      return (
        <div className={styles.loadingState}>
          <Loader2 size={15} className={styles.spinner} />
          <span>
            {state.loadingAction === 'reverting'  ? 'Revertendo...'  :
             state.loadingAction === 'restoring'  ? 'Restaurando...' :
             'Aplicando...'}
          </span>
        </div>
      );
    }

    const disabled = !!globalDisabled;

    // Estado 1: não aplicado → Aplicar
    if (!tweak.is_applied) {
      return (
        <div className={styles.btnWrap}>
          <button
            className={styles.btnApply}
            onClick={!disabled ? onApply : undefined}
            disabled={disabled}
          >
            <Play size={13} />Aplicar
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.<br />Aguarde a conclusão.
            </div>
          )}
        </div>
      );
    }

    // Estado 2: aplicado com backup → Reverter (usando backup)
    if (tweak.has_backup) {
      return (
        <div className={styles.btnWrap}>
          <button
            className={styles.btnRevert}
            onClick={!disabled ? onRevert : undefined}
            disabled={disabled}
          >
            <RotateCcw size={13} />Reverter
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.<br />Aguarde a conclusão.
            </div>
          )}
        </div>
      );
    }

    // Estado 3a: aplicado externamente, tweak com suporte a "Restaurar Padrão"
    if (isExternal) {
      return (
        <div className={styles.btnWrap}>
          <button
            className={styles.btnRestoreDefault}
            onClick={!disabled ? onRestoreDefault : undefined}
            disabled={disabled}
          >
            <RotateCcw size={13} />Restaurar Padrão
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.<br />Aguarde a conclusão.
            </div>
          )}
        </div>
      );
    }

    // Estado 3b: aplicado sem backup, revert não precisa de backup → Reverter normalmente
    return (
      <div className={styles.btnWrap}>
        <button
          className={styles.btnRevert}
          onClick={!disabled ? onRevert : undefined}
          disabled={disabled}
        >
          <RotateCcw size={13} />Reverter
        </button>
        {disabled && (
          <div className={styles.btnTip}>
            Outro comando em execução.<br />Aguarde a conclusão.
          </div>
        )}
      </div>
    );
  }

  return (
    <div className={`${styles.tweakCard} ${state.loading ? styles.tweakCardBusy : ''}`}>

      {/* ── Layout principal: esquerda + direita ── */}
      <div className={styles.tweakBody}>

        {/* ── Lado esquerdo ── */}
        <div className={styles.tweakLeft}>
          <div className={styles.tweakName}>{tweak.name}</div>
          <p className={styles.tweakDesc}>{tweak.description}</p>

          <button className={styles.btnDetails} onClick={onToggleDetails}>
            {state.showDetails ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            {state.showDetails ? 'Menos detalhes' : 'Saiba mais'}
          </button>

          {state.showDetails && (
            <div className={styles.detailsPanel}>
              <div className={styles.evidenceDetailRow}>
                <span className={styles.evidenceDetailLabel}>Nível de evidência:</span>
                <EvidenceBadge level={tweak.evidence_level} />
                <span className={styles.evidenceDetailDesc}>
                  — {EVIDENCE_META[tweak.evidence_level].tooltip}
                </span>
              </div>
              {technicalDetail && (
                <pre className={styles.detailsText}>{technicalDetail}</pre>
              )}
            </div>
          )}

          <div className={styles.badgeRow}>
            <EvidenceBadge level={tweak.evidence_level} />
            <span className={`${styles.riskBadge} ${riskClass}`}>
              {RISK_LABEL[tweak.risk_level]}
            </span>
            {tweak.requires_restart && (
              <span className={styles.restartBadge}>
                Requer reinicialização
              </span>
            )}
          </div>

          <div className={styles.lastApplied}>
            {tweak.last_applied
              ? `Última aplicação: ${formatDate(tweak.last_applied)}`
              : 'Nunca aplicado pelo FrameGuard'}
          </div>

          {/* Dica de padrão Windows visível quando aplicado externamente */}
          {isExternal && (
            <div className={styles.defaultValueHint}>
              ↩ {tweak.default_value_description}
            </div>
          )}
        </div>

        {/* ── Lado direito ── */}
        <div className={styles.tweakRight}>
          <div className={`${styles.statusBadge} ${statusMod}`}>
            <span className={`${styles.statusDot} ${dotMod}`} />
            {statusLabel}
          </div>

          <ActionButton />
        </div>
      </div>

      {/* ── Log de progresso DISM (streaming) ── */}
      {state.dismLog.length > 0 && (
        <div className={styles.dismLog} ref={dismLogRef}>
          {state.dismLog.map((line, i) => (
            <div key={i} className={styles.dismLine}>{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
