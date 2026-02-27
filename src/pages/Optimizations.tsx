// Página de Otimizações do FrameGuard.
//
// Exibe tweaks de performance agrupados por seção. Cada tweak tem botões
// Aplicar/Reverter/Restaurar Padrão com feedback em tempo real e disable global
// quando qualquer comando de longa duração estiver em execução em outra página.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Loader2, XCircle, RefreshCw, ChevronDown, ChevronsUpDown, ShieldAlert, AlertTriangle, Skull, Ban } from 'lucide-react';
import styles from './Optimizations.module.css';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';
import { useSearchHighlight } from '../hooks/useSearchHighlight';
import {
  TweakInfo, CardState, TweakCard, makeCardState,
} from '../components/TweakCard';

// ── Constantes ─────────────────────────────────────────────────────────────────

const TWEAK_IDS = [
  // GPU e Display
  'enable_hags',
  'disable_game_dvr',
  'disable_xbox_overlay',
  'enable_msi_mode_gpu',
  'disable_mpo',
  'disable_nvidia_telemetry',
  // Gaming
  'enable_game_mode',
  'disable_vbs',
  'enable_timer_resolution',
  'disable_mouse_acceleration',
  'disable_fullscreen_optimizations',
  // Energia e CPU
  'enable_ultimate_performance',
  'disable_power_throttling',
  // Armazenamento
  'disable_reserved_storage',
  'disable_hibernation',
  'disable_ntfs_last_access',
  // Rede
  'disable_delivery_optimization',
  'disable_nagle',
  // Visual e Experiência
  'disable_wallpaper_compression',
  'disable_sticky_keys',
  'disable_bing_search',
] as const;

const INFO_COMMANDS: Record<string, string> = {
  enable_hags:                      'get_hags_info',
  disable_game_dvr:                 'get_game_dvr_info',
  disable_xbox_overlay:             'get_xbox_overlay_info',
  enable_msi_mode_gpu:              'get_msi_mode_gpu_info',
  disable_mpo:                      'get_mpo_info',
  disable_nvidia_telemetry:         'get_nvidia_telemetry_info',
  enable_game_mode:                 'get_game_mode_info',
  disable_vbs:                      'get_vbs_info',
  enable_timer_resolution:          'get_timer_resolution_info',
  disable_mouse_acceleration:       'get_mouse_acceleration_info',
  disable_fullscreen_optimizations: 'get_fullscreen_optimizations_info',
  enable_ultimate_performance:      'get_ultimate_performance_info',
  disable_power_throttling:         'get_power_throttling_info',
  disable_reserved_storage:         'get_reserved_storage_info',
  disable_hibernation:              'get_hibernation_info',
  disable_ntfs_last_access:         'get_ntfs_last_access_info',
  disable_delivery_optimization:    'get_delivery_optimization_info',
  disable_nagle:                    'get_nagle_info',
  disable_wallpaper_compression:    'get_wallpaper_compression_info',
  disable_sticky_keys:              'get_sticky_keys_info',
  disable_bing_search:              'get_bing_search_info',
};

const APPLY_COMMANDS: Record<string, string> = {
  enable_hags:                      'enable_hags',
  disable_game_dvr:                 'disable_game_dvr',
  disable_xbox_overlay:             'disable_xbox_overlay',
  enable_msi_mode_gpu:              'enable_msi_mode_gpu',
  disable_mpo:                      'disable_mpo',
  disable_nvidia_telemetry:         'disable_nvidia_telemetry',
  enable_game_mode:                 'enable_game_mode',
  disable_vbs:                      'disable_vbs',
  enable_timer_resolution:          'enable_timer_resolution',
  disable_mouse_acceleration:       'disable_mouse_acceleration',
  disable_fullscreen_optimizations: 'disable_fullscreen_optimizations',
  enable_ultimate_performance:      'enable_ultimate_performance',
  disable_power_throttling:         'disable_power_throttling',
  disable_reserved_storage:         'disable_reserved_storage',
  disable_hibernation:              'disable_hibernation',
  disable_ntfs_last_access:         'disable_ntfs_last_access',
  disable_delivery_optimization:    'disable_delivery_optimization',
  disable_nagle:                    'disable_nagle',
  disable_wallpaper_compression:    'disable_wallpaper_compression',
  disable_sticky_keys:              'disable_sticky_keys',
  disable_bing_search:              'disable_bing_search',
};

const REVERT_COMMANDS: Record<string, string> = {
  enable_hags:                      'disable_hags',
  disable_game_dvr:                 'revert_game_dvr',
  disable_xbox_overlay:             'revert_xbox_overlay',
  enable_msi_mode_gpu:              'disable_msi_mode_gpu',
  disable_mpo:                      'revert_mpo',
  disable_nvidia_telemetry:         'revert_nvidia_telemetry',
  enable_game_mode:                 'disable_game_mode',
  disable_vbs:                      'enable_vbs',
  enable_timer_resolution:          'disable_timer_resolution',
  disable_mouse_acceleration:       'revert_mouse_acceleration',
  disable_fullscreen_optimizations: 'revert_fullscreen_optimizations',
  enable_ultimate_performance:      'revert_ultimate_performance',
  disable_power_throttling:         'revert_power_throttling',
  disable_reserved_storage:         'enable_reserved_storage',
  disable_hibernation:              'enable_hibernation',
  disable_ntfs_last_access:         'revert_ntfs_last_access',
  disable_delivery_optimization:    'revert_delivery_optimization',
  disable_nagle:                    'revert_nagle',
  disable_wallpaper_compression:    'revert_wallpaper_compression',
  disable_sticky_keys:              'revert_sticky_keys',
  disable_bing_search:              'revert_bing_search',
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
  disable_reserved_storage:          'dism-reserved-storage',
  restore_reserved_storage_default:  'dism-reserved-storage',
};

// Seções da página com IDs dos tweaks pertencentes a cada uma
const SECTIONS = [
  {
    id: 'gpu_display',
    title: 'GPU e Display',
    subtitle: 'Otimizações de driver gráfico e pipeline de renderização',
    tweakIds: ['enable_hags', 'disable_game_dvr', 'disable_xbox_overlay', 'enable_msi_mode_gpu', 'disable_mpo', 'disable_nvidia_telemetry'],
  },
  {
    id: 'gaming',
    title: 'Gaming',
    subtitle: 'Configurações de desempenho para jogos',
    tweakIds: ['enable_game_mode', 'disable_vbs', 'enable_timer_resolution', 'disable_mouse_acceleration', 'disable_fullscreen_optimizations'],
  },
  {
    id: 'energy_cpu',
    title: 'Energia e CPU',
    subtitle: 'Plano de energia e gerenciamento de processador',
    tweakIds: ['enable_ultimate_performance', 'disable_power_throttling'],
  },
  {
    id: 'storage',
    title: 'Armazenamento',
    subtitle: 'Gerenciamento de espaço e desempenho de disco',
    tweakIds: ['disable_reserved_storage', 'disable_hibernation', 'disable_ntfs_last_access'],
  },
  {
    id: 'network',
    title: 'Rede',
    subtitle: 'Configurações de conectividade e protocolo',
    tweakIds: ['disable_delivery_optimization', 'disable_nagle'],
  },
  {
    id: 'visual',
    title: 'Visual e Experiência',
    subtitle: 'Ajustes visuais e de usabilidade',
    tweakIds: ['disable_wallpaper_compression', 'disable_sticky_keys', 'disable_bing_search'],
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

  disable_game_dvr:
`O Xbox Game Bar mantém um buffer circular de gravação de vídeo em segundo plano (Game DVR), capturando os últimos segundos de gameplay mesmo sem o usuário solicitar.

Este processo usa GPU encode (NVENC / VCE / Quick Sync) de forma contínua e consome RAM adicional para o buffer circular.

Registro: HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\GameDVR
Chave:    AppCaptureEnabled = 0  (desabilitado)

Também pode ser desabilitado via Configurações → Xbox Game Bar → Gravação em segundo plano.`,

  disable_xbox_overlay:
`O Xbox Game Bar é um overlay de sistema que permanece em memória para fornecer funcionalidades de captura, desempenho e social durante jogos.

O processo GameBarPresenceWriter.exe consome recursos mesmo quando o overlay não está visível, verificando periodicamente se um jogo está em execução.

Registro: HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\GameBar
Chave:    UseNexusForGameBarEnabled = 0

Atenção: desabilitar impede o uso de capturas de tela e clips via Win+G.`,

  enable_msi_mode_gpu:
`Message Signaled Interrupts (MSI) é um método de interrupção mais eficiente que o tradicional Line-Based Interrupts (LBI).

Com MSI habilitado, a GPU envia interrupções diretamente pelo barramento PCIe sem precisar de linhas físicas de IRQ compartilhadas, reduzindo latência de interrupção e eliminando possíveis conflitos de IRQ com outros dispositivos.

Registro: HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Enum\\PCI\\<ID da GPU>\\Device Parameters\\Interrupt Management\\MessageSignaledInterruptProperties
Chave:    MSISupported = 1  (habilitado)  |  0  (desabilitado)

Requer reinicialização. Verifique compatibilidade com seu driver antes de aplicar.`,

  disable_mpo:
`Multiplane Overlay (MPO) permite que a GPU componha múltiplos planos de imagem de forma independente, teóricamente reduzindo carga da GPU em cenários multi-janela.

Na prática, MPO pode causar stuttering, tearing e artefatos visuais em configurações multi-monitor com determinados drivers NVIDIA e AMD — especialmente ao arrastar janelas sobrepostas.

Registro: HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\Dwm
Chave:    OverlayTestMode = 5  (desabilita MPO)

Requer reinicialização para ter efeito.`,

  disable_nvidia_telemetry:
`O driver NVIDIA instala serviços de telemetria que coletam dados de uso e enviam periodicamente para servidores da NVIDIA.

Serviços tipicamente afetados: NvTelemetryContainer, nvidia-reporter, NvContainerLocalSystem.

Esses serviços realizam operações de I/O e rede em segundo plano que são desnecessárias para uso gamer.

Nota: aplicável apenas em sistemas com GPU NVIDIA instalada. A ausência dos serviços não causa erro.`,

  enable_timer_resolution:
`O Windows usa por padrão um timer de interrupção com resolução de ~15,6 ms — o sistema "acorda" para verificar tarefas pendentes ~64 vezes por segundo.

Reduzir para 1 ms faz o sistema verificar tarefas 1000x por segundo, melhorando responsividade e reduzindo variações de frametime (jitter).

Implementação via registro:
HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\kernel
Chave: GlobalTimerResolutionRequests = 1

Nota: no Windows 11 23H2+, a resolução do timer passou a ser por processo — jogos modernos já solicitam alta resolução automaticamente.`,

  disable_mouse_acceleration:
`Enhanced Pointer Precision é a implementação Windows do algoritmo de aceleração de mouse — a velocidade do cursor aumenta de forma não-linear com a velocidade física do mouse.

Para jogos FPS, esse comportamento é prejudicial: o músculo aprende a associar distância física de movimento com distância na tela, e a aceleração quebra essa correlação.

Registro: HKEY_CURRENT_USER\\Control Panel\\Mouse
Chaves:   MouseSpeed = 0
          MouseThreshold1 = 0
          MouseThreshold2 = 0

Efeito imediato após logoff/logon ou via API SPI_SETMOUSE.`,

  disable_fullscreen_optimizations:
`"Otimizações de Tela Cheia" intercepta janelas exclusivas e as converte internamente em borderless windowed, permitindo alt-tab rápido e funcionamento de overlays.

Benefício: alt-tab mais rápido, sobreposições (Discord, Steam) funcionam melhor.
Desvantagem: adiciona camada de composição DWM que pode aumentar latência de input.

Resultado varia: jogos Vulkan/DX12 nativos têm impacto mínimo. Jogos DX11 antigos com modo exclusivo real podem se beneficiar da desativação.

Registro (por executável): HKEY_CURRENT_USER\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers
Valor: DISABLEDXMAXIMIZEDWINDOWEDMODE`,

  enable_ultimate_performance:
`O plano "Desempenho Máximo" foi introduzido no Windows 10 1803 para workstations e é oculto por padrão no Windows 11 Home/Pro.

Diferenças em relação ao plano Alto Desempenho:
  - Desabilita C-states profundos do processador (sem deep sleep)
  - Remove latência de transição entre estados de energia da CPU
  - Mantém processador em frequência máxima continuamente

Ativação via powercfg:
powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61

Atenção: aumenta consumo de energia e temperatura em repouso. Não recomendado para notebooks na bateria.`,

  disable_power_throttling:
`Power Throttling é um recurso do Windows 10+ que limita a alocação de CPU para processos classificados como "background" pelo sistema de estimativa de energia (EIGEN).

O problema para gamers: o Windows pode classificar incorretamente processos relacionados a jogos, launchers ou streaming como background e limitar sua CPU.

Registro: HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power
Chave:    PowerThrottlingOff = 1  (desabilitado)  |  ausente / 0  (habilitado)

Processos em foreground nunca são throttled — o impacto real varia por caso de uso.`,

  disable_hibernation:
`A hibernação salva o estado completo da RAM em disco (hiberfil.sys) para permitir boot rápido e retomada do estado após desligamento total.

O arquivo hiberfil.sys ocupa tipicamente 40–70% do total de RAM instalada.
Exemplo: 16 GB RAM → hiberfil.sys de ~6,4–11 GB

Desabilitar via:  powercfg /hibernate off

Também desabilita o Fast Startup do Windows (que usa hibernação do kernel para boot rápido).

Nota: SSDs NVMe modernos têm tempo de boot suficientemente rápido sem precisar de hibernação.`,

  disable_ntfs_last_access:
`Por padrão, o NTFS atualiza o timestamp "Last Access Time" de cada arquivo toda vez que ele é lido — mesmo em operações de leitura simples como listar uma pasta.

Em sistemas com muitas operações de leitura (antivírus, indexação, caches), isso gera escritas desnecessárias e contínuas no disco.

Comando: fsutil behavior set disablelastaccess 1

Valores possíveis:
  0 = atualização de timestamp habilitada (padrão Windows)
  1 = desabilitado pelo usuário
  2 = habilitado pelo sistema (Windows 10 1803+, gerenciado pelo SO)
  3 = desabilitado pelo sistema`,

  disable_nagle:
`O algoritmo de Nagle (RFC 896) agrupa múltiplos pacotes TCP pequenos em um único pacote maior antes de enviar, otimizando uso de banda às custas de latência adicional.

Para jogos com protocolo TCP (alguns MMORPGs, jogos de estratégia online), isso adiciona delay aguardando mais dados para completar o pacote — podendo chegar a 200 ms.

Registro: HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\<adaptador>
Chaves:   TcpAckFrequency = 1
          TCPNoDelay = 1

Atenção: efetivo apenas em jogos TCP. A maioria dos FPS modernos usa UDP — sem impacto nesses casos.`,

  disable_sticky_keys:
`Sticky Keys é um recurso de acessibilidade que mantém teclas modificadoras (Shift, Ctrl, Alt) pressionadas enquanto a próxima tecla é digitada.

O atalho de ativação padrão (Shift × 5 rapidamente) é facilmente acionado acidentalmente em jogos que usam Shift para sprint, esquiva ou itens, interrompendo o gameplay com uma janela de diálogo.

Registro: HKEY_CURRENT_USER\\Control Panel\\Accessibility\\StickyKeys
Chave:    Flags — remove o bit de atalho de teclado do bitmask de configuração

Este tweak desabilita apenas o atalho de ativação acidental, não o recurso Sticky Keys em si (que pode ser ativado manualmente nas configurações de acessibilidade).`,

  disable_bing_search:
`Por padrão, o Menu Iniciar do Windows 11 envia cada pesquisa ao Bing, gerando requisições de rede mesmo para buscas de aplicativos locais.

Isso adiciona latência à pesquisa local (aguarda resposta do Bing antes de exibir resultados), consome banda de rede e pode revelar hábitos de uso para a Microsoft.

Registro: HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Search
Chave:    BingSearchEnabled = 0

Complementar recomendado: DisableSearchBoxSuggestions = 1 para desabilitar sugestões de busca na web.`,
};

// ── Componente principal ───────────────────────────────────────────────────────

export default function Optimizations() {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);
  const [cardStates, setCardStates] = useState<Record<string, CardState>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();

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
      invoke('log_tweak_activity', { name: tweak.name, applied: true, success: true }).catch(() => {});
      if (tweak.requires_restart) {
        showToast('warning', 'Reinicialização necessária',
          `"${tweak.name}" só terá efeito após reiniciar o Windows.`, 0);
      }
    } catch (e) {
      showToast('error', 'Erro ao aplicar tweak', String(e));
      invoke('log_tweak_activity', { name: tweak.name, applied: true, success: false }).catch(() => {});
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
      invoke('log_tweak_activity', { name: tweak.name, applied: false, success: true }).catch(() => {});
    } catch (e) {
      showToast('error', 'Erro ao reverter tweak', String(e));
      invoke('log_tweak_activity', { name: tweak.name, applied: false, success: false }).catch(() => {});
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
          const sectionTweaks = section.tweakIds
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
                            onRestoreDefault={() => handleRestoreDefault(tweak)}
                            onToggleDetails={() => toggleDetails(tweak.id)}
                            globalDisabled={isRunning && !state.loading}
                            technicalDetail={TECHNICAL_DETAILS[tweak.id]}
                            isBackupBased={BACKUP_BASED.has(tweak.id)}
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

      {/* ── Mitos de Otimização ── */}
      <MythBuster />
    </div>
  );
}

// ── Mitos de Otimização ────────────────────────────────────────────────────────

type Verdict = 'false' | 'depends' | 'dangerous';

interface Myth {
  title: string;
  verdict: Verdict;
  verdictLabel: string;
  paragraphs: string[];
}

const VERDICT_CONFIG: Record<Verdict, { Icon: typeof Ban; className: string }> = {
  false:     { Icon: Ban,           className: 'mythVerdictFalse' },
  depends:   { Icon: AlertTriangle, className: 'mythVerdictDepends' },
  dangerous: { Icon: Skull,         className: 'mythVerdictDangerous' },
};

const MYTHS: Myth[] = [
  {
    title: '"Desabilitar efeitos visuais do Windows melhora FPS em jogos"',
    verdict: 'false',
    verdictLabel: 'Não funciona — pode piorar',
    paragraphs: [
      'Desde o Windows Vista, o Desktop Window Manager (DWM) usa aceleração de hardware (GPU) para renderizar todos os efeitos visuais. Desabilitar animações via "Ajustar para melhor desempenho" força o sistema a fazer renderização por software na CPU — o oposto do desejado.',
      'Raymond Chen, engenheiro sênior da Microsoft, documentou isso no blog "The Old New Thing": desabilitar composição de desktop pode aumentar carga da CPU. Testes independentes mostram diferença de <1 FPS entre configurações.',
      'Em PCs com GPU dedicada (que é o público-alvo de otimização gaming), o impacto é literalmente zero. A exceção teórica seria PCs com GPU integrada muito fraca e <4 GB RAM — que não servem para gaming de qualquer forma.',
    ],
  },
  {
    title: '"Desabilitar Windows Update melhora performance"',
    verdict: 'dangerous',
    verdictLabel: 'Perigoso',
    paragraphs: [
      'Desabilitar o Windows Update deixa o PC vulnerável a exploits conhecidos, remove patches de segurança do driver de GPU e pode impedir o funcionamento de anti-cheats (que exigem patches recentes do Windows).',
      'Componentes como .NET e Visual C++ Redistributables não são atualizados, quebrando jogos que dependem de versões recentes. Drivers de GPU sem patches podem conter bugs de estabilidade e vulnerabilidades.',
      'O que o FrameGuard faz de diferente: permite pausar updates temporariamente quando necessário. Nunca desabilita permanentemente — a atualização contínua é essencial para segurança e compatibilidade com jogos modernos.',
    ],
  },
  {
    title: '"O Windows reserva 20% da sua banda — altere QoS"',
    verdict: 'false',
    verdictLabel: 'Mito urbano',
    paragraphs: [
      'A chave NonBestEffortLimit do QoS Packet Scheduler controla priorização de pacotes, não reserva fixa de banda. O Windows NÃO reserva 20% da sua conexão — isso é um mito que circula desde o Windows XP.',
      'A largura de banda só é "reservada" quando um aplicativo solicita explicitamente via API QoS, e mesmo assim a banda ociosa fica 100% disponível para outros processos. Nenhum jogo ou aplicativo comum faz essa solicitação.',
      'Alterar a chave no registro não produz nenhuma diferença mensurável em velocidade de download, upload ou latência. Todos os testes de bandwidth mostram resultados idênticos antes e depois da alteração.',
    ],
  },
  {
    title: '"Desabilitar Prefetch/Superfetch (SysMain) em SSD"',
    verdict: 'depends',
    verdictLabel: 'Raramente útil',
    paragraphs: [
      'O SysMain (antigo Superfetch) pré-carrega aplicativos frequentes na RAM ociosa. Em PCs com 16 GB+ de RAM, o serviço é inteligente e utiliza apenas memória que não está sendo usada — sem impacto negativo.',
      'Desabilitar o serviço pode aumentar o tempo de carga de aplicativos frequentes, já que eles não estarão pré-carregados na memória. O impacto no desgaste do SSD é insignificante considerando a vida útil de SSDs modernos (centenas de TBW).',
      'Só faz sentido desabilitar em PCs com menos de 8 GB de RAM e problemas específicos de uso excessivo de memória. Para a maioria dos gamers com 16-32 GB, manter ativo é a melhor escolha.',
    ],
  },
  {
    title: '"Desabilitar transparência e sombras do cursor melhora FPS"',
    verdict: 'false',
    verdictLabel: 'Impacto zero',
    paragraphs: [
      'Efeitos de transparência e sombras de cursor são renderizados pelo DWM com aceleração de hardware, consumindo frações de milissegundo de tempo de GPU — custo completamente imperceptível.',
      'Em jogos rodando em fullscreen exclusivo (a maioria dos títulos competitivos), o DWM nem está compondo a interface do Windows. Os efeitos simplesmente não existem durante o jogo.',
      'Este é o mesmo princípio do Mito 1: o DWM usa GPU para composição visual. Desabilitar sombras e transparências não libera recursos significativos — a GPU já dedicaria poder de processamento muito maior ao jogo em si.',
    ],
  },
];

function MythBuster() {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  function toggle(idx: number) {
    setExpanded(prev => {
      const s = new Set(prev);
      if (s.has(idx)) s.delete(idx); else s.add(idx);
      return s;
    });
  }

  return (
    <div className={styles.mythSection}>
      <div className={styles.mythHeader}>
        <ShieldAlert size={16} strokeWidth={2} className={styles.mythHeaderIcon} />
        <div>
          <h2 className={styles.mythTitle}>Mitos de Otimização</h2>
          <p className={styles.mythSubtitle}>
            Tweaks populares que NÃO incluímos — e por que o FrameGuard é diferente
          </p>
        </div>
      </div>

      <div className={styles.mythList}>
        {MYTHS.map((myth, idx) => {
          const isOpen = expanded.has(idx);
          const cfg = VERDICT_CONFIG[myth.verdict];
          const VIcon = cfg.Icon;

          return (
            <div key={idx} className={`${styles.mythCard} ${isOpen ? styles.mythCardOpen : ''}`}>
              <button className={styles.mythToggle} onClick={() => toggle(idx)}>
                <div className={styles.mythToggleLeft}>
                  <span className={`${styles.mythVerdict} ${styles[cfg.className]}`}>
                    <VIcon size={11} strokeWidth={2.5} />
                    {myth.verdictLabel}
                  </span>
                  <span className={styles.mythName}>{myth.title}</span>
                </div>
                <ChevronDown
                  size={14}
                  strokeWidth={2}
                  className={`${styles.mythChevron} ${isOpen ? styles.mythChevronOpen : ''}`}
                />
              </button>

              {isOpen && (
                <div className={styles.mythContent}>
                  {myth.paragraphs.map((p, i) => (
                    <p key={i} className={styles.mythParagraph}>{p}</p>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
