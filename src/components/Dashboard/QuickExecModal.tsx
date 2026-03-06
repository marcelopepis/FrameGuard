import { useState } from 'react';
import {
  CheckCircle2,
  XCircle,
  MinusCircle,
  Clock,
  Loader2,
  X,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import type { Plan, ExecState, ItemStatus } from '../../hooks';
import { formatDuration } from './ActivityItem';
import styles from '../../pages/Dashboard.module.css';

// Mapa de tweaks para nomes legíveis
const TWEAK_NAMES: Record<string, string> = {
  disable_wallpaper_compression: 'Desabilitar Compressão de Wallpaper',
  disable_reserved_storage: 'Recuperar Armazenamento Reservado',
  disable_delivery_optimization: 'Desabilitar Delivery Optimization',
  enable_hags: 'HAGS',
  enable_game_mode: 'Game Mode',
  disable_vbs: 'Desabilitar VBS',
  disable_game_dvr: 'Desabilitar Game DVR',
  disable_xbox_overlay: 'Desabilitar Xbox Overlay',
  enable_msi_mode_gpu: 'MSI Mode GPU',
  disable_mpo: 'Desabilitar MPO',
  disable_nvidia_telemetry: 'Desabilitar Telemetria NVIDIA',
  enable_timer_resolution: 'Timer 1 ms',
  disable_mouse_acceleration: 'Desabilitar Aceleração do Mouse',
  disable_fullscreen_optimizations: 'Desabilitar Fullscreen Optimizations',
  enable_ultimate_performance: 'Ultimate Performance',
  disable_power_throttling: 'Desabilitar Power Throttling',
  disable_hibernation: 'Desabilitar Hibernação',
  disable_ntfs_last_access: 'Desabilitar Timestamp NTFS',
  disable_nagle: 'Desabilitar Nagle',
  disable_sticky_keys: 'Desabilitar Teclas de Aderência',
  disable_bing_search: 'Desabilitar Bing no Menu Iniciar',
  disable_telemetry_registry: 'Desabilitar Telemetria Windows',
  disable_copilot: 'Desabilitar Copilot / Cortana',
  disable_content_delivery: 'Desabilitar Content Delivery',
  disable_background_apps: 'Desabilitar Apps em Background',
  flush_dns: 'Flush DNS',
  temp_cleanup: 'Limpeza de Temporários',
  dism_checkhealth: 'DISM — CheckHealth',
  dism_scanhealth: 'DISM — ScanHealth',
  dism_restorehealth: 'DISM — RestoreHealth',
  dism_cleanup: 'DISM — ComponentCleanup',
  sfc_scannow: 'SFC — System File Checker',
  chkdsk: 'Check Disk (C:)',
  ssd_trim: 'TRIM de SSDs',
};

function ExecStatusIcon({
  status,
  hcStatus,
}: {
  status: ItemStatus;
  hcStatus?: 'success' | 'warning' | 'error';
}) {
  if (status === 'pending') return <Clock size={14} strokeWidth={2} className={styles.stPending} />;
  if (status === 'running')
    return <Loader2 size={14} strokeWidth={2} className={styles.stRunning} />;
  if (status === 'failed') return <XCircle size={14} strokeWidth={2} className={styles.stFailed} />;
  if (status === 'skipped')
    return <MinusCircle size={14} strokeWidth={2} className={styles.stSkipped} />;
  if (hcStatus === 'warning')
    return <CheckCircle2 size={14} strokeWidth={2} className={styles.stWarning} />;
  if (hcStatus === 'error')
    return <CheckCircle2 size={14} strokeWidth={2} className={styles.stFailed} />;
  return <CheckCircle2 size={14} strokeWidth={2} className={styles.stCompleted} />;
}

export function QuickExecModal({
  plan,
  state,
  onClose,
}: {
  plan: Plan;
  state: ExecState;
  onClose: () => void;
}) {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const sortedItems = [...plan.items].sort((a, b) => a.order - b.order);
  const isDone = !state.running;

  function toggleExpand(id: string) {
    setExpandedItems((prev) => {
      const s = new Set(prev);
      if (s.has(id)) s.delete(id);
      else s.add(id);
      return s;
    });
  }

  return (
    <div className={styles.execOverlay}>
      <div className={styles.execModal} onClick={(e) => e.stopPropagation()}>
        {/* Cabeçalho */}
        <div className={styles.execHeader}>
          <div className={styles.execTitleGroup}>
            <h2 className={styles.execTitle}>{plan.name}</h2>
            <p className={styles.execSubtitle}>
              {state.running
                ? `Executando… ${state.progress}%`
                : state.fatalError
                  ? 'Execução interrompida com erro'
                  : state.summary
                    ? `Concluído em ${formatDuration(state.summary.duration_seconds)}`
                    : 'Concluído'}
            </p>
          </div>
          <button
            className={styles.execClose}
            onClick={onClose}
            disabled={state.running}
            title={state.running ? 'Aguarde a conclusão' : 'Fechar'}
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* Barra de progresso */}
        <div className={styles.execProgress}>
          <div
            className={`${styles.execProgressFill} ${isDone && !state.fatalError ? styles.execProgressDone : ''}`}
            style={{ width: `${state.progress}%` }}
          />
        </div>

        {/* Lista de itens */}
        <div className={styles.execItemList}>
          {sortedItems.map((item) => {
            const itemState = state.items[item.tweak_id] ?? { status: 'pending' as ItemStatus };
            const isExpanded = expandedItems.has(item.tweak_id);
            const hasDetails = !!(itemState.message || itemState.details || itemState.error);

            return (
              <div
                key={item.tweak_id}
                className={`${styles.execItemRow} ${styles[`execSt_${itemState.status}`] ?? ''}`}
              >
                <div className={styles.execItemMain}>
                  <ExecStatusIcon status={itemState.status} hcStatus={itemState.hcStatus} />
                  <div className={styles.execItemText}>
                    <span className={styles.execItemName}>
                      {TWEAK_NAMES[item.tweak_id] ?? item.tweak_id}
                    </span>
                    {itemState.message && !isExpanded && (
                      <span className={styles.execItemMsg}>{itemState.message}</span>
                    )}
                  </div>
                  {hasDetails && (
                    <button
                      className={styles.execExpandBtn}
                      onClick={() => toggleExpand(item.tweak_id)}
                    >
                      {isExpanded ? (
                        <ChevronUp size={13} strokeWidth={2} />
                      ) : (
                        <ChevronDown size={13} strokeWidth={2} />
                      )}
                    </button>
                  )}
                </div>
                {isExpanded && hasDetails && (
                  <div className={styles.execItemDetails}>
                    {itemState.error && <p className={styles.execError}>{itemState.error}</p>}
                    {itemState.message && <p className={styles.execMsg}>{itemState.message}</p>}
                    {itemState.details && <pre className={styles.execLog}>{itemState.details}</pre>}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Resumo final */}
        {isDone && state.summary && (
          <div className={styles.execFooter}>
            <div className={styles.execStats}>
              <span className={styles.execStatSuccess}>
                <CheckCircle2 size={13} strokeWidth={2.5} />
                {state.summary.completed_count}
              </span>
              {state.summary.failed_count > 0 && (
                <span className={styles.execStatFailed}>
                  <XCircle size={13} strokeWidth={2.5} />
                  {state.summary.failed_count}
                </span>
              )}
              {state.summary.skipped_count > 0 && (
                <span className={styles.execStatSkipped}>
                  <MinusCircle size={13} strokeWidth={2.5} />
                  {state.summary.skipped_count}
                </span>
              )}
            </div>
            <button className={styles.execCloseBtn} onClick={onClose}>
              Fechar
            </button>
          </div>
        )}

        {isDone && state.fatalError && (
          <div className={styles.execFooter}>
            <p className={styles.execError}>{state.fatalError}</p>
            <button className={styles.execCloseBtn} onClick={onClose}>
              Fechar
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
