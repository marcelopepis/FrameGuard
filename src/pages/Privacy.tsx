// Página de Privacidade e Debloat do FrameGuard.
//
// Exibe tweaks de privacidade agrupados por seção: telemetria, assistentes/bloatware
// e apps em segundo plano. Usa o hook useTweakPage para toda a lógica de estado.

import { Loader2, XCircle, RefreshCw, ChevronDown, ChevronsUpDown } from 'lucide-react';
import styles from './Privacy.module.css';
import { TweakInfo, TweakCard } from '../components/TweakCard';
import { useTweakPage } from '../hooks/useTweakPage';
import BloatwareSection from '../components/BloatwareSection';

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  // Telemetria e Diagnósticos
  'disable_telemetry_registry',
  'disable_wer',
  'disable_feedback_requests',
  // Assistentes e Bloatware
  'disable_copilot',
  'disable_content_delivery',
  'edge_debloat',
  // Rastreamento e Histórico
  'disable_activity_history',
  'disable_location_tracking',
  'disable_windows_recall',
  // Apps em Segundo Plano
  'disable_background_apps',
] as const;

const SECTIONS = [
  {
    id: 'telemetry',
    title: 'Telemetria e Diagnósticos',
    subtitle: 'Coleta de dados e envio de informações para a Microsoft',
    tweakIds: ['disable_telemetry_registry', 'disable_wer', 'disable_feedback_requests'],
  },
  {
    id: 'assistants',
    title: 'Assistentes e Bloatware',
    subtitle: 'Remoção de assistentes e apps pré-instalados indesejados',
    tweakIds: ['disable_copilot', 'disable_content_delivery', 'edge_debloat'],
  },
  {
    id: 'tracking',
    title: 'Rastreamento e Histórico',
    subtitle: 'Controle de rastreamento de atividades, localização e IA',
    tweakIds: ['disable_activity_history', 'disable_location_tracking', 'disable_windows_recall'],
  },
  {
    id: 'background',
    title: 'Apps em Segundo Plano',
    subtitle: 'Controle de execução de apps UWP em background',
    tweakIds: ['disable_background_apps'],
  },
];

const TECHNICAL_DETAILS: Record<string, string> = {
  disable_telemetry_registry: `Este tweak configura 3 chaves de registro simultaneamente para reduzir a coleta de dados:

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

  disable_copilot: `Este tweak configura 3 chaves para desabilitar completamente Copilot e Cortana:

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

  disable_content_delivery: `O Content Delivery Manager instala silenciosamente apps "sugeridos" (bloatware) e exibe propagandas na interface do Windows.

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

  disable_background_apps: `Apps UWP da Microsoft Store podem executar em segundo plano mesmo quando não estão em uso, consumindo CPU, RAM e banda de rede.

Chaves configuradas (HKCU):

1. HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications
   GlobalUserDisabled = 1

2. HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Search
   BackgroundAppGlobalToggle = 0

Desabilita globalmente o background para todos os apps UWP do usuário atual.

Apps individuais podem ser reconfigurados em:
Configurações → Apps → Apps instalados → [app] → Opções avançadas

Atenção: pode afetar notificações push de apps como Mail, Calendar e Teams.`,

  disable_windows_recall: `O Windows Recall é um recurso de IA que tira screenshots periódicos da tela para criar uma "memória" pesquisável. Mesmo em PCs sem hardware Copilot+, desabilitar preventivamente evita ativação futura.

Chaves configuradas (HKLM — política de grupo):
HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsAI
  DisableAIDataAnalysis = 1
  TurnOffSavingSnapshots = 1

Desabilita tanto a análise de dados por IA quanto a captura de snapshots.`,

  disable_wer: `O Windows Error Reporting (WER) envia relatórios de crash e erros para a Microsoft. Desabilitar reduz tráfego de rede e evita envio de dados potencialmente sensíveis.

Chave configurada (HKLM):
HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting
  Disabled = 1

A maioria dos usuários não se beneficia dos relatórios — desenvolvedores podem querer manter habilitado.`,

  disable_activity_history: `O Histórico de Atividades rastreia apps usados, arquivos abertos e sites visitados para alimentar a Timeline e funcionalidades cross-device.

Chaves configuradas (HKLM — política de grupo):
HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\System
  EnableActivityFeed = 0
  PublishUserActivities = 0
  UploadUserActivities = 0

Desabilita coleta, publicação e upload de atividades. A Timeline deixa de ser alimentada.`,

  disable_location_tracking: `O rastreamento de localização permite que Windows e apps acessem sua posição geográfica via GPS, Wi-Fi ou IP.

Chave configurada (HKLM):
HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\location
  Value = "Deny"

Bloqueia o acesso à localização para todos os apps e serviços do sistema.`,

  disable_feedback_requests: `O Windows exibe periodicamente pop-ups solicitando feedback sobre o sistema operacional.

Chave configurada (HKCU):
HKEY_CURRENT_USER\\Software\\Microsoft\\Siuf\\Rules
  NumberOfSIUFInPeriod = 0

Definir como 0 impede que o Windows solicite feedback. A reversão remove a chave, restaurando o comportamento padrão.`,

  edge_debloat: `Aplica 12 políticas de grupo para reduzir recursos de background e telemetria do Microsoft Edge.

Chaves configuradas em HKLM\\SOFTWARE\\Policies\\Microsoft\\Edge:
  StartupBoostEnabled = 0 (pré-carga na inicialização)
  BackgroundModeEnabled = 0 (processos em background)
  NewTabPagePrerenderEnabled = 0 (pré-renderização de nova aba)
  HubsSidebarEnabled = 0 (sidebar com Copilot/Discover)
  EdgeShoppingAssistantEnabled = 0 (assistente de compras)
  EdgeCollectionsEnabled = 0 (coleções)
  ShowRecommendationsEnabled = 0 (recomendações)
  DefaultBrowserSettingsCampaignEnabled = 0 (nags de browser padrão)
  NewTabPageBingChatEnabled = 0 (Copilot na nova aba)
  DiagnosticData = 0 (telemetria do Edge)
  PersonalizationReportingEnabled = 0 (relatórios de personalização)
  UserFeedbackAllowed = 0 (feedback do usuário)

A reversão remove cada política individualmente, restaurando o comportamento padrão do Edge.`,
};

// ── Componente principal ───────────────────────────────────────────────────────

export default function Privacy() {
  const {
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
  } = useTweakPage({
    tweakIds: TWEAK_IDS,
    sections: SECTIONS,
    errorLabel: 'tweaks de privacidade',
  });

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
          <p className={styles.subtitle}>
            Controle de telemetria, rastreamento e apps desnecessários
          </p>
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
        {SECTIONS.map((section) => {
          const compatibleIds = filterCompatible(section.tweakIds);
          const sectionTweaks = compatibleIds
            .map((id) => tweaks.find((t) => t.id === id))
            .filter((t): t is TweakInfo => t !== undefined);

          if (sectionTweaks.length === 0) return null;

          const activeCount = sectionTweaks.filter((t) => t.is_applied).length;
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
                    <span className={styles.sectionActive}>
                      {activeCount} ativo{activeCount !== 1 ? 's' : ''}
                    </span>
                  )}
                </div>
                <ChevronDown
                  size={15}
                  strokeWidth={2}
                  className={`${styles.sectionChevron} ${isOpen ? styles.sectionChevronOpen : ''}`}
                />
              </div>

              <div
                className={`${styles.sectionContent} ${isOpen ? styles.sectionContentOpen : ''}`}
              >
                <div className={styles.sectionContentInner}>
                  <div className={styles.tweakList}>
                    {sectionTweaks.map((tweak) => {
                      const state = cardStates[tweak.id];
                      if (!state) return null;
                      return (
                        <div key={tweak.id} data-tweak-id={tweak.id}>
                          <TweakCard
                            tweak={tweak}
                            state={state}
                            onApply={() => handleApply(tweak)}
                            onRevert={() => handleRevert(tweak)}
                            onRestoreDefault={() => handleRestoreDefault(tweak)}
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
