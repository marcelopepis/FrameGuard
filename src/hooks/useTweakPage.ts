// Hook que encapsula toda a lógica de estado e ações de uma página de tweaks.
//
// Extraído de Optimizations.tsx e Privacy.tsx — ambas tinham código idêntico
// para carregar tweaks, gerenciar cards, aplicar/reverter e controlar seções.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';
import { useSearchHighlight } from './useSearchHighlight';
import { useHardwareFilter } from './useHardwareFilter';
import { TweakInfo, CardState, makeCardState } from '../components/TweakCard';
import { ensureRestorePoint, showRestorePointToast } from '../utils/restorePoint';
import { buildCommandMaps } from '../data/tweakRegistry';

// ── Tipos ────────────────────────────────────────────────────────────────────

/** Definição de uma seção (accordion) de tweaks na página. */
export interface Section {
  /** Identificador único da seção (ex: `"gpu_display"`) */
  id: string;
  /** Título exibido no cabeçalho da seção */
  title: string;
  /** Subtítulo descritivo abaixo do título */
  subtitle: string;
  /** IDs dos tweaks que pertencem a esta seção */
  tweakIds: string[];
}

/**
 * Configuração para o hook `useTweakPage`.
 *
 * Define quais tweaks a página exibe, como eles são agrupados em seções,
 * e comportamentos opcionais como restore-default e eventos DISM.
 */
export interface TweakPageConfig {
  /** Lista ordenada de IDs de tweaks exibidos na página */
  tweakIds: readonly string[];
  /** Seções (accordions) com agrupamento de tweaks */
  sections: Section[];
  /** Mensagem de erro customizada ao falhar o carregamento (ex: `"tweaks de privacidade"`) */
  errorLabel?: string;

  // ── Opcionais: usados apenas em Optimizations ──

  /**
   * IDs de tweaks cujo revert depende de backup — quando aplicados sem backup
   * do FrameGuard, exibem "Restaurar Padrão" em vez de "Reverter".
   */
  backupBased?: Set<string>;
  /**
   * Mapa de tweak ID → comando Tauri para restaurar o padrão Windows.
   * Necessário apenas para tweaks em `backupBased`.
   */
  restoreDefaultCommands?: Record<string, string>;
  /**
   * Mapa de tweak/comando ID → canal de evento Tauri para streaming DISM.
   * Usado para tweaks que emitem progresso via eventos (ex: reserved storage).
   */
  dismEvents?: Record<string, string>;
}

/**
 * Retorno do hook `useTweakPage`.
 *
 * Contém todos os estados e handlers necessários para renderizar uma página
 * de tweaks com seções colapsáveis, cards interativos e ações assíncronas.
 */
export interface UseTweakPageReturn {
  /** Lista de tweaks carregados do backend */
  tweaks: TweakInfo[];
  /** `true` durante o carregamento inicial dos tweaks */
  pageLoading: boolean;
  /** Mensagem de erro se o carregamento falhou, `null` caso contrário */
  pageError: string | null;
  /** Estado de cada card (loading, showDetails, dismLog) indexado por tweak ID */
  cardStates: Record<string, CardState>;
  /** Estado de expansão de cada seção indexado por section ID */
  expanded: Record<string, boolean>;
  /** `true` quando todas as seções estão expandidas */
  allExpanded: boolean;
  /** `true` quando alguma ação global está em execução (via RunningContext) */
  isRunning: boolean;
  /** Recarrega todos os tweaks do backend */
  loadTweaks: () => void;
  /** Aplica um tweak (com ponto de restauração e log de atividade) */
  handleApply: (tweak: TweakInfo) => Promise<void>;
  /** Reverte um tweak para o valor do backup */
  handleRevert: (tweak: TweakInfo) => Promise<void>;
  /** Restaura o padrão Windows sem backup (apenas para tweaks `backupBased`) */
  handleRestoreDefault: (tweak: TweakInfo) => Promise<void>;
  /** Alterna a expansão de uma seção */
  toggleSection: (id: string) => void;
  /** Expande ou colapsa todas as seções */
  toggleAll: () => void;
  /** Alterna os detalhes técnicos de um card */
  toggleDetails: (id: string) => void;
  /** Filtra IDs compatíveis com o hardware detectado */
  filterCompatible: (ids: string[]) => string[];
  /** Retorna badge de vendor para um tweak (ex: "NVIDIA"), ou `null` */
  getVendorBadge: (id: string) => string | null;
  /** Retorna `true` se o tweak é backup-based */
  isBackupBased: (id: string) => boolean;
  /** Retorna o detalhe técnico DISM log de um card (para tweaks com evento DISM) */
  technicalDetail?: Record<string, string>;
}

/**
 * Hook que encapsula toda a lógica de uma página de tweaks (Optimizations, Privacy).
 *
 * Gerencia:
 * - Carregamento de `TweakInfo` do backend via `invoke`
 * - Estados de card (loading, details, DISM log)
 * - Ações: apply, revert, restore-default (com ponto de restauração automático)
 * - Seções colapsáveis (toggle individual e toggle all)
 * - Integração com `useSearchHighlight`, `useHardwareFilter`, `RunningContext`, `ToastContext`
 *
 * @param config - Configuração da página (tweaks, seções, opções)
 * @returns Estado e handlers para renderização da página
 *
 * @example
 * ```tsx
 * const { tweaks, pageLoading, cardStates, handleApply, handleRevert } = useTweakPage({
 *   tweakIds: ['disable_vbs', 'enable_hags'],
 *   sections: [{ id: 'gaming', title: 'Gaming', subtitle: '...', tweakIds: ['disable_vbs'] }],
 * });
 * ```
 */
export function useTweakPage(config: TweakPageConfig): UseTweakPageReturn {
  const {
    tweakIds,
    sections,
    errorLabel = 'tweaks',
    backupBased = new Set<string>(),
    restoreDefaultCommands = {},
    dismEvents = {},
  } = config;

  // Deriva os mapas de comandos do tweakRegistry
  const { infoCommands, applyCommands, revertCommands } = buildCommandMaps(
    tweakIds as unknown as string[],
  );

  // ── Estado ──────────────────────────────────────────────────────────────────

  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [cardStates, setCardStates] = useState<Record<string, CardState>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();
  const { filterCompatible, getVendorBadge } = useHardwareFilter();

  // ── Search highlight ────────────────────────────────────────────────────────

  const expandSection = useCallback((id: string) => {
    setExpanded((prev) => ({ ...prev, [id]: true }));
  }, []);

  useSearchHighlight({
    dataAttribute: 'data-tweak-id',
    pageLoading,
    expandSection,
  });

  // ── Seções ──────────────────────────────────────────────────────────────────

  const allExpanded = sections.every((s) => expanded[s.id]);

  const toggleSection = useCallback((sectionId: string) => {
    setExpanded((prev) => ({ ...prev, [sectionId]: !prev[sectionId] }));
  }, []);

  const toggleAll = useCallback(() => {
    const next = !allExpanded;
    const state: Record<string, boolean> = {};
    for (const s of sections) state[s.id] = next;
    setExpanded(state);
  }, [allExpanded, sections]);

  // ── Carregamento ────────────────────────────────────────────────────────────

  const loadTweaks = useCallback(async () => {
    setPageLoading(true);
    setPageError(null);
    try {
      const results = await Promise.all(tweakIds.map((id) => invoke<TweakInfo>(infoCommands[id])));
      setTweaks(results);
      const states: Record<string, CardState> = {};
      for (const id of tweakIds) {
        states[id as string] = makeCardState();
      }
      setCardStates(states);
    } catch (e) {
      setPageError(`Erro ao carregar ${errorLabel}: ${e}`);
    } finally {
      setPageLoading(false);
    }
  }, [tweakIds, infoCommands, errorLabel]);

  useEffect(() => {
    loadTweaks();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Helpers internos ────────────────────────────────────────────────────────

  function updateCard(id: string, updates: Partial<CardState>) {
    setCardStates((prev) => ({
      ...prev,
      [id]: { ...prev[id], ...updates },
    }));
  }

  async function subscribeDism(tweakId: string, eventKey?: string): Promise<(() => void) | null> {
    const key = eventKey ?? tweakId;
    if (!(key in dismEvents)) return null;
    return listen<string>(dismEvents[key], (event) => {
      setCardStates((prev) => ({
        ...prev,
        [tweakId]: {
          ...prev[tweakId],
          dismLog: [...prev[tweakId].dismLog, event.payload],
        },
      }));
    });
  }

  // ── Toggles ─────────────────────────────────────────────────────────────────

  const toggleDetails = useCallback((id: string) => {
    setCardStates((prev) => ({
      ...prev,
      [id]: { ...prev[id], showDetails: !prev[id].showDetails },
    }));
  }, []);

  // ── Ações ───────────────────────────────────────────────────────────────────

  const handleApply = useCallback(
    async (tweak: TweakInfo) => {
      updateCard(tweak.id, { loading: true, loadingAction: 'applying', dismLog: [] });
      const unlisten = await subscribeDism(tweak.id);

      try {
        const rpResult = await ensureRestorePoint(`Antes de aplicar: ${tweak.name}`);
        showRestorePointToast(rpResult, showToast);

        await invoke(applyCommands[tweak.id]);
        const updated = await invoke<TweakInfo>(infoCommands[tweak.id]);
        setTweaks((prev) => prev.map((t) => (t.id === tweak.id ? updated : t)));
        showToast('success', 'Tweak aplicado!', tweak.name);
        invoke('log_tweak_activity', { name: tweak.name, applied: true, success: true }).catch(
          () => {},
        );
        if (tweak.requires_restart) {
          showToast(
            'warning',
            'Reinicialização necessária',
            `"${tweak.name}" só terá efeito após reiniciar o Windows.`,
            0,
          );
        }
      } catch (e) {
        showToast('error', 'Erro ao aplicar tweak', String(e));
        invoke('log_tweak_activity', { name: tweak.name, applied: true, success: false }).catch(
          () => {},
        );
      } finally {
        unlisten?.();
        updateCard(tweak.id, { loading: false, loadingAction: null });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [applyCommands, infoCommands, showToast],
  );

  const handleRevert = useCallback(
    async (tweak: TweakInfo) => {
      updateCard(tweak.id, { loading: true, loadingAction: 'reverting', dismLog: [] });
      const unlisten = await subscribeDism(tweak.id);

      try {
        await invoke(revertCommands[tweak.id]);
        const updated = await invoke<TweakInfo>(infoCommands[tweak.id]);
        setTweaks((prev) => prev.map((t) => (t.id === tweak.id ? updated : t)));
        showToast('success', 'Tweak revertido!', tweak.name);
        invoke('log_tweak_activity', { name: tweak.name, applied: false, success: true }).catch(
          () => {},
        );
      } catch (e) {
        showToast('error', 'Erro ao reverter tweak', String(e));
        invoke('log_tweak_activity', { name: tweak.name, applied: false, success: false }).catch(
          () => {},
        );
      } finally {
        unlisten?.();
        updateCard(tweak.id, { loading: false, loadingAction: null });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [revertCommands, infoCommands, showToast],
  );

  const handleRestoreDefault = useCallback(
    async (tweak: TweakInfo) => {
      const cmd = restoreDefaultCommands[tweak.id];
      if (!cmd) return;

      updateCard(tweak.id, { loading: true, loadingAction: 'restoring', dismLog: [] });
      const unlisten = await subscribeDism(tweak.id, cmd);

      try {
        await invoke(cmd);
        const updated = await invoke<TweakInfo>(infoCommands[tweak.id]);
        setTweaks((prev) => prev.map((t) => (t.id === tweak.id ? updated : t)));
        showToast('success', 'Padrão restaurado', 'Agora você pode aplicar novamente com backup.');
        if (tweak.requires_restart) {
          showToast(
            'warning',
            'Reinicialização necessária',
            `"${tweak.name}" só terá efeito após reiniciar o Windows.`,
            0,
          );
        }
      } catch (e) {
        showToast('error', 'Erro ao restaurar padrão', String(e));
      } finally {
        unlisten?.();
        updateCard(tweak.id, { loading: false, loadingAction: null });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [restoreDefaultCommands, infoCommands, showToast],
  );

  const isBackupBased = useCallback((id: string) => backupBased.has(id), [backupBased]);

  return {
    tweaks,
    pageLoading,
    pageError,
    cardStates,
    expanded,
    allExpanded,
    isRunning,
    loadTweaks,
    handleApply,
    handleRevert,
    handleRestoreDefault,
    toggleSection,
    toggleAll,
    toggleDetails,
    filterCompatible,
    getVendorBadge,
    isBackupBased,
  };
}
