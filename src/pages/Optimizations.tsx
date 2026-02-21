// Página de Otimizações do FrameGuard.
//
// Exibe tweaks de performance agrupados por seção. Cada tweak tem botões
// Aplicar/Reverter/Restaurar Padrão com feedback em tempo real e disable global
// quando qualquer comando de longa duração estiver em execução em outra página.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  ChevronDown, ChevronUp, Loader2,
  XCircle, RotateCcw, Play, RefreshCw,
} from 'lucide-react';
import styles from './Optimizations.module.css';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';

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
  default_value_description: string;
}

interface CardState {
  loading: boolean;
  loadingAction: 'applying' | 'reverting' | 'restoring' | null;
  showDetails: boolean;
  dismLog: string[];
}

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  'disable_wallpaper_compression',
  'disable_reserved_storage',
  'disable_delivery_optimization',
  'enable_hags',
  'enable_game_mode',
  'disable_vbs',
] as const;

const INFO_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'get_wallpaper_compression_info',
  disable_reserved_storage:      'get_reserved_storage_info',
  disable_delivery_optimization: 'get_delivery_optimization_info',
  enable_hags:                   'get_hags_info',
  enable_game_mode:              'get_game_mode_info',
  disable_vbs:                   'get_vbs_info',
};

const APPLY_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'disable_wallpaper_compression',
  disable_reserved_storage:      'disable_reserved_storage',
  disable_delivery_optimization: 'disable_delivery_optimization',
  enable_hags:                   'enable_hags',
  enable_game_mode:              'enable_game_mode',
  disable_vbs:                   'disable_vbs',
};

const REVERT_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'revert_wallpaper_compression',
  disable_reserved_storage:      'enable_reserved_storage',
  disable_delivery_optimization: 'revert_delivery_optimization',
  enable_hags:                   'disable_hags',
  enable_game_mode:              'disable_game_mode',
  disable_vbs:                   'enable_vbs',
};

// Tweaks cujo revert usa backup — quando aplicados sem backup do FrameGuard,
// exibem "Restaurar Padrão" em vez de "Reverter".
const BACKUP_BASED = new Set([
  'disable_wallpaper_compression',
  'disable_reserved_storage',
  'disable_delivery_optimization',
]);

// Comandos para restaurar o padrão Windows sem precisar de backup.
const RESTORE_DEFAULT_COMMANDS: Record<string, string> = {
  disable_wallpaper_compression: 'restore_wallpaper_default',
  disable_reserved_storage:      'restore_reserved_storage_default',
  disable_delivery_optimization: 'restore_delivery_optimization_default',
};

// Tweaks baseados em DISM que emitem progresso via eventos Tauri
const DISM_EVENT: Record<string, string> = {
  disable_reserved_storage: 'dism-reserved-storage',
  restore_reserved_storage_default: 'dism-reserved-storage',
};

// Seções da página com IDs dos tweaks pertencentes a cada uma
const SECTIONS = [
  {
    id: 'geral',
    title: 'Geral',
    subtitle: 'Otimizações visuais e de experiência',
    tweakIds: ['disable_wallpaper_compression'],
  },
  {
    id: 'armazenamento',
    title: 'Armazenamento',
    subtitle: 'Gerenciamento de espaço em disco',
    tweakIds: ['disable_reserved_storage'],
  },
  {
    id: 'windows_update',
    title: 'Windows Update',
    subtitle: 'Configurações de atualização e distribuição',
    tweakIds: ['disable_delivery_optimization'],
  },
  {
    id: 'gamer',
    title: 'Gamer',
    subtitle: 'Otimizações de performance para jogos',
    tweakIds: ['enable_hags', 'enable_game_mode', 'disable_vbs'],
  },
];

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

  enable_hags:
`Hardware-Accelerated GPU Scheduling move o agendamento de trabalhos da GPU da CPU para a própria GPU, reduzindo latência e carga da CPU durante jogos.

Antes do HAGS, a CPU controlava quando a GPU executava cada frame, adicionando latência. Com o HAGS, a GPU agenda seu próprio trabalho internamente — mais eficiente.

Registro: HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers
Chave:    HwSchMode = 2  (ativado)   |  HwSchMode = 0  (desativado)

Requer reinicialização para ter efeito. Disponível no Windows 10 2004+ com GPU e driver compatíveis.`,

  enable_game_mode:
`O Game Mode instrui o Windows a priorizar o processo do jogo em execução para recursos de CPU, GPU e memória, reduzindo a interferência de tarefas em segundo plano.

Quando ativo, o Windows pode atrasar atualizações automáticas, reduzir prioridade de outros processos e otimizar a alocação de recursos para o jogo focado.

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\GameBar
Chave:    AutoGameModeEnabled = 1  (ativado)  |  AutoGameModeEnabled = 0  (desativado)

Não requer reinicialização. Ativa automaticamente ao detectar um jogo em tela cheia.`,

  disable_vbs:
`Virtualization Based Security usa recursos de virtualização do processador (Intel VT-x / AMD-V) para isolar processos críticos do sistema operacional em um ambiente protegido.

Embora aumente a segurança, a VBS introduz overhead de virtualização que pode reduzir o desempenho de aplicativos de alto desempenho como jogos em 5–15%.

Registro: HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\DeviceGuard
Chave:    EnableVirtualizationBasedSecurity = 0  (desativado)  |  = 1  (ativado)

Requer reinicialização. Em alguns sistemas, pode ser necessário desabilitar também na BIOS/UEFI.`,
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
    showDetails: false,
    dismLog: [],
  };
}

// ── Subcomponente TweakCard ────────────────────────────────────────────────────

interface TweakCardProps {
  tweak: TweakInfo;
  state: CardState;
  onApply: () => void;
  onRevert: () => void;
  onRestoreDefault: () => void;
  onToggleDetails: () => void;
  globalDisabled?: boolean;
}

function TweakCard({
  tweak, state,
  onApply, onRevert, onRestoreDefault,
  onToggleDetails,
  globalDisabled,
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

  // Determina se este tweak pode ficar "aplicado sem backup" (configurado externamente).
  const isExternal = BACKUP_BASED.has(tweak.id) && tweak.is_applied && !tweak.has_backup;

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

    // Estado 3a: aplicado sem backup, tweak com suporte a backup (configurado externamente)
    // → Restaurar Padrão Windows
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

    // Estado 3b: aplicado sem backup, tweak gamer (revert não precisa de backup)
    // → Reverter normalmente
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
              <pre className={styles.detailsText}>{TECHNICAL_DETAILS[tweak.id]}</pre>
            </div>
          )}

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

// ── Componente principal ───────────────────────────────────────────────────────

export default function Optimizations() {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [cardStates, setCardStates] = useState<Record<string, CardState>>({});

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();

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

  function updateCard(id: string, updates: Partial<CardState>) {
    setCardStates(prev => ({
      ...prev,
      [id]: { ...prev[id], ...updates },
    }));
  }

  // Subscreve ao canal DISM para o tweak/comando informado e retorna unlisten
  async function subscribeDism(tweakId: string, eventKey?: string): Promise<(() => void) | null> {
    const key = eventKey ?? tweakId;
    if (!(key in DISM_EVENT)) return null;
    return listen<string>(DISM_EVENT[key], event => {
      setCardStates(prev => ({
        ...prev,
        [tweakId]: {
          ...prev[tweakId],
          dismLog: [...prev[tweakId].dismLog, event.payload],
        },
      }));
    });
  }

  async function handleApply(tweak: TweakInfo) {
    updateCard(tweak.id, { loading: true, loadingAction: 'applying', dismLog: [] });
    const unlisten = await subscribeDism(tweak.id);

    try {
      await invoke(APPLY_COMMANDS[tweak.id]);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showToast('success', 'Tweak aplicado!', tweak.name);
      if (tweak.requires_restart) {
        showToast('warning', 'Reinicialização necessária',
          `"${tweak.name}" só terá efeito após reiniciar o Windows.`, 0);
      }
    } catch (e) {
      showToast('error', 'Erro ao aplicar tweak', String(e));
    } finally {
      unlisten?.();
      updateCard(tweak.id, { loading: false, loadingAction: null });
    }
  }

  async function handleRevert(tweak: TweakInfo) {
    updateCard(tweak.id, { loading: true, loadingAction: 'reverting', dismLog: [] });
    const unlisten = await subscribeDism(tweak.id);

    try {
      await invoke(REVERT_COMMANDS[tweak.id]);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showToast('success', 'Tweak revertido!', tweak.name);
    } catch (e) {
      showToast('error', 'Erro ao reverter tweak', String(e));
    } finally {
      unlisten?.();
      updateCard(tweak.id, { loading: false, loadingAction: null });
    }
  }

  async function handleRestoreDefault(tweak: TweakInfo) {
    updateCard(tweak.id, { loading: true, loadingAction: 'restoring', dismLog: [] });
    const cmd = RESTORE_DEFAULT_COMMANDS[tweak.id];
    const unlisten = await subscribeDism(tweak.id, cmd);

    try {
      await invoke(cmd);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showToast('success', 'Padrão restaurado', 'Agora você pode aplicar novamente com backup.');
      if (tweak.requires_restart) {
        showToast('warning', 'Reinicialização necessária',
          `"${tweak.name}" só terá efeito após reiniciar o Windows.`, 0);
      }
    } catch (e) {
      showToast('error', 'Erro ao restaurar padrão', String(e));
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

      {/* ── Seções de tweaks ── */}
      <div className={styles.sections}>
        {SECTIONS.map(section => {
          const sectionTweaks = section.tweakIds
            .map(id => tweaks.find(t => t.id === id))
            .filter((t): t is TweakInfo => t !== undefined);

          if (sectionTweaks.length === 0) return null;

          return (
            <div key={section.id} className={styles.section}>
              <div className={styles.sectionHeader}>
                <span className={styles.sectionTitle}>{section.title}</span>
                <span className={styles.sectionSubtitle}>{section.subtitle}</span>
              </div>

              <div className={styles.tweakList}>
                {sectionTweaks.map(tweak => {
                  const state = cardStates[tweak.id];
                  if (!state) return null;
                  return (
                    <TweakCard
                      key={tweak.id}
                      tweak={tweak}
                      state={state}
                      onApply={() => handleApply(tweak)}
                      onRevert={() => handleRevert(tweak)}
                      onRestoreDefault={() => handleRestoreDefault(tweak)}
                      onToggleDetails={() => toggleDetails(tweak.id)}
                      globalDisabled={isRunning && !state.loading}
                    />
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
