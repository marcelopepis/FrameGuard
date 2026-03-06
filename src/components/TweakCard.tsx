/**
 * Componente TweakCard compartilhado entre as páginas Optimizations e Privacy.
 *
 * Responsável exclusivamente pela UI do card individual de tweak:
 * nome, descrição, botões Aplicar/Reverter/Restaurar Padrão, badges e log DISM.
 * Toda a lógica de invocação Tauri fica nas páginas pai.
 *
 * @module TweakCard
 */

import React, { useEffect, useRef } from 'react';
import {
  ChevronDown,
  ChevronUp,
  Loader2,
  RotateCcw,
  Play,
  ShieldCheck,
  ShieldAlert,
  ShieldQuestion,
} from 'lucide-react';
import styles from '../pages/Optimizations.module.css';

// ── Tipos exportados ──────────────────────────────────────────────────────────

/** Informações de um tweak retornadas pelo backend (`get_{tweak}_info`). */
export interface TweakInfo {
  /** Identificador único em snake_case (ex: `"disable_vbs"`) */
  id: string;
  /** Nome legível exibido na UI */
  name: string;
  /** Descrição curta do efeito do tweak */
  description: string;
  /** Categoria de agrupamento (ex: `"gamer"`, `"gpu_display"`, `"privacy"`) */
  category: string;
  /** `true` se o tweak está atualmente aplicado no sistema */
  is_applied: boolean;
  /** `true` se o tweak requer reinicialização para surtir efeito */
  requires_restart: boolean;
  /** Timestamp ISO 8601 da última aplicação, ou `null` se nunca aplicado */
  last_applied: string | null;
  /** `true` se existe backup do valor original (permite revert seguro) */
  has_backup: boolean;
  /** Nível de risco: impacto potencial em estabilidade */
  risk_level: 'low' | 'medium' | 'high';
  /** Nível de evidência: qualidade da documentação/benchmarks */
  evidence_level: 'proven' | 'plausible' | 'unproven';
  /** Descrição do valor padrão do Windows (exibida no estado "aplicado externamente") */
  default_value_description: string;
}

/** Estado de UI de um card de tweak individual. */
export interface CardState {
  /** `true` durante uma operação assíncrona (apply/revert/restore) */
  loading: boolean;
  /** Tipo da ação em andamento (determina o texto do spinner) */
  loadingAction: 'applying' | 'reverting' | 'restoring' | null;
  /** `true` quando o painel de detalhes técnicos está expandido */
  showDetails: boolean;
  /** Linhas de log DISM acumuladas (streaming em tempo real) */
  dismLog: string[];
}

/** Props do componente `TweakCard`. */
export interface TweakCardProps {
  /** Dados do tweak retornados pelo backend */
  tweak: TweakInfo;
  /** Estado de UI do card (loading, detalhes, log) */
  state: CardState;
  /** Callback ao clicar "Aplicar" */
  onApply: () => void;
  /** Callback ao clicar "Reverter" */
  onRevert: () => void;
  /** Callback ao clicar "Restaurar Padrão" (apenas tweaks backup-based) */
  onRestoreDefault: () => void;
  /** Callback ao clicar "Saiba mais" / "Menos detalhes" */
  onToggleDetails: () => void;
  /** Desabilita botões de ação quando outro comando está em execução */
  globalDisabled?: boolean;
  /** Texto técnico detalhado exibido ao expandir "Saiba mais". */
  technicalDetail?: string;
  /**
   * Indica que este tweak suporta o fluxo "Restaurar Padrão Windows" quando
   * o sistema já está no estado aplicado mas sem backup do FrameGuard
   * (ex: configurado externamente via DISM ou ferramenta de terceiros).
   */
  isBackupBased?: boolean;
  /** Label do badge de vendor (ex: "NVIDIA", "AMD") para tweaks vendor-specific. */
  vendorBadge?: string | null;
}

// ── Constantes ─────────────────────────────────────────────────────────────────

/** Mapa de `risk_level` → label em PT-BR para exibição nos badges de risco. */
export const RISK_LABEL: Record<string, string> = {
  low: 'Baixo Risco',
  medium: 'Risco Médio',
  high: 'Alto Risco',
};

/** Metadados de exibição para cada nível de evidência (label, tooltip, ícone, classe CSS). */
export const EVIDENCE_META: Record<
  TweakInfo['evidence_level'],
  {
    label: string;
    tooltip: string;
    icon: React.ElementType;
    className: string;
  }
> = {
  proven: {
    label: 'Comprovado',
    tooltip: 'Benchmarks documentados confirmam o benefício deste tweak',
    icon: ShieldCheck,
    className: styles.evidenceProven,
  },
  plausible: {
    label: 'Plausível',
    tooltip: 'Raciocínio técnico sólido, mas sem benchmarks rigorosos publicados',
    icon: ShieldQuestion,
    className: styles.evidencePlausible,
  },
  unproven: {
    label: 'Não comprovado',
    tooltip: 'Amplamente compartilhado na comunidade, sem evidência formal',
    icon: ShieldAlert,
    className: styles.evidenceUnproven,
  },
};

// ── Utilitários exportados ────────────────────────────────────────────────────

/** Formata um timestamp ISO 8601 para exibição em PT-BR (`DD/MM/YYYY HH:MM`). */
export function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** Cria o estado inicial de um card de tweak (idle, sem detalhes, sem log). */
export function makeCardState(): CardState {
  return {
    loading: false,
    loadingAction: null,
    showDetails: false,
    dismLog: [],
  };
}

// ── EvidenceBadge ─────────────────────────────────────────────────────────────

/**
 * Badge inline que exibe o nível de evidência com ícone, cor e tooltip.
 *
 * @param props.level - Nível de evidência do tweak (`"proven"`, `"plausible"` ou `"unproven"`)
 * @returns Elemento `<span>` com ícone + label + tooltip on hover
 */
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

/**
 * Card de tweak individual com status, badges, botões de ação e log DISM.
 *
 * Renderiza 4 estados de ação possíveis:
 * 1. Não aplicado → botão "Aplicar"
 * 2. Aplicado com backup → botão "Reverter"
 * 3. Aplicado externamente (backup-based) → botão "Restaurar Padrão"
 * 4. Aplicado sem backup (não backup-based) → botão "Reverter"
 *
 * @param props - Props do card (ver {@link TweakCardProps})
 * @returns Elemento do card completo com badges, botões e log DISM
 *
 * @example
 * ```tsx
 * <TweakCard
 *   tweak={tweakInfo}
 *   state={cardStates[tweakInfo.id]}
 *   onApply={() => handleApply(tweakInfo)}
 *   onRevert={() => handleRevert(tweakInfo)}
 *   onRestoreDefault={() => handleRestoreDefault(tweakInfo)}
 *   onToggleDetails={() => toggleDetails(tweakInfo.id)}
 *   globalDisabled={isRunning}
 * />
 * ```
 */
export function TweakCard({
  tweak,
  state,
  onApply,
  onRevert,
  onRestoreDefault,
  onToggleDetails,
  globalDisabled,
  technicalDetail,
  isBackupBased = false,
  vendorBadge,
}: TweakCardProps) {
  const dismLogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (dismLogRef.current) {
      dismLogRef.current.scrollTop = dismLogRef.current.scrollHeight;
    }
  }, [state.dismLog]);

  const riskClass = {
    low: styles.riskLow,
    medium: styles.riskMedium,
    high: styles.riskHigh,
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
  const dotMod = tweak.is_applied ? styles.dotActive : styles.dotInactive;

  // ── Botão de ação ──
  function ActionButton() {
    if (state.loading) {
      return (
        <div className={styles.loadingState}>
          <Loader2 size={15} className={styles.spinner} />
          <span>
            {state.loadingAction === 'reverting'
              ? 'Revertendo...'
              : state.loadingAction === 'restoring'
                ? 'Restaurando...'
                : 'Aplicando...'}
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
            <Play size={13} />
            Aplicar
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.
              <br />
              Aguarde a conclusão.
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
            <RotateCcw size={13} />
            Reverter
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.
              <br />
              Aguarde a conclusão.
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
            <RotateCcw size={13} />
            Restaurar Padrão
          </button>
          {disabled && (
            <div className={styles.btnTip}>
              Outro comando em execução.
              <br />
              Aguarde a conclusão.
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
          <RotateCcw size={13} />
          Reverter
        </button>
        {disabled && (
          <div className={styles.btnTip}>
            Outro comando em execução.
            <br />
            Aguarde a conclusão.
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
              {technicalDetail && <pre className={styles.detailsText}>{technicalDetail}</pre>}
            </div>
          )}

          <div className={styles.badgeRow}>
            <EvidenceBadge level={tweak.evidence_level} />
            <span className={`${styles.riskBadge} ${riskClass}`}>
              {RISK_LABEL[tweak.risk_level]}
            </span>
            {vendorBadge && <span className={styles.vendorBadge}>{vendorBadge}</span>}
            {tweak.requires_restart && (
              <span className={styles.restartBadge}>Requer reinicialização</span>
            )}
          </div>

          <div className={styles.lastApplied}>
            {tweak.last_applied
              ? `Última aplicação: ${formatDate(tweak.last_applied)}`
              : 'Nunca aplicado pelo FrameGuard'}
          </div>

          {/* Dica de padrão Windows visível quando aplicado externamente */}
          {isExternal && (
            <div className={styles.defaultValueHint}>↩ {tweak.default_value_description}</div>
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
            <div key={i} className={styles.dismLine}>
              {line}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
