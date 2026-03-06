/**
 * Registro centralizado de todos os tweaks do FrameGuard.
 *
 * Fonte única de verdade para IDs, nomes, descrições e comandos Tauri.
 * Consumido por Optimizations.tsx, Privacy.tsx e Plans.tsx para evitar
 * duplicação manual de catálogo.
 *
 * @module tweakRegistry
 */

/** Entrada do registro de tweaks com metadados e comandos Tauri associados. */
export interface TweakRegistryEntry {
  /** Identificador único em snake_case (ex: `"disable_wallpaper_compression"`) */
  id: string;
  /** Nome legível exibido na UI (ex: `"Desabilitar Compressão de Wallpaper"`) */
  name: string;
  /** Descrição curta do efeito — usada no catálogo de Plans */
  description: string;
  /** Chave de categoria para agrupamento (ex: `"optimization"`, `"privacy"`, `"maintenance-dism"`) */
  categoryKey: string;
  /** Nome do comando Tauri que retorna `TweakInfo` (ex: `"get_wallpaper_compression_info"`) */
  infoCommand: string;
  /** Nome do comando Tauri que aplica o tweak (ex: `"disable_wallpaper_compression"`) */
  applyCommand: string;
  /** Nome do comando Tauri que reverte o tweak (ex: `"revert_wallpaper_compression"`) */
  revertCommand: string;
}

/**
 * Catálogo completo de todos os tweaks disponíveis no FrameGuard.
 *
 * Inclui tweaks de otimização (21), privacidade (4) e ações de manutenção (9).
 * A ordem segue o agrupamento lógico por domínio.
 */
export const TWEAK_REGISTRY: readonly TweakRegistryEntry[] = [
  // ── GPU e Display ───────────────────────────────────────────────────────────
  {
    id: 'enable_hags',
    name: 'Hardware-Accelerated GPU Scheduling (HAGS)',
    description: 'Permite que a GPU gerencie sua própria memória, reduzindo latência de frames',
    categoryKey: 'gamer',
    infoCommand: 'get_hags_info',
    applyCommand: 'enable_hags',
    revertCommand: 'disable_hags',
  },
  {
    id: 'disable_game_dvr',
    name: 'Desabilitar Game DVR',
    description: 'Desabilita gravação em segundo plano, liberando GPU (encoder) e CPU',
    categoryKey: 'gpu_display',
    infoCommand: 'get_game_dvr_info',
    applyCommand: 'disable_game_dvr',
    revertCommand: 'revert_game_dvr',
  },
  {
    id: 'disable_xbox_overlay',
    name: 'Desabilitar Xbox Game Bar Overlay',
    description: 'Remove overlay da Xbox Game Bar (Win+G) que pode interferir em jogos',
    categoryKey: 'gpu_display',
    infoCommand: 'get_xbox_overlay_info',
    applyCommand: 'disable_xbox_overlay',
    revertCommand: 'revert_xbox_overlay',
  },
  {
    id: 'enable_msi_mode_gpu',
    name: 'Habilitar MSI Mode para GPU',
    description:
      'Message Signaled Interrupts reduz latência de DPC — benefício maior em GPUs RTX 30 e anteriores',
    categoryKey: 'gpu_display',
    infoCommand: 'get_msi_mode_gpu_info',
    applyCommand: 'enable_msi_mode_gpu',
    revertCommand: 'disable_msi_mode_gpu',
  },
  {
    id: 'disable_mpo',
    name: 'Desabilitar Multiplane Overlay (MPO)',
    description:
      'Remove stuttering/flickering em setups multi-monitor com refresh rates diferentes',
    categoryKey: 'gpu_display',
    infoCommand: 'get_mpo_info',
    applyCommand: 'disable_mpo',
    revertCommand: 'revert_mpo',
  },
  {
    id: 'disable_nvidia_telemetry',
    name: 'Desabilitar Telemetria NVIDIA',
    description: 'Remove coleta de telemetria do driver NVIDIA sem afetar funcionalidade',
    categoryKey: 'gpu_display',
    infoCommand: 'get_nvidia_telemetry_info',
    applyCommand: 'disable_nvidia_telemetry',
    revertCommand: 'revert_nvidia_telemetry',
  },

  // ── Gaming ──────────────────────────────────────────────────────────────────
  {
    id: 'enable_game_mode',
    name: 'Windows Game Mode',
    description:
      'Prioriza CPU e GPU para o jogo em execução, reduzindo interferência em background',
    categoryKey: 'gamer',
    infoCommand: 'get_game_mode_info',
    applyCommand: 'enable_game_mode',
    revertCommand: 'disable_game_mode',
  },
  {
    id: 'disable_vbs',
    name: 'Desabilitar VBS (Virtualização Baseada em Segurança)',
    description: 'Remove overhead de virtualização que pode reduzir FPS em 5–15%',
    categoryKey: 'gamer',
    infoCommand: 'get_vbs_info',
    applyCommand: 'disable_vbs',
    revertCommand: 'enable_vbs',
  },
  {
    id: 'enable_timer_resolution',
    name: 'Timer de Alta Resolução (1 ms)',
    description:
      'Timer resolution de 1 ms melhora frame pacing e reduz input lag em monitores 144Hz+',
    categoryKey: 'gaming',
    infoCommand: 'get_timer_resolution_info',
    applyCommand: 'enable_timer_resolution',
    revertCommand: 'disable_timer_resolution',
  },
  {
    id: 'disable_mouse_acceleration',
    name: 'Desabilitar Aceleração do Mouse',
    description: 'Remove curva não-linear do mouse — essencial para mira 1:1 em jogos FPS',
    categoryKey: 'gaming',
    infoCommand: 'get_mouse_acceleration_info',
    applyCommand: 'disable_mouse_acceleration',
    revertCommand: 'revert_mouse_acceleration',
  },
  {
    id: 'disable_fullscreen_optimizations',
    name: 'Desabilitar Fullscreen Optimizations',
    description: 'Força fullscreen exclusivo em vez do modo otimizado — beneficia jogos DX9/DX11',
    categoryKey: 'gaming',
    infoCommand: 'get_fullscreen_optimizations_info',
    applyCommand: 'disable_fullscreen_optimizations',
    revertCommand: 'revert_fullscreen_optimizations',
  },

  // ── Energia e CPU ───────────────────────────────────────────────────────────
  {
    id: 'enable_ultimate_performance',
    name: 'Plano Ultimate Performance',
    description: 'Mantém o processador em frequência máxima, eliminando latência de boost',
    categoryKey: 'energy_cpu',
    infoCommand: 'get_ultimate_performance_info',
    applyCommand: 'enable_ultimate_performance',
    revertCommand: 'revert_ultimate_performance',
  },
  {
    id: 'disable_power_throttling',
    name: 'Desabilitar Power Throttling',
    description: 'Impede redução de frequência de CPU para processos em background',
    categoryKey: 'energy_cpu',
    infoCommand: 'get_power_throttling_info',
    applyCommand: 'disable_power_throttling',
    revertCommand: 'revert_power_throttling',
  },

  // ── Armazenamento ──────────────────────────────────────────────────────────
  {
    id: 'disable_reserved_storage',
    name: 'Recuperar Armazenamento Reservado',
    description: 'Desabilita reserva de espaço do Windows para atualizações (~7 GB liberados)',
    categoryKey: 'optimization',
    infoCommand: 'get_reserved_storage_info',
    applyCommand: 'disable_reserved_storage',
    revertCommand: 'enable_reserved_storage',
  },
  {
    id: 'disable_hibernation',
    name: 'Desabilitar Hibernação',
    description: 'Remove hiberfil.sys liberando 8-16 GB e desabilita Fast Startup',
    categoryKey: 'storage',
    infoCommand: 'get_hibernation_info',
    applyCommand: 'disable_hibernation',
    revertCommand: 'enable_hibernation',
  },
  {
    id: 'disable_ntfs_last_access',
    name: 'Desabilitar Timestamp de Último Acesso NTFS',
    description: 'Reduz operações de escrita no disco ao não atualizar timestamps de acesso',
    categoryKey: 'storage',
    infoCommand: 'get_ntfs_last_access_info',
    applyCommand: 'disable_ntfs_last_access',
    revertCommand: 'revert_ntfs_last_access',
  },

  // ── Rede ────────────────────────────────────────────────────────────────────
  {
    id: 'disable_delivery_optimization',
    name: 'Desabilitar Delivery Optimization (P2P)',
    description: 'Impede que o Windows distribua atualizações pelo seu disco a terceiros',
    categoryKey: 'optimization',
    infoCommand: 'get_delivery_optimization_info',
    applyCommand: 'disable_delivery_optimization',
    revertCommand: 'revert_delivery_optimization',
  },
  {
    id: 'disable_nagle',
    name: 'Desabilitar Algoritmo de Nagle',
    description: 'Reduz latência TCP em 10-20 ms — beneficia jogos que usam TCP (MMOs, LoL)',
    categoryKey: 'network',
    infoCommand: 'get_nagle_info',
    applyCommand: 'disable_nagle',
    revertCommand: 'revert_nagle',
  },

  // ── Visual e Experiência ────────────────────────────────────────────────────
  {
    id: 'disable_wallpaper_compression',
    name: 'Desabilitar Compressão de Wallpaper',
    description: 'Define qualidade JPEG máxima para wallpapers — melhora nitidez visual',
    categoryKey: 'optimization',
    infoCommand: 'get_wallpaper_compression_info',
    applyCommand: 'disable_wallpaper_compression',
    revertCommand: 'revert_wallpaper_compression',
  },
  {
    id: 'disable_sticky_keys',
    name: 'Desabilitar Teclas de Aderência',
    description: 'Remove atalho 5x Shift que interrompe sessões de jogo acidentalmente',
    categoryKey: 'visual',
    infoCommand: 'get_sticky_keys_info',
    applyCommand: 'disable_sticky_keys',
    revertCommand: 'revert_sticky_keys',
  },
  {
    id: 'disable_bing_search',
    name: 'Desabilitar Busca Bing no Menu Iniciar',
    description: 'Buscas ficam apenas locais — mais rápidas e sem envio de dados',
    categoryKey: 'visual',
    infoCommand: 'get_bing_search_info',
    applyCommand: 'disable_bing_search',
    revertCommand: 'revert_bing_search',
  },

  // ── Privacidade ─────────────────────────────────────────────────────────────
  {
    id: 'disable_telemetry_registry',
    name: 'Desabilitar Telemetria do Windows',
    description: 'Bloqueia coleta e envio de dados de diagnóstico e uso para a Microsoft',
    categoryKey: 'privacy',
    infoCommand: 'get_telemetry_registry_info',
    applyCommand: 'disable_telemetry_registry',
    revertCommand: 'revert_telemetry_registry',
  },
  {
    id: 'disable_copilot',
    name: 'Desabilitar Copilot / Cortana',
    description: 'Remove integração do Copilot e Cortana do Windows',
    categoryKey: 'privacy',
    infoCommand: 'get_copilot_info',
    applyCommand: 'disable_copilot',
    revertCommand: 'revert_copilot',
  },
  {
    id: 'disable_content_delivery',
    name: 'Desabilitar Content Delivery Manager',
    description: 'Remove sugestões de apps e instalações automáticas de bloatware',
    categoryKey: 'privacy',
    infoCommand: 'get_content_delivery_info',
    applyCommand: 'disable_content_delivery',
    revertCommand: 'revert_content_delivery',
  },
  {
    id: 'disable_background_apps',
    name: 'Desabilitar Apps em Background',
    description: 'Impede apps da Microsoft Store de rodarem em segundo plano',
    categoryKey: 'privacy',
    infoCommand: 'get_background_apps_info',
    applyCommand: 'disable_background_apps',
    revertCommand: 'revert_background_apps',
  },

  // ── Manutenção: Limpeza ─────────────────────────────────────────────────────
  {
    id: 'flush_dns',
    name: 'Flush DNS',
    description: 'Limpa o cache de resolução DNS para corrigir problemas de conectividade',
    categoryKey: 'maintenance-clean',
    infoCommand: '',
    applyCommand: 'flush_dns',
    revertCommand: '',
  },
  {
    id: 'temp_cleanup',
    name: 'Limpeza de Arquivos Temporários',
    description: 'Remove arquivos de %TEMP%, Windows\\Temp e SoftwareDistribution\\Download',
    categoryKey: 'maintenance-clean',
    infoCommand: '',
    applyCommand: 'run_temp_cleanup',
    revertCommand: '',
  },

  // ── Manutenção: DISM ────────────────────────────────────────────────────────
  {
    id: 'dism_checkhealth',
    name: 'DISM — CheckHealth',
    description: 'Verificação rápida de integridade do componente store (sem internet)',
    categoryKey: 'maintenance-dism',
    infoCommand: '',
    applyCommand: 'run_dism_checkhealth',
    revertCommand: '',
  },
  {
    id: 'dism_scanhealth',
    name: 'DISM — ScanHealth',
    description: 'Varredura completa do componente store — mais lento e mais preciso',
    categoryKey: 'maintenance-dism',
    infoCommand: '',
    applyCommand: 'run_dism_scanhealth',
    revertCommand: '',
  },
  {
    id: 'dism_restorehealth',
    name: 'DISM — RestoreHealth',
    description: 'Repara o componente store baixando arquivos de referência da Microsoft',
    categoryKey: 'maintenance-dism',
    infoCommand: '',
    applyCommand: 'run_dism_restorehealth',
    revertCommand: '',
  },
  {
    id: 'dism_cleanup',
    name: 'DISM — StartComponentCleanup',
    description: 'Remove componentes obsoletos do Windows Component Store, liberando espaço',
    categoryKey: 'maintenance-dism',
    infoCommand: '',
    applyCommand: 'run_dism_cleanup',
    revertCommand: '',
  },

  // ── Manutenção: Verificação ─────────────────────────────────────────────────
  {
    id: 'sfc_scannow',
    name: 'SFC — System File Checker',
    description: 'Verifica e repara arquivos de sistema protegidos do Windows',
    categoryKey: 'maintenance-verify',
    infoCommand: '',
    applyCommand: 'run_sfc',
    revertCommand: '',
  },
  {
    id: 'chkdsk',
    name: 'Check Disk (C:)',
    description: 'Verifica e agenda reparo de erros lógicos e físicos no disco C:',
    categoryKey: 'maintenance-verify',
    infoCommand: '',
    applyCommand: 'run_chkdsk',
    revertCommand: '',
  },
  {
    id: 'ssd_trim',
    name: 'TRIM de SSDs',
    description: 'Executa otimização em todos os SSDs conectados para manter performance',
    categoryKey: 'maintenance-verify',
    infoCommand: '',
    applyCommand: 'run_ssd_trim',
    revertCommand: '',
  },
] as const;

// ── Índice por ID (acesso O(1)) ─────────────────────────────────────────────

const registryById = new Map<string, TweakRegistryEntry>(
  TWEAK_REGISTRY.map((entry) => [entry.id, entry]),
);

/**
 * Retorna a entrada do registro para o tweak com o ID informado.
 *
 * @param id - Identificador do tweak em snake_case (ex: `"disable_vbs"`)
 * @returns A entrada correspondente, ou `undefined` se não existir
 */
export function getTweakById(id: string): TweakRegistryEntry | undefined {
  return registryById.get(id);
}

/**
 * Retorna todas as entradas que pertencem à categoria informada.
 *
 * @param categoryKey - Chave de categoria (ex: `"gpu_display"`, `"privacy"`)
 * @returns Array de entradas filtradas (vazio se nenhuma corresponder)
 */
export function getTweaksByCategory(categoryKey: string): TweakRegistryEntry[] {
  return TWEAK_REGISTRY.filter((entry) => entry.categoryKey === categoryKey);
}

/**
 * Deriva os três dicionários de comandos Tauri a partir de uma lista de IDs.
 *
 * Útil para páginas que precisam de `infoCommands`, `applyCommands` e
 * `revertCommands` como `Record<string, string>` — elimina a necessidade
 * de declará-los manualmente.
 *
 * Entradas com comando vazio (`""`) são omitidas dos mapas resultantes
 * (ex: ações de manutenção não têm `infoCommand` nem `revertCommand`).
 *
 * @param ids - Lista de IDs de tweaks para gerar os mapas
 * @returns Objeto com `infoCommands`, `applyCommands` e `revertCommands`
 *
 * @example
 * ```ts
 * const { infoCommands, applyCommands, revertCommands } = buildCommandMaps(['disable_vbs', 'enable_hags']);
 * // infoCommands['disable_vbs'] === 'get_vbs_info'
 * ```
 */
export function buildCommandMaps(ids: string[]): {
  infoCommands: Record<string, string>;
  applyCommands: Record<string, string>;
  revertCommands: Record<string, string>;
} {
  const infoCommands: Record<string, string> = {};
  const applyCommands: Record<string, string> = {};
  const revertCommands: Record<string, string> = {};

  for (const id of ids) {
    const entry = registryById.get(id);
    if (!entry) continue;
    if (entry.infoCommand) infoCommands[id] = entry.infoCommand;
    if (entry.applyCommand) applyCommands[id] = entry.applyCommand;
    if (entry.revertCommand) revertCommands[id] = entry.revertCommand;
  }

  return { infoCommands, applyCommands, revertCommands };
}
