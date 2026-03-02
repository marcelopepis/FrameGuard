// Página de Privacidade e Debloat do FrameGuard.
//
// Exibe tweaks de privacidade agrupados por seção: telemetria, assistentes/bloatware
// e apps em segundo plano. Lógica idêntica à página de Otimizações — a diferença
// é apenas a lista de tweaks e as seções.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2, XCircle, RefreshCw, ChevronDown, ChevronsUpDown } from 'lucide-react';
import styles from './Optimizations.module.css';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';
import { useSearchHighlight } from '../hooks/useSearchHighlight';
import { useHardwareFilter } from '../hooks/useHardwareFilter';
import {
  TweakInfo, CardState, TweakCard, makeCardState,
} from '../components/TweakCard';
import { ensureRestorePoint, showRestorePointToast } from '../utils/restorePoint';
import BloatwareSection from '../components/BloatwareSection';

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  // Telemetria e Diagnósticos
  'disable_telemetry_registry',
  // Assistentes e Bloatware
  'disable_copilot',
  'disable_content_delivery',
  // Apps em Segundo Plano
  'disable_background_apps',
] as const;

const INFO_COMMANDS: Record<string, string> = {
  disable_telemetry_registry: 'get_telemetry_registry_info',
  disable_copilot:            'get_copilot_info',
  disable_content_delivery:   'get_content_delivery_info',
  disable_background_apps:    'get_background_apps_info',
};

const APPLY_COMMANDS: Record<string, string> = {
  disable_telemetry_registry: 'disable_telemetry_registry',
  disable_copilot:            'disable_copilot',
  disable_content_delivery:   'disable_content_delivery',
  disable_background_apps:    'disable_background_apps',
};

const REVERT_COMMANDS: Record<string, string> = {
  disable_telemetry_registry: 'revert_telemetry_registry',
  disable_copilot:            'revert_copilot',
  disable_content_delivery:   'revert_content_delivery',
  disable_background_apps:    'revert_background_apps',
};

// Seções da página com IDs dos tweaks pertencentes a cada uma
const SECTIONS = [
  {
    id: 'telemetry',
    title: 'Telemetria e Diagnósticos',
    subtitle: 'Coleta de dados e envio de informações para a Microsoft',
    tweakIds: [
      'disable_telemetry_registry',
    ],
  },
  {
    id: 'assistants',
    title: 'Assistentes e Bloatware',
    subtitle: 'Remoção de assistentes e apps pré-instalados indesejados',
    tweakIds: [
      'disable_copilot',
      'disable_content_delivery',
    ],
  },
  {
    id: 'background',
    title: 'Apps em Segundo Plano',
    subtitle: 'Controle de execução de apps UWP em background',
    tweakIds: [
      'disable_background_apps',
    ],
  },
];

const TECHNICAL_DETAILS: Record<string, string> = {
  disable_telemetry_registry:
`Este tweak configura 3 chaves de registro simultaneamente para reduzir a coleta de dados:

1. Nível de telemetria (HKLM — política de grupo):
   HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection
   AllowTelemetry = 0
   Nota: em Home/Pro o valor efetivo mínimo é 1 (Básico) mesmo com a chave em 0.

2. Experiências personalizadas (HKCU):
   HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Privacy
   TailoredExperiencesWithDiagnosticDataEnabled = 0
   Remove sugestões e anúncios baseados no histórico de uso.

3. ID de publicidade (HKCU):
   HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo
   Enabled = 0
   Impede apps UWP de acessar o GUID de rastreamento cross-app.

Complementar: desabilitar os serviços DiagTrack e dmwappushservice reduz ainda mais o envio.`,

  disable_copilot:
`Este tweak configura 3 chaves para desabilitar completamente Copilot e Cortana:

1. Política Copilot (HKCU):
   HKEY_CURRENT_USER\\Software\\Policies\\Microsoft\\Windows\\WindowsCopilot
   TurnOffWindowsCopilot = 1

2. Botão na barra de tarefas (HKCU):
   HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced
   ShowCopilotButton = 0

3. Política Cortana (HKLM):
   HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search
   AllowCortana = 0

Nota: em Windows 11 24H2+, o Copilot foi redesenhado como app independente — este tweak desabilita a integração nativa anterior. Pode necessitar reiniciar o Explorer.`,

  disable_content_delivery:
`O Content Delivery Manager instala silenciosamente apps "sugeridos" (bloatware) e exibe propagandas na interface do Windows.

Chaves desabilitadas em HKCU (14 valores = 0):
HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager
  ContentDeliveryAllowed, OemPreInstalledAppsEnabled, PreInstalledAppsEnabled,
  PreInstalledAppsEverEnabled, SilentInstalledAppsEnabled, SoftLandingEnabled,
  SubscribedContentEnabled, SubscribedContent-310093Enabled,
  SubscribedContent-338388Enabled, SubscribedContent-338389Enabled,
  SubscribedContent-338393Enabled, SubscribedContent-353694Enabled,
  SubscribedContent-353696Enabled, SystemPaneSuggestionsEnabled

Políticas adicionais em HKLM (3 valores):
  HKLM\\...\\CloudContent\\DisableWindowsConsumerFeatures = 1
  HKLM\\...\\CloudContent\\DisableConsumerAccountStateContent = 1
  HKLM\\...\\PushToInstall\\DisablePushToInstall = 1

Atenção: não desinstala apps já instalados — apenas impede novas instalações automáticas.`,

  disable_background_apps:
`Apps UWP da Microsoft Store podem executar em segundo plano mesmo quando não estão em uso, consumindo CPU, RAM e banda de rede.

Chaves configuradas (HKCU):

1. HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications
   GlobalUserDisabled = 1

2. HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Search
   BackgroundAppGlobalToggle = 0

Desabilita globalmente o background para todos os apps UWP do usuário atual.

Apps individuais podem ser reconfigurados em:
Configurações → Apps → Apps instalados → [app] → Opções avançadas

Atenção: pode afetar notificações push de apps como Mail, Calendar e Teams.`,
};

// ── Componente principal ───────────────────────────────────────────────────────

export default function Privacy() {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [cardStates, setCardStates] = useState<Record<string, CardState>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();
  const { filterCompatible, getVendorBadge } = useHardwareFilter();

  const expandSection = useCallback((id: string) => {
    setExpanded(prev => ({ ...prev, [id]: true }));
  }, []);

  useSearchHighlight({
    dataAttribute: 'data-tweak-id',
    pageLoading,
    expandSection,
  });

  function toggleSection(sectionId: string) {
    setExpanded(prev => ({ ...prev, [sectionId]: !prev[sectionId] }));
  }

  const allExpanded = SECTIONS.every(s => expanded[s.id]);

  function toggleAll() {
    const next = !allExpanded;
    const state: Record<string, boolean> = {};
    for (const s of SECTIONS) state[s.id] = next;
    setExpanded(state);
  }

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
      setPageError(`Erro ao carregar tweaks de privacidade: ${e}`);
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

  async function handleApply(tweak: TweakInfo) {
    updateCard(tweak.id, { loading: true, loadingAction: 'applying' });

    try {
      // Cria ponto de restauração antes de aplicar (se habilitado nas configurações)
      const rpResult = await ensureRestorePoint(`Antes de aplicar: ${tweak.name}`);
      showRestorePointToast(rpResult, showToast);

      await invoke(APPLY_COMMANDS[tweak.id]);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showToast('success', 'Tweak aplicado!', tweak.name);
      invoke('log_tweak_activity', { name: tweak.name, applied: true, success: true }).catch(() => {});
      if (tweak.requires_restart) {
        showToast('warning', 'Reinicialização necessária',
          `"${tweak.name}" só terá efeito após reiniciar o Windows.`, 0);
      }
    } catch (e) {
      showToast('error', 'Erro ao aplicar tweak', String(e));
      invoke('log_tweak_activity', { name: tweak.name, applied: true, success: false }).catch(() => {});
    } finally {
      updateCard(tweak.id, { loading: false, loadingAction: null });
    }
  }

  async function handleRevert(tweak: TweakInfo) {
    updateCard(tweak.id, { loading: true, loadingAction: 'reverting' });

    try {
      await invoke(REVERT_COMMANDS[tweak.id]);
      const updated = await invoke<TweakInfo>(INFO_COMMANDS[tweak.id]);
      setTweaks(prev => prev.map(t => t.id === tweak.id ? updated : t));
      showToast('success', 'Tweak revertido!', tweak.name);
      invoke('log_tweak_activity', { name: tweak.name, applied: false, success: true }).catch(() => {});
    } catch (e) {
      showToast('error', 'Erro ao reverter tweak', String(e));
      invoke('log_tweak_activity', { name: tweak.name, applied: false, success: false }).catch(() => {});
    } finally {
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
          <h1 className={styles.title}>Privacidade e Debloat</h1>
          <p className={styles.subtitle}>Controle de telemetria, rastreamento e apps desnecessários</p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button className={styles.toggleAllBtn} onClick={toggleAll}>
            <ChevronsUpDown size={13} />
            {allExpanded ? 'Colapsar tudo' : 'Expandir tudo'}
          </button>
          <button className={styles.btnRefresh} onClick={loadTweaks} title="Recarregar status">
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      {/* ── Seções de tweaks ── */}
      <div className={styles.sections}>
        {SECTIONS.map(section => {
          const compatibleIds = filterCompatible(section.tweakIds);
          const sectionTweaks = compatibleIds
            .map(id => tweaks.find(t => t.id === id))
            .filter((t): t is TweakInfo => t !== undefined);

          if (sectionTweaks.length === 0) return null;

          const activeCount = sectionTweaks.filter(t => t.is_applied).length;
          const isOpen = !!expanded[section.id];

          return (
            <div key={section.id} className={styles.section}>
              <div className={styles.sectionHeader} onClick={() => toggleSection(section.id)}>
                <div className={styles.sectionHeaderLeft}>
                  <span className={styles.sectionTitle}>{section.title}</span>
                  <span className={styles.sectionSubtitle}>{section.subtitle}</span>
                </div>
                <div className={styles.sectionStats}>
                  <span className={styles.sectionCount}>{sectionTweaks.length} ajustes</span>
                  {activeCount > 0 && (
                    <span className={styles.sectionActive}>{activeCount} ativo{activeCount !== 1 ? 's' : ''}</span>
                  )}
                </div>
                <ChevronDown
                  size={15}
                  strokeWidth={2}
                  className={`${styles.sectionChevron} ${isOpen ? styles.sectionChevronOpen : ''}`}
                />
              </div>

              <div className={`${styles.sectionContent} ${isOpen ? styles.sectionContentOpen : ''}`}>
                <div className={styles.sectionContentInner}>
                  <div className={styles.tweakList}>
                    {sectionTweaks.map(tweak => {
                      const state = cardStates[tweak.id];
                      if (!state) return null;
                      return (
                        <div key={tweak.id} data-tweak-id={tweak.id}>
                          <TweakCard
                            tweak={tweak}
                            state={state}
                            onApply={() => handleApply(tweak)}
                            onRevert={() => handleRevert(tweak)}
                            onRestoreDefault={() => {}}
                            onToggleDetails={() => toggleDetails(tweak.id)}
                            globalDisabled={isRunning && !state.loading}
                            technicalDetail={TECHNICAL_DETAILS[tweak.id]}
                            vendorBadge={getVendorBadge(tweak.id)}
                          />
                        </div>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* ── Remoção de Bloatware UWP ── */}
      <BloatwareSection />
    </div>
  );
}
