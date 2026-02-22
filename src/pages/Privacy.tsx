// Página de Privacidade e Debloat do FrameGuard.
//
// Exibe tweaks de privacidade agrupados por seção: telemetria, assistentes/bloatware
// e apps em segundo plano. Lógica idêntica à página de Otimizações — a diferença
// é apenas a lista de tweaks e as seções.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2, XCircle, RefreshCw } from 'lucide-react';
import styles from './Optimizations.module.css';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';
import {
  TweakInfo, CardState, TweakCard, makeCardState,
} from '../components/TweakCard';

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  // Telemetria e Diagnósticos
  'disable_telemetry_registry',
  'disable_advertising_id',
  'disable_tailored_experiences',
  // Assistentes e Bloatware
  'disable_copilot',
  'disable_content_delivery',
  // Apps em Segundo Plano
  'disable_background_apps',
] as const;

const INFO_COMMANDS: Record<string, string> = {
  disable_telemetry_registry:   'get_telemetry_registry_info',
  disable_advertising_id:       'get_advertising_id_info',
  disable_tailored_experiences: 'get_tailored_experiences_info',
  disable_copilot:              'get_copilot_info',
  disable_content_delivery:     'get_content_delivery_info',
  disable_background_apps:      'get_background_apps_info',
};

const APPLY_COMMANDS: Record<string, string> = {
  disable_telemetry_registry:   'disable_telemetry_registry',
  disable_advertising_id:       'disable_advertising_id',
  disable_tailored_experiences: 'disable_tailored_experiences',
  disable_copilot:              'disable_copilot',
  disable_content_delivery:     'disable_content_delivery',
  disable_background_apps:      'disable_background_apps',
};

const REVERT_COMMANDS: Record<string, string> = {
  disable_telemetry_registry:   'revert_telemetry_registry',
  disable_advertising_id:       'revert_advertising_id',
  disable_tailored_experiences: 'revert_tailored_experiences',
  disable_copilot:              'revert_copilot',
  disable_content_delivery:     'revert_content_delivery',
  disable_background_apps:      'revert_background_apps',
};

// Seções da página com IDs dos tweaks pertencentes a cada uma
const SECTIONS = [
  {
    id: 'telemetry',
    title: 'Telemetria e Diagnósticos',
    subtitle: 'Coleta de dados e envio de informações para a Microsoft',
    tweakIds: [
      'disable_telemetry_registry',
      'disable_advertising_id',
      'disable_tailored_experiences',
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
`O Windows envia dados de diagnóstico em múltiplos níveis para a Microsoft:
  0 = Segurança (mínimo, apenas Enterprise/Education)
  1 = Básico (mínimo real em Home/Pro)
  2 = Aprimorado
  3 = Completo

A chave de política reduz o escopo de coleta, mas em Home/Pro o valor efetivo mínimo é 1 (Básico) mesmo com a chave definida em 0.

Registro: HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection
Chave:    AllowTelemetry = 0

Complementar: desabilitar os serviços DiagTrack (Connected User Experiences and Telemetry) e dmwappushservice reduz ainda mais o envio de dados.`,

  disable_advertising_id:
`O Windows atribui um ID de publicidade único por usuário (GUID gerado na instalação) que permite rastrear comportamento entre diferentes apps da Microsoft Store para personalização de anúncios.

Diferente de cookies de navegador, este ID persiste mesmo após limpar dados de apps individuais.

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo
Chave:    Enabled = 0

Desabilitar impede que apps UWP acessem o identificador via API:
Windows.System.UserProfile.AdvertisingManager.AdvertisingId

Não afeta cookies de navegador web — apenas o rastreamento nativo do Windows.`,

  disable_tailored_experiences:
`O Windows usa dados de diagnóstico e histórico de uso coletados para personalizar sugestões, dicas contextuais e anúncios exibidos na interface — incluindo Menu Iniciar, tela de bloqueio e notificações de sistema.

Exemplos do que é personalizado: apps sugeridos no Menu Iniciar, dicas de uso baseadas em hábitos, promoções de serviços Microsoft.

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Privacy
Chave:    TailoredExperiencesWithDiagnosticDataEnabled = 0

Desabilitar reduz conteúdo patrocinado e sugestões "baseadas no seu uso" na interface do Windows.`,

  disable_copilot:
`O Copilot do Windows 11 23H2+ é um assistente de IA integrado à barra de tarefas que envia consultas e contexto do sistema para servidores da Microsoft para gerar respostas.

Componentes desabilitados por este tweak:
  - Botão Copilot na barra de tarefas
  - Integração nativa do Copilot via política de grupo

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced
Chave:    ShowCopilotButton = 0

Política: HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Microsoft\\Windows\\WindowsCopilot
Chave:    TurnOffWindowsCopilot = 1

Nota: em Windows 11 24H2+, o Copilot foi redesenhado como app independente — este tweak desabilita a integração nativa anterior.`,

  disable_content_delivery:
`O Content Delivery Manager instala silenciosamente apps "sugeridos" (bloatware) da Microsoft Store e exibe dicas, sugestões e propagandas na interface do Windows sem interação do usuário.

Comportamentos desabilitados:
  - Instalação automática silenciosa de apps sugeridos
  - Sugestões de apps no Menu Iniciar
  - Dicas e truques do Windows (tela de bloqueio e notificações)
  - Sugestões de serviços Microsoft

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager
Chaves:   SilentInstalledAppsEnabled = 0
          SystemPaneSuggestionsEnabled = 0
          SubscribedContent-338389Enabled = 0
          SubscribedContent-338388Enabled = 0
          SubscribedContent-353694Enabled = 0

Atenção: não desinstala apps já instalados — apenas impede novas instalações automáticas.`,

  disable_background_apps:
`Apps UWP (Universal Windows Platform) da Microsoft Store executam processos em segundo plano mesmo quando não estão em uso, consumindo CPU, RAM e potencialmente banda de rede para verificar notificações push e atualizar tiles ao vivo.

Esta configuração desabilita globalmente a execução em background para todos os apps UWP do usuário atual.

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications
Chave:    GlobalUserDisabled = 1

Complementar:
HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Search
Chave:    BackgroundAppGlobalToggle = 0

Apps individuais podem ser reconfigurados em:
Configurações → Apps → Apps instalados → [app] → Opções avançadas → Permissões do app em segundo plano`,
};

// ── Componente principal ───────────────────────────────────────────────────────

export default function Privacy() {
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
    } catch (e) {
      showToast('error', 'Erro ao reverter tweak', String(e));
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
                      onRestoreDefault={() => {}}
                      onToggleDetails={() => toggleDetails(tweak.id)}
                      globalDisabled={isRunning && !state.loading}
                      technicalDetail={TECHNICAL_DETAILS[tweak.id]}
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
