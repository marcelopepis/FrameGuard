// Seção de remoção de bloatware UWP.
//
// Componente self-contained renderizado na página de Privacidade.
// Escaneia apps UWP instalados via backend, exibe em tabela com checkboxes
// por categoria, e permite remoção em batch com confirmação.

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Loader2, XCircle, RefreshCw, ChevronDown, Info, AlertTriangle, CheckCircle, X,
} from 'lucide-react';
import styles from './BloatwareSection.module.css';
import { useGlobalRunning } from '../../contexts/RunningContext';
import { useToast } from '../../contexts/ToastContext';

// ── Tipos (espelham structs Rust) ────────────────────────────────────────────

interface UwpApp {
  name: string;
  package_full_name: string;
  display_name: string;
  description: string;
  category: string;
  recommended_action: 'remove' | 'optional' | 'keep';
  is_installed: boolean;
}

interface RemovalResult {
  succeeded: string[];
  failed: { name: string; display_name: string; error: string }[];
  total_requested: number;
}

// ── Categorias para agrupamento ──────────────────────────────────────────────

const APP_CATEGORIES = [
  {
    id: 'microsoft_bloatware',
    title: 'Microsoft Bloatware',
    hint: 'Apps pré-instalados da Microsoft sem valor para gaming',
  },
  {
    id: 'games_preinstalled',
    title: 'Jogos e Xbox',
    hint: 'Apps Xbox e jogos pré-instalados',
  },
  {
    id: 'third_party',
    title: 'Apps de terceiros',
    hint: 'Instalados por parcerias OEM',
  },
  {
    id: 'useful',
    title: 'Opcionais úteis',
    hint: 'Apps que podem ser úteis ocasionalmente',
  },
  {
    id: 'system',
    title: 'Sistema (protegidos)',
    hint: 'Apps essenciais — remoção bloqueada',
  },
];

// ── Helpers ──────────────────────────────────────────────────────────────────

function badgeClass(action: string): string {
  switch (action) {
    case 'remove':   return `${styles.badge} ${styles.badgeRemove}`;
    case 'optional': return `${styles.badge} ${styles.badgeOptional}`;
    case 'keep':     return `${styles.badge} ${styles.badgeKeep}`;
    default:         return styles.badge;
  }
}

function badgeLabel(action: string): string {
  switch (action) {
    case 'remove':   return 'Remover';
    case 'optional': return 'Opcional';
    case 'keep':     return 'Manter';
    default:         return action;
  }
}

// ── Componente ───────────────────────────────────────────────────────────────

export default function BloatwareSection() {
  const [apps, setApps] = useState<UwpApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [showConfirm, setShowConfirm] = useState(false);
  const [lastResult, setLastResult] = useState<RemovalResult | null>(null);

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();
  const globalDisabled = isRunning || removing;

  // ── Carregamento ──

  const loadApps = useCallback(async () => {
    setLoading(true);
    setError(null);
    setLastResult(null);
    try {
      const result = await invoke<UwpApp[]>('get_installed_uwp_apps');
      setApps(result);

      // Pré-selecionar apps recomendados para remoção que estão instalados
      const recommended = new Set<string>();
      for (const app of result) {
        if (app.recommended_action === 'remove' && app.is_installed) {
          recommended.add(app.name);
        }
      }
      setSelected(recommended);
    } catch (e) {
      setError(`Erro ao escanear apps UWP: ${e}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadApps(); }, [loadApps]);

  // ── Toggle ──

  function toggleCategory(catId: string) {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(catId)) next.delete(catId);
      else next.add(catId);
      return next;
    });
  }

  function toggleApp(name: string) {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }

  function selectRecommended() {
    const recommended = new Set<string>();
    for (const app of apps) {
      if (app.recommended_action === 'remove' && app.is_installed) {
        recommended.add(app.name);
      }
    }
    setSelected(recommended);
  }

  // ── Remoção ──

  async function handleRemove() {
    const names = Array.from(selected);
    if (names.length === 0) return;

    setShowConfirm(false);
    setRemoving(true);
    setLastResult(null);

    try {
      const result = await invoke<RemovalResult>('remove_uwp_apps', { names });
      setLastResult(result);

      if (result.succeeded.length > 0) {
        showToast('success', 'Apps removidos',
          `${result.succeeded.length} app(s) removido(s) com sucesso.`);
        invoke('log_tweak_activity', {
          name: `Remover ${result.succeeded.length} app(s) UWP`,
          applied: true, success: true,
        }).catch(() => {});
      }

      if (result.failed.length > 0) {
        for (const f of result.failed) {
          showToast('error', `Falha: ${f.display_name}`, f.error);
        }
        if (result.succeeded.length === 0) {
          invoke('log_tweak_activity', {
            name: `Remover apps UWP (${result.failed.length} falha(s))`,
            applied: true, success: false,
          }).catch(() => {});
        }
      }

      // Rescan para atualizar lista
      const updated = await invoke<UwpApp[]>('get_installed_uwp_apps');
      setApps(updated);
      setSelected(new Set());
    } catch (e) {
      showToast('error', 'Erro ao remover apps', String(e));
      invoke('log_tweak_activity', {
        name: 'Remover apps UWP', applied: true, success: false,
      }).catch(() => {});
    } finally {
      setRemoving(false);
    }
  }

  // ── Render: Loading ──

  if (loading) {
    return (
      <div className={styles.sectionCard}>
        <div className={styles.sectionHeader}>
          <span className={styles.sectionTitle}>Remoção de Bloatware UWP</span>
        </div>
        <div className={styles.loadingState}>
          <Loader2 size={16} className={styles.spinner} />
          <span>Escaneando apps instalados...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={styles.sectionCard}>
        <div className={styles.sectionHeader}>
          <span className={styles.sectionTitle}>Remoção de Bloatware UWP</span>
        </div>
        <div className={styles.errorState}>
          <XCircle size={16} />
          <span>{error}</span>
          <button className={styles.btnRetry} onClick={loadApps}>Tentar novamente</button>
        </div>
      </div>
    );
  }

  // ── Contagens ──

  const installedCount = apps.filter(a => a.is_installed).length;
  const selectedInstalled = Array.from(selected).filter(name => {
    const app = apps.find(a => a.name === name);
    return app?.is_installed && app?.recommended_action !== 'keep';
  });

  return (
    <div className={styles.sectionCard}>

      {/* ── Header ── */}
      <div className={styles.sectionHeader}>
        <span className={styles.sectionTitle}>Remoção de Bloatware UWP</span>
        <span className={styles.sectionSubtitle}>
          {installedCount} instalado(s) de {apps.length} na lista curada
        </span>
      </div>

      {/* ── Banner informativo ── */}
      <div className={styles.infoBanner}>
        <Info size={15} />
        <span>
          Apps removidos podem ser reinstalados pela Microsoft Store.
          Apps marcados como "Manter" são protegidos e não podem ser removidos.
        </span>
      </div>

      {/* ── Resultado da última remoção ── */}
      {lastResult && (
        <div className={`${styles.resultBanner} ${
          lastResult.failed.length > 0 ? styles.resultPartial : styles.resultSuccess
        }`}>
          {lastResult.failed.length > 0
            ? <AlertTriangle size={16} />
            : <CheckCircle size={16} />
          }
          <div className={styles.resultDetails}>
            <span>
              {lastResult.succeeded.length > 0 &&
                `${lastResult.succeeded.length} removido(s) com sucesso.`}
              {lastResult.failed.length > 0 &&
                ` ${lastResult.failed.length} falha(s).`}
            </span>
            {lastResult.failed.length > 0 && (
              <span style={{ fontSize: '11.5px', opacity: 0.8 }}>
                Falhas: {lastResult.failed.map(f => f.display_name).join(', ')}
              </span>
            )}
          </div>
          <button className={styles.resultDismiss} onClick={() => setLastResult(null)}>
            <X size={14} />
          </button>
        </div>
      )}

      {/* ── Categorias com tabelas ── */}
      {APP_CATEGORIES.map(cat => {
        const catApps = apps.filter(a => a.category === cat.id);
        if (catApps.length === 0) return null;

        const catKey = `bloat_${cat.id}`;
        const isCollapsed = collapsed.has(catKey);
        const catInstalled = catApps.filter(a => a.is_installed).length;

        return (
          <div key={cat.id}>
            <div
              className={styles.categoryHeader}
              onClick={() => toggleCategory(catKey)}
            >
              <ChevronDown
                size={14}
                className={`${styles.chevron} ${isCollapsed ? styles.chevronCollapsed : ''}`}
              />
              <span className={styles.categoryTitle}>{cat.title}</span>
              <span className={styles.categoryCount}>
                {catInstalled}/{catApps.length} instalado(s)
              </span>
              <span className={styles.categoryHint}>{cat.hint}</span>
            </div>

            {!isCollapsed && (
              <table className={styles.table}>
                <thead>
                  <tr className={styles.tableHead}>
                    <th></th>
                    <th>Nome</th>
                    <th>Descrição</th>
                    <th className={styles.colRecommendation}>Recomendação</th>
                  </tr>
                </thead>
                <tbody>
                  {catApps.map(app => {
                    const isKeep = app.recommended_action === 'keep';
                    const notInstalled = !app.is_installed;

                    return (
                      <tr
                        key={app.name}
                        className={`${styles.tableRow} ${
                          notInstalled ? styles.rowNotInstalled : ''
                        } ${isKeep ? styles.rowKeep : ''}`}
                      >
                        <td>
                          <input
                            type="checkbox"
                            className={styles.checkbox}
                            checked={selected.has(app.name)}
                            onChange={() => toggleApp(app.name)}
                            disabled={globalDisabled || isKeep || notInstalled}
                          />
                        </td>
                        <td>
                          <div className={styles.nameCell}>
                            <span className={styles.displayName}>
                              {app.display_name}
                            </span>
                            <span className={styles.techName}>{app.name}</span>
                          </div>
                        </td>
                        <td className={styles.descCell}>{app.description}</td>
                        <td>
                          <span className={badgeClass(app.recommended_action)}>
                            {badgeLabel(app.recommended_action)}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        );
      })}

      {/* ── Confirmação inline ── */}
      {showConfirm && (
        <div className={styles.confirmOverlay}>
          <AlertTriangle size={16} />
          <span className={styles.confirmText}>
            Remover {selectedInstalled.length} app(s)? Alguns podem ser reinstalados pela Microsoft Store.
          </span>
          <button className={styles.btnConfirm} onClick={handleRemove}>
            Confirmar Remoção
          </button>
          <button className={styles.btnCancel} onClick={() => setShowConfirm(false)}>
            Cancelar
          </button>
        </div>
      )}

      {/* ── Barra de ações ── */}
      <div className={styles.actionBar}>
        <span className={styles.selectionCount}>
          {selectedInstalled.length > 0
            ? `${selectedInstalled.length} selecionado(s)`
            : 'Nenhum selecionado'
          }
        </span>
        <button
          className={styles.btnSecondary}
          onClick={selectRecommended}
          disabled={globalDisabled}
        >
          Selecionar Recomendados
        </button>
        <button
          className={styles.btnPrimary}
          onClick={() => setShowConfirm(true)}
          disabled={globalDisabled || selectedInstalled.length === 0}
        >
          {removing && <Loader2 size={14} className={styles.spinner} />}
          Remover Selecionados ({selectedInstalled.length})
        </button>
        <button
          className={styles.btnRefreshSmall}
          onClick={loadApps}
          disabled={globalDisabled}
          title="Rescanear apps"
        >
          <RefreshCw size={13} />
        </button>
      </div>
    </div>
  );
}
