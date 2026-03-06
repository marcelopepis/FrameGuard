/**
 * Hook para detecção de hardware e filtragem de tweaks vendor-specific.
 *
 * Chama `get_detected_vendors()` uma vez e fornece funções para filtrar tweaks
 * incompatíveis com o hardware do usuário. Fallback seguro: se a detecção
 * falhar, mostra todos os tweaks (nunca esconde por engano).
 *
 * @module useHardwareFilter
 */

import { useState, useEffect, useCallback } from 'react';
import { getDetectedVendors } from '../services/systemInfo';
import type { DetectedVendors } from '../services/systemInfo';

// ── Mapa estático de tweaks vendor-specific ─────────────────────────────────
//
// Mantido em sincronia com:
//   - get_tweak_hardware_filter() em plans.rs
//   - campo hardware_filter do TweakInfo em optimizations.rs

/** Filtro de compatibilidade de hardware para um tweak vendor-specific. */
export interface HardwareFilter {
  /** Vendor de GPU requerido (ex: `"nvidia"`, `"amd"`) — `undefined` = qualquer GPU */
  gpu_vendor?: string;
  /** Vendor de CPU requerido (ex: `"intel"`, `"amd"`) — `undefined` = qualquer CPU */
  cpu_vendor?: string;
}

/** Tweaks que requerem hardware específico. Ausência = universal. */
export const TWEAK_HARDWARE_MAP: Record<string, HardwareFilter> = {
  disable_nvidia_telemetry: { gpu_vendor: 'nvidia' },
  // Adicionar futuros tweaks vendor-specific aqui
};

// ── Hook ────────────────────────────────────────────────────────────────────

/**
 * Detecta o hardware do usuário e fornece funções de filtragem de tweaks.
 *
 * Chama `get_detected_vendors()` no mount. Enquanto a detecção não completa
 * ou se falhar, todos os tweaks são considerados compatíveis (fallback seguro).
 *
 * @returns Objeto com:
 *   - `vendors` — vendors detectados (ou `null` enquanto carrega)
 *   - `isCompatible(tweakId)` — verifica compatibilidade de um tweak
 *   - `getVendorBadge(tweakId)` — retorna label do badge (ex: `"NVIDIA"`) ou `null`
 *   - `filterCompatible(ids)` — filtra array mantendo apenas compatíveis
 *
 * @example
 * ```tsx
 * const { filterCompatible, getVendorBadge } = useHardwareFilter();
 * const visibleTweaks = filterCompatible(allTweakIds);
 * ```
 */
export function useHardwareFilter() {
  const [vendors, setVendors] = useState<DetectedVendors | null>(null);

  useEffect(() => {
    getDetectedVendors()
      .then(setVendors)
      .catch(() => {
        // Fallback: show all tweaks if detection fails
        setVendors({ gpu_vendor: 'unknown', cpu_vendor: 'unknown' });
      });
  }, []);

  /** Retorna true se o tweak é compatível com o hardware detectado. */
  const isCompatible = useCallback(
    (tweakId: string): boolean => {
      if (!vendors) return true; // ainda carregando → mostra tudo
      const filter = TWEAK_HARDWARE_MAP[tweakId];
      if (!filter) return true; // tweak universal

      if (filter.gpu_vendor && filter.gpu_vendor !== vendors.gpu_vendor) return false;
      if (filter.cpu_vendor && filter.cpu_vendor !== vendors.cpu_vendor) return false;
      return true;
    },
    [vendors],
  );

  /** Retorna label do badge de vendor ("NVIDIA", "AMD", etc.) ou null se universal. */
  const getVendorBadge = useCallback((tweakId: string): string | null => {
    const filter = TWEAK_HARDWARE_MAP[tweakId];
    if (!filter) return null;
    if (filter.gpu_vendor) return filter.gpu_vendor.toUpperCase();
    if (filter.cpu_vendor) return filter.cpu_vendor.toUpperCase();
    return null;
  }, []);

  /** Filtra array de tweak IDs mantendo apenas os compatíveis. */
  const filterCompatible = useCallback(
    (tweakIds: string[]): string[] => tweakIds.filter(isCompatible),
    [isCompatible],
  );

  return { vendors, isCompatible, getVendorBadge, filterCompatible };
}
