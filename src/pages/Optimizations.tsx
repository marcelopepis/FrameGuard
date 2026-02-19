// Página de Otimizações do FrameGuard.
//
// Exibe os tweaks de performance disponíveis com controles para aplicar
// e reverter cada otimização individualmente, com feedback em tempo real.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  ChevronDown, ChevronUp, Loader2, CheckCircle2,
  XCircle, AlertTriangle, RotateCcw, Play, RefreshCw, X,
} from 'lucide-react';
import styles from './Optimizations.module.css';

// ── Tipos ──────────────────────────────────────────────────────────────────────

interface TweakInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  is_applied: boolean;
  requires_restart: boolean;
  last_applied: string | null;
  has_backup: boolean;
  risk_level: 'low' | 'medium' | 'high';
}

interface CardState {
  loading: boolean;
  loadingAction: 'applying' | 'reverting' | null;
  feedback: { type: 'success' | 'error'; message: string } | null;
  showDetails: boolean;
  showRestartWarning: boolean;
  dismLog: string[];
}

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  'disable_wallpaper_compression',
  'disable_reserved_storage',
  'disable_delivery_optimization',
] as const;

const INFO_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'get_wallpaper_compression_info',
  disable_reserved_storage:      'get_reserved_storage_info',
  disable_delivery_optimization: 'get_delivery_optimization_info',
};

const APPLY_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'disable_wallpaper_compression',
  disable_reserved_storage:      'disable_reserved_storage',
  disable_delivery_optimization: 'disable_delivery_optimization',
};

const REVERT_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'revert_wallpaper_compression',
  disable_reserved_storage:      'enable_reserved_storage',
  disable_delivery_optimization: 'revert_delivery_optimization',
};

// Tweaks baseados em DISM que emitem progresso via eventos Tauri
const DISM_EVENT: Record<string, string> = {
  disable_reserved_storage: 'dism-reserved-storage',
};

const TECHNICAL_DETAILS: Record<string, string> = {
  disable_wallpaper_compression:
`O Windows comprime automaticamente imagens JPEG usadas como wallpaper para 85% de qualidade ao importá-las para o perfil do usuário. A chave JPEGImportQuality controla essa qualidade (0–100).

Definir para 100 instrui o Windows a manter a imagem original sem perda de qualidade.

Registro: HKEY_CURRENT_USER\\Control Panel\\Desktop
Chave:    JPEGImportQuality = 100  (padrão Windows: ausente = 85%)

Reversão: remove a chave (restaura 85%) ou restaura o valor original.`,

  disable_reserved_storage:
`O Windows reserva ~7 GB do disco para garantir espaço durante instalação de atualizações, recursos opcionais e arquivos temporários. Esse espaço fica inacessível ao usuário normal.

Desabilitar via DISM libera o espaço imediatamente, mas você passa a ser responsável por manter espaço livre suficiente ao instalar updates do Windows.

Atenção: pode impedir a instalação de atualizações em discos muito cheios.

Comando: DISM /Online /Set-ReservedStorageState /State:Disabled
Reversão: DISM /Online /Set-ReservedStorageState /State:Enabled`,

  disable_delivery_optimization:
`O Windows Update usa P2P por padrão (DODownloadMode = 1) para distribuir partes de atualizações entre computadores da rede local e da internet. Esse processo consome upload de forma silenciosa e pode aumentar a latência durante sessões de jogo online.

DODownloadMode = 0 (HTTP only) força o Windows a baixar atualizações exclusivamente dos servidores da Microsoft.

Registro: HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\DeliveryOptimization\\Config
Chave:    DODownloadMode = 0  (padrão: 1)

Reversão: remove a chave ou restaura o valor original.`,
};

const RISK_LABEL: Record<string, string> = {
  low:    'Baixo Risco',
  medium: 'Risco Médio',
  high:   'Alto Risco',
};

// ── Utilitários ────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  });
}

function makeCardState(): CardState {
  return {
    loading: false,
    loadingAction: null,
    feedback: null,
    showDetails: false,
    showRestartWarning: false,
    dismLog: [],
  };
}

// ── Subcomponente TweakCard ────────────────────────────────────────────────────

interface TweakCardProps {
  tweak: TweakInfo;
  state: CardState;
  onApply: () => void;
  onRevert: () => void;
  onToggleDetails: () => void;
  onDismissRestart: () => void;
}

function TweakCard({ tweak, state, onApply, onRevert, onToggleDetails, onDismissRestart }: TweakCardProps) {
  const dismLogRef = useRef<HTMLDivElement>(null);

  // Auto-scroll do log DISM à medida que linhas chegam
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

  return (
    <div className={`${styles.tweakCard} ${state.loading ? styles.tweakCardBusy : ''}`}>

      {/* ── Layout principal: esquerda + direita ── */}
      <div className={styles.tweakBody}>

        {/* ── Lado esquerdo ── */}
        <div className={styles.tweakLeft}>
          <div className={styles.tweakName}>{tweak.name}</div>
          <p className={styles.tweakDesc}>{tweak.description}</p>

          {/* Botão "Saiba mais" */}
          <button className={styles.btnDetails} onClick={onToggleDetails}>
            {state.showDetails ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            {state.showDetails ? 'Menos detalhes' : 'Saiba mais'}
          </button>

          {/* Painel de detalhes técnicos expandido */}
          {state.showDetails && (
            <div className={styles.detailsPanel}>
              <pre className={styles.detailsText}>{TECHNICAL_DETAILS[tweak.id]}</pre>
            </div>
          )}

          {/* Badges de risco e reinicialização */}
          <div className={styles.badgeRow}>
            <span className={`${styles.riskBadge} ${riskClass}`}>
              {RISK_LABEL[tweak.risk_level]}
            </span>
            {tweak.requires_restart && (
              <span className={styles.restartBadge}>
                Requer reinicialização
              </span>
            )}
          </div>

          {/* Data da última aplicação */}
          <div className={styles.lastApplied}>
            {tweak.last_applied
              ? `Última aplicação: ${formatDate(tweak.last_applied)}`
              : 'Nunca aplicado'}
          </div>
        </div>

        {/* ── Lado direito ── */}
        <div className={styles.tweakRight}>

          {/* Badge de status atual */}
          <div className={`${styles.statusBadge} ${tweak.is_applied ? styles.statusActive : styles.statusInactive}`}>
            <span className={`${styles.statusDot} ${tweak.is_applied ? styles.dotActive : styles.dotInactive}`} />
            {tweak.is_applied ? 'Ativo' : 'Inativo'}
          </div>

          {/* Botão de ação ou estado de carregamento */}
          {state.loading ? (
            <div className={styles.loadingState}>
              <Loader2 size={15} className={styles.spinner} />
              <span>{state.loadingAction === 'reverting' ? 'Revertendo...' : 'Aplicando...'}</span>
            </div>
          ) : tweak.is_applied ? (
            tweak.has_backup ? (
              <button className={styles.btnRevert} onClick={onRevert}>
                <RotateCcw size={13} />
                Reverter
              </button>
            ) : (
              <span className={styles.noBackup}>Sem backup para reverter</span>
            )
          ) : (
            <button className={styles.btnApply} onClick={onApply}>
              <Play size={13} />
              Aplicar
            </button>
          )}

          {/* Feedback de sucesso ou erro (desaparece em 3s) */}
          {state.feedback && (
            <div className={`${styles.feedback} ${state.feedback.type === 'success' ? styles.feedbackSuccess : styles.feedbackError}`}>
              {state.feedback.type === 'success'
                ? <CheckCircle2 size={13} />
                : <XCircle size={13} />}
              <span>{state.feedback.message}</span>
            </div>
          )}
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

      {/* ── Aviso de reinicialização necessária ── */}
      {state.showRestartWarning && (
        <div className={styles.restartWarning}>
          <AlertTriangle size={14} />
          <span>Este tweak só terá efeito após reinicialização do Windows.</span>
          <button className={styles.btnDismissWarning} onClick={onDismissRestart} title="Fechar aviso">
            <X size={12} />
          </button>
        </div>
      )}
    </div>
  );
}

// ── Componente principal ───────────────────────────────────────────────────────

export default function Optimizations() {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [cardStates, setCardStates] = useState<Record<string, CardState>>({});

  // Carrega o estado atual de todos os tweaks via Tauri
  async function loadTweaks() {
    setPageLoading(true);
    setPageError(null);
    try {
      const results = await Promise.all(
        TWEAK_IDS.map(id => invoke<TweakInfo>(INFO_COMMANDS[id]))
      );
      setTweaks(results);
      const states: Record<string, CardState> = {};
      for (const id of TWEAK_IDS) {
        states[id] = makeCardState();
      }
      setCardStates(states);
    } catch (e) {
      setPageError(`Erro ao carregar tweaks: ${e}`);
    } finally {
      setPageLoading(false);
    }
  }

  useEffect(() => { loadTweaks(); }, []);

  // Atualiza campos parciais de um card sem afetar os demais
  function updateCard(id: string, updates: Partial<CardState>) {
    setCardStates(prev => ({
      ...prev,
      [id]: { ...prev[id], ...updates },
    }));
  }

  // Exibe feedback temporário por 3 segundos e depois limpa
  function showFeedback(id: string, type: 'success' | 'error', message: string) {
    updateCard(id, { feedback: { type, message } });
    setTimeout(() => updateCard(id, { feedback: null }), 3000);
  }

  async function handleApply(tweak: TweakInfo) {
    updateCard(tweak.id, {
      loading: true,
      loadingAction: 'applying',
      dismLog: [],
      showRestartWarning: false,
    });

    // Tweaks DISM: registra listener antes do invoke para capturar o streaming
    let unlisten: (() => void) | null = null;
    if (tweak.id in DISM_EVENT) {
      unlisten = await listen<string>(DISM_EVENT[tweak.id], event => {
        setCardStates(prev => ({
          ...prev,
          [tweak.id]: {
            ...prev[tweak.id],
            dismLog: [...prev[tweak.id].dismLog, event.payload],
          },
        }));
      });
    }

    try {
      await invoke(APPLY_COMMANDS[tweak.id]);
      // Atualiza o estado do tweak após aplicação bem-sucedida
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showFeedback(tweak.id, 'success', 'Tweak aplicado com sucesso!');
      if (tweak.requires_restart) {
        updateCard(tweak.id, { showRestartWarning: true });
      }
    } catch (e) {
      showFeedback(tweak.id, 'error', String(e));
    } finally {
      unlisten?.();
      updateCard(tweak.id, { loading: false, loadingAction: null });
    }
  }

  async function handleRevert(tweak: TweakInfo) {
    updateCard(tweak.id, {
      loading: true,
      loadingAction: 'reverting',
      dismLog: [],
      showRestartWarning: false,
    });

    let unlisten: (() => void) | null = null;
    if (tweak.id in DISM_EVENT) {
      unlisten = await listen<string>(DISM_EVENT[tweak.id], event => {
        setCardStates(prev => ({
          ...prev,
          [tweak.id]: {
            ...prev[tweak.id],
            dismLog: [...prev[tweak.id].dismLog, event.payload],
          },
        }));
      });
    }

    try {
      await invoke(REVERT_COMMANDS[tweak.id]);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showFeedback(tweak.id, 'success', 'Tweak revertido com sucesso!');
    } catch (e) {
      showFeedback(tweak.id, 'error', String(e));
    } finally {
      unlisten?.();
      updateCard(tweak.id, { loading: false, loadingAction: null });
    }
  }

  function toggleDetails(id: string) {
    setCardStates(prev => ({
      ...prev,
      [id]: { ...prev[id], showDetails: !prev[id].showDetails },
    }));
  }

  // ── Render ──

  if (pageLoading) {
    return (
      <div className={styles.page}>
        <div className={styles.pageLoading}>
          <Loader2 size={20} className={styles.spinner} />
          <span>Verificando estado dos tweaks...</span>
        </div>
      </div>
    );
  }

  if (pageError) {
    return (
      <div className={styles.page}>
        <div className={styles.pageError}>
          <XCircle size={18} />
          <span>{pageError}</span>
          <button className={styles.btnRetry} onClick={loadTweaks}>
            Tentar novamente
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.page}>

      {/* ── Header ── */}
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Otimizações</h1>
          <p className={styles.subtitle}>Configurações de performance do Windows</p>
        </div>
        <button className={styles.btnRefresh} onClick={loadTweaks} title="Recarregar status">
          <RefreshCw size={14} />
        </button>
      </div>

      {/* ── Lista de tweaks ── */}
      <div className={styles.tweakList}>
        {tweaks.map(tweak => {
          const state = cardStates[tweak.id];
          if (!state) return null;
          return (
            <TweakCard
              key={tweak.id}
              tweak={tweak}
              state={state}
              onApply={() => handleApply(tweak)}
              onRevert={() => handleRevert(tweak)}
              onToggleDetails={() => toggleDetails(tweak.id)}
              onDismissRestart={() => updateCard(tweak.id, { showRestartWarning: false })}
            />
          );
        })}
      </div>
    </div>
  );
}
