// Página de Planos de Execução do FrameGuard.
//
// Permite criar rotinas de manutenção personalizadas combinando qualquer
// subconjunto dos tweaks disponíveis, definindo a ordem de execução e
// rodando tudo com um clique — com feedback em tempo real via eventos Tauri.

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Plus, Play, Pencil, Trash2, GripVertical,
  CheckCircle2, XCircle, Clock, MinusCircle,
  Loader2, X, ChevronDown, ChevronUp,
  ClipboardList,
} from 'lucide-react';
import styles from './Plans.module.css';

// ── Tipos ──────────────────────────────────────────────────────────────────────

interface PlanItem {
  tweak_id: string;
  order: number;
  enabled: boolean;
}

interface Plan {
  id: string;
  name: string;
  description: string;
  created_at: string;
  last_executed: string | null;
  items: PlanItem[];
}

interface HealthCheckData {
  id: string;
  name: string;
  status: 'success' | 'warning' | 'error';
  message: string;
  details?: string;
  duration_seconds: number;
  space_freed_mb?: number;
}

interface ItemResult {
  tweak_id: string;
  status: 'completed' | 'failed' | 'skipped';
  error: string | null;
  result_data: HealthCheckData | null;
}

interface PlanProgress {
  plan_id: string;
  current_item: string;
  current_item_index: number;
  total_items: number;
  item_status: 'running' | 'completed' | 'failed' | 'skipped';
  item_result: ItemResult | null;
  overall_progress_percent: number;
}

interface PlanExecutionSummary {
  plan_id: string;
  plan_name: string;
  duration_seconds: number;
  total_items: number;
  completed_count: number;
  failed_count: number;
  skipped_count: number;
  results: ItemResult[];
}

type ItemStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

interface ItemExecState {
  status: ItemStatus;
  message?: string;
  details?: string;
  error?: string;
  hcStatus?: 'success' | 'warning' | 'error';
}

interface ExecState {
  running: boolean;
  items: Record<string, ItemExecState>;
  progress: number;
  summary: PlanExecutionSummary | null;
  fatalError: string | null;
}

// ── Catálogo de tweaks disponíveis ────────────────────────────────────────────

interface TweakMeta {
  id: string;
  name: string;
  description: string;
  categoryKey: string;
}

const TWEAKS: TweakMeta[] = [
  // Otimizações
  {
    id: 'disable_wallpaper_compression',
    name: 'Desabilitar Compressão de Wallpaper',
    description: 'Define qualidade JPEG máxima para wallpapers — melhora nitidez visual',
    categoryKey: 'optimization',
  },
  {
    id: 'disable_reserved_storage',
    name: 'Recuperar Armazenamento Reservado',
    description: 'Desabilita reserva de espaço do Windows para atualizações (~7 GB liberados)',
    categoryKey: 'optimization',
  },
  {
    id: 'disable_delivery_optimization',
    name: 'Desabilitar Delivery Optimization (P2P)',
    description: 'Impede que o Windows distribua atualizações pelo seu disco a terceiros',
    categoryKey: 'optimization',
  },
  // Saúde — DISM
  {
    id: 'dism_cleanup',
    name: 'DISM — Limpeza de Componentes',
    description: 'Remove componentes obsoletos do Windows Component Store',
    categoryKey: 'health-dism',
  },
  {
    id: 'dism_checkhealth',
    name: 'DISM — CheckHealth',
    description: 'Verificação rápida de integridade do componente store (sem internet)',
    categoryKey: 'health-dism',
  },
  {
    id: 'dism_scanhealth',
    name: 'DISM — ScanHealth',
    description: 'Varredura completa do componente store — mais lento e mais preciso',
    categoryKey: 'health-dism',
  },
  {
    id: 'dism_restorehealth',
    name: 'DISM — RestoreHealth',
    description: 'Repara o componente store baixando arquivos de referência da Microsoft',
    categoryKey: 'health-dism',
  },
  // Saúde — Verificações
  {
    id: 'sfc_scannow',
    name: 'SFC — System File Checker',
    description: 'Verifica e repara arquivos de sistema protegidos do Windows',
    categoryKey: 'health-verify',
  },
  {
    id: 'chkdsk',
    name: 'Check Disk (C:)',
    description: 'Verifica e agenda reparo de erros lógicos e físicos no disco C:',
    categoryKey: 'health-verify',
  },
  {
    id: 'ssd_trim',
    name: 'TRIM de SSDs',
    description: 'Executa otimização em todos os SSDs conectados para manter performance',
    categoryKey: 'health-verify',
  },
  // Saúde — Manutenção
  {
    id: 'flush_dns',
    name: 'Flush DNS',
    description: 'Limpa o cache de resolução DNS para corrigir problemas de conectividade',
    categoryKey: 'health-maintain',
  },
  {
    id: 'temp_cleanup',
    name: 'Limpeza de Arquivos Temporários',
    description: 'Remove arquivos de %TEMP%, Windows\\Temp e SoftwareDistribution\\Download',
    categoryKey: 'health-maintain',
  },
];

const TWEAK_MAP: Record<string, TweakMeta> = Object.fromEntries(
  TWEAKS.map(t => [t.id, t]),
);

const CATEGORIES = [
  { key: 'optimization',    label: 'Otimizações'                     },
  { key: 'health-dism',     label: 'Saúde do Sistema — DISM'         },
  { key: 'health-verify',   label: 'Saúde do Sistema — Verificações' },
  { key: 'health-maintain', label: 'Saúde do Sistema — Manutenção'   },
];

// ── Helpers ────────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString('pt-BR', {
      day: '2-digit', month: '2-digit', year: 'numeric',
      hour: '2-digit', minute: '2-digit',
    });
  } catch {
    return iso;
  }
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
}

// ── Componente principal ───────────────────────────────────────────────────────

type PageView = 'list' | 'edit';

export default function Plans() {
  const [view, setView] = useState<PageView>('list');
  const [plans, setPlans] = useState<Plan[]>([]);
  const [editingPlan, setEditingPlan] = useState<Plan | null>(null);
  const [loading, setLoading] = useState(true);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  // Modal de execução
  const [executingPlan, setExecutingPlan] = useState<Plan | null>(null);
  const [execState, setExecState] = useState<ExecState | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    loadPlans();
    // Limpa listener se o componente desmontar durante uma execução
    return () => { unlistenRef.current?.(); };
  }, []);

  async function loadPlans() {
    try {
      setLoading(true);
      const result = await invoke<Plan[]>('get_all_plans');
      setPlans(result);
    } catch (err) {
      console.error('[Plans] Erro ao carregar planos:', err);
    } finally {
      setLoading(false);
    }
  }

  function handleNewPlan() {
    setEditingPlan(null);
    setView('edit');
  }

  function handleEditPlan(plan: Plan) {
    setEditingPlan(plan);
    setView('edit');
  }

  async function handleDeletePlan(planId: string) {
    // Primeiro clique: pede confirmação inline
    if (confirmDeleteId !== planId) {
      setConfirmDeleteId(planId);
      return;
    }
    setConfirmDeleteId(null);
    try {
      await invoke('delete_plan', { planId });
      await loadPlans();
    } catch (err) {
      console.error('[Plans] Erro ao excluir plano:', err);
    }
  }

  async function handleSavePlan(
    name: string,
    description: string,
    items: PlanItem[],
    planId?: string,
  ) {
    try {
      if (planId) {
        await invoke('update_plan', { planId, name, description, items });
      } else {
        await invoke('create_plan', { name, description, items });
      }
      await loadPlans();
      setView('list');
    } catch (err) {
      console.error('[Plans] Erro ao salvar plano:', err);
    }
  }

  async function handleExecutePlan(plan: Plan) {
    // Inicializa estado: todos os itens como 'pending'
    const initialItems: Record<string, ItemExecState> = {};
    plan.items.forEach(item => {
      initialItems[item.tweak_id] = { status: 'pending' };
    });

    setExecutingPlan(plan);
    setExecState({ running: true, items: initialItems, progress: 0, summary: null, fatalError: null });

    // Registra listener ANTES do invoke (para não perder eventos iniciais)
    const unlisten = await listen<PlanProgress>('plan_progress', (event) => {
      const p = event.payload;

      const newItemState: ItemExecState = { status: p.item_status };

      if (p.item_result?.error) {
        newItemState.error = p.item_result.error;
      }
      if (p.item_result?.result_data) {
        const rd = p.item_result.result_data;
        newItemState.message = rd.message;
        newItemState.details = rd.details;
        newItemState.hcStatus = rd.status;
      }

      setExecState(prev => prev ? {
        ...prev,
        progress: p.overall_progress_percent,
        items: { ...prev.items, [p.current_item]: newItemState },
      } : prev);
    });

    unlistenRef.current = unlisten;

    try {
      const summary = await invoke<PlanExecutionSummary>('execute_plan', { planId: plan.id });
      setExecState(prev => prev
        ? { ...prev, running: false, summary, progress: 100 }
        : prev,
      );
      // Atualiza lista para refletir o novo last_executed
      await loadPlans();
    } catch (err) {
      setExecState(prev => prev
        ? { ...prev, running: false, fatalError: String(err) }
        : prev,
      );
    } finally {
      unlisten();
      unlistenRef.current = null;
    }
  }

  function handleCloseModal() {
    if (execState?.running) return;
    setExecutingPlan(null);
    setExecState(null);
  }

  if (view === 'edit') {
    return (
      <PlanEditor
        initial={editingPlan}
        onSave={handleSavePlan}
        onCancel={() => setView('list')}
      />
    );
  }

  return (
    <div className={styles.page}>
      {/* Cabeçalho */}
      <header className={styles.header}>
        <div>
          <h1 className={styles.title}>Planos de Execução</h1>
          <p className={styles.subtitle}>Crie rotinas de manutenção personalizadas</p>
        </div>
        <button className={styles.btnPrimary} onClick={handleNewPlan}>
          <Plus size={15} strokeWidth={2.5} />
          Novo Plano
        </button>
      </header>

      {/* Lista de planos */}
      {loading ? (
        <p className={styles.loading}>Carregando planos…</p>
      ) : plans.length === 0 ? (
        <EmptyState onNew={handleNewPlan} />
      ) : (
        <div className={styles.grid}>
          {plans.map(plan => (
            <PlanCard
              key={plan.id}
              plan={plan}
              confirmDeleteId={confirmDeleteId}
              onEdit={() => handleEditPlan(plan)}
              onDelete={() => handleDeletePlan(plan.id)}
              onDeleteCancel={() => setConfirmDeleteId(null)}
              onRun={() => handleExecutePlan(plan)}
            />
          ))}
        </div>
      )}

      {/* Modal de execução */}
      {executingPlan && execState && (
        <ExecutionModal
          plan={executingPlan}
          state={execState}
          onClose={handleCloseModal}
        />
      )}
    </div>
  );
}

// ── EmptyState ─────────────────────────────────────────────────────────────────

function EmptyState({ onNew }: { onNew: () => void }) {
  return (
    <div className={styles.emptyState}>
      <ClipboardList size={52} strokeWidth={1.2} className={styles.emptyIcon} />
      <h3 className={styles.emptyTitle}>Nenhum plano criado</h3>
      <p className={styles.emptyText}>
        Crie seu primeiro plano para automatizar sequências de manutenção
        e executar múltiplos tweaks com um único clique.
      </p>
      <button className={styles.btnPrimary} onClick={onNew}>
        <Plus size={15} strokeWidth={2.5} />
        Criar primeiro plano
      </button>
    </div>
  );
}

// ── PlanCard ───────────────────────────────────────────────────────────────────

interface PlanCardProps {
  plan: Plan;
  confirmDeleteId: string | null;
  onEdit: () => void;
  onDelete: () => void;
  onDeleteCancel: () => void;
  onRun: () => void;
}

function PlanCard({ plan, confirmDeleteId, onEdit, onDelete, onDeleteCancel, onRun }: PlanCardProps) {
  const isConfirmingDelete = confirmDeleteId === plan.id;
  const enabledCount = plan.items.filter(i => i.enabled).length;

  return (
    <div className={styles.planCard}>
      <div className={styles.cardBody}>
        <h3 className={styles.planName}>{plan.name}</h3>
        {plan.description && (
          <p className={styles.planDesc}>{plan.description}</p>
        )}
        <div className={styles.planMeta}>
          <span className={styles.metaChip}>
            {enabledCount} {enabledCount === 1 ? 'item' : 'itens'}
          </span>
          <span className={styles.metaSep}>·</span>
          <span className={styles.metaDate}>
            {plan.last_executed
              ? `Último: ${formatDate(plan.last_executed)}`
              : 'Nunca executado'}
          </span>
        </div>
      </div>

      <div className={styles.cardActions}>
        <button
          className={styles.btnRun}
          onClick={onRun}
          title="Executar plano"
          disabled={enabledCount === 0}
        >
          <Play size={13} strokeWidth={2.5} fill="currentColor" />
          Executar
        </button>
        <button className={styles.btnIcon} onClick={onEdit} title="Editar plano">
          <Pencil size={14} strokeWidth={2} />
        </button>
        {isConfirmingDelete ? (
          <>
            <button className={styles.btnDanger} onClick={onDelete}>
              Confirmar
            </button>
            <button className={styles.btnGhost} onClick={onDeleteCancel}>
              Cancelar
            </button>
          </>
        ) : (
          <button className={styles.btnIcon} onClick={onDelete} title="Excluir plano">
            <Trash2 size={14} strokeWidth={2} />
          </button>
        )}
      </div>
    </div>
  );
}

// ── PlanEditor ─────────────────────────────────────────────────────────────────

interface PlanEditorProps {
  initial: Plan | null;
  onSave: (name: string, description: string, items: PlanItem[], id?: string) => Promise<void>;
  onCancel: () => void;
}

function PlanEditor({ initial, onSave, onCancel }: PlanEditorProps) {
  const [name, setName] = useState(initial?.name ?? '');
  const [description, setDescription] = useState(initial?.description ?? '');
  const [checkedIds, setCheckedIds] = useState<Set<string>>(
    () => new Set(initial?.items.map(i => i.tweak_id) ?? []),
  );
  const [orderedIds, setOrderedIds] = useState<string[]>(
    () => initial
      ? [...initial.items].sort((a, b) => a.order - b.order).map(i => i.tweak_id)
      : [],
  );
  const [saving, setSaving] = useState(false);

  // Drag-and-drop state
  const draggedRef = useRef<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);

  function toggleTweak(id: string, checked: boolean) {
    if (checked) {
      setCheckedIds(prev => new Set([...prev, id]));
      setOrderedIds(prev => [...prev, id]);
    } else {
      setCheckedIds(prev => { const s = new Set(prev); s.delete(id); return s; });
      setOrderedIds(prev => prev.filter(x => x !== id));
    }
  }

  function handleDragStart(id: string) {
    draggedRef.current = id;
  }

  function handleDragEnter(id: string) {
    if (draggedRef.current && draggedRef.current !== id) {
      setDragOverId(id);
    }
  }

  function handleDragOver(e: React.DragEvent) {
    e.preventDefault();
  }

  function handleDrop(targetId: string) {
    setDragOverId(null);
    const source = draggedRef.current;
    if (!source || source === targetId) return;

    setOrderedIds(prev => {
      const arr = [...prev];
      const fromIdx = arr.indexOf(source);
      const toIdx = arr.indexOf(targetId);
      if (fromIdx < 0 || toIdx < 0) return prev;
      arr.splice(fromIdx, 1);
      arr.splice(toIdx, 0, source);
      return arr;
    });

    draggedRef.current = null;
  }

  function handleDragEnd() {
    draggedRef.current = null;
    setDragOverId(null);
  }

  async function handleSave() {
    if (!name.trim() || orderedIds.length === 0) return;
    setSaving(true);
    try {
      const items: PlanItem[] = orderedIds.map((id, idx) => ({
        tweak_id: id,
        order: idx,
        enabled: true,
      }));
      await onSave(name.trim(), description.trim(), items, initial?.id);
    } finally {
      setSaving(false);
    }
  }

  const isValid = name.trim().length > 0 && orderedIds.length > 0;

  return (
    <div className={styles.page}>
      {/* Cabeçalho do editor */}
      <header className={styles.header}>
        <div>
          <h1 className={styles.title}>
            {initial ? 'Editar Plano' : 'Novo Plano'}
          </h1>
          <p className={styles.subtitle}>
            {initial
              ? `Editando: ${initial.name}`
              : 'Selecione os tweaks e defina a sequência de execução'}
          </p>
        </div>
      </header>

      {/* Layout de duas colunas: seleção | sequência */}
      <div className={styles.editorLayout}>

        {/* Coluna esquerda: formulário + lista de tweaks */}
        <div className={styles.editorLeft}>
          <div className={styles.formGroup}>
            <label className={styles.formLabel}>Nome do plano *</label>
            <input
              className={styles.formInput}
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder='Ex.: Manutenção Semanal'
              maxLength={60}
            />
          </div>

          <div className={styles.formGroup}>
            <label className={styles.formLabel}>Descrição (opcional)</label>
            <textarea
              className={styles.formTextarea}
              value={description}
              onChange={e => setDescription(e.target.value)}
              placeholder='Descreva o objetivo deste plano…'
              rows={2}
              maxLength={200}
            />
          </div>

          {/* Tweaks por categoria */}
          <div className={styles.tweakSection}>
            <p className={styles.tweakSectionLabel}>Selecionar tweaks</p>
            {CATEGORIES.map(cat => {
              const tweaksInCat = TWEAKS.filter(t => t.categoryKey === cat.key);
              return (
                <div key={cat.key} className={styles.tweakCategory}>
                  <p className={styles.categoryLabel}>{cat.label}</p>
                  {tweaksInCat.map(tweak => (
                    <label
                      key={tweak.id}
                      className={`${styles.tweakItem} ${checkedIds.has(tweak.id) ? styles.tweakItemChecked : ''}`}
                    >
                      <input
                        type='checkbox'
                        className={styles.tweakCheckbox}
                        checked={checkedIds.has(tweak.id)}
                        onChange={e => toggleTweak(tweak.id, e.target.checked)}
                      />
                      <div className={styles.tweakMeta}>
                        <span className={styles.tweakName}>{tweak.name}</span>
                        <span className={styles.tweakDesc}>{tweak.description}</span>
                      </div>
                    </label>
                  ))}
                </div>
              );
            })}
          </div>
        </div>

        {/* Coluna direita: sequência de execução (sticky) */}
        <div className={styles.editorRight}>
          <p className={styles.orderLabel}>
            Sequência de execução
            {orderedIds.length > 0 && (
              <span className={styles.orderCount}>{orderedIds.length}</span>
            )}
          </p>

          {orderedIds.length === 0 ? (
            <div className={styles.orderEmpty}>
              <p>Marque os tweaks ao lado para definir a sequência</p>
            </div>
          ) : (
            <div className={styles.orderList}>
              {orderedIds.map((id, idx) => {
                const tweak = TWEAK_MAP[id];
                return (
                  <div
                    key={id}
                    className={`${styles.orderItem} ${dragOverId === id ? styles.orderItemDragOver : ''}`}
                    draggable
                    onDragStart={() => handleDragStart(id)}
                    onDragEnter={() => handleDragEnter(id)}
                    onDragOver={handleDragOver}
                    onDrop={() => handleDrop(id)}
                    onDragEnd={handleDragEnd}
                  >
                    <GripVertical size={14} strokeWidth={2} className={styles.dragHandle} />
                    <span className={styles.orderNum}>{idx + 1}</span>
                    <span className={styles.orderName}>{tweak?.name ?? id}</span>
                  </div>
                );
              })}
            </div>
          )}

          {/* Ações */}
          <div className={styles.editorActions}>
            <button
              className={styles.btnPrimary}
              onClick={handleSave}
              disabled={!isValid || saving}
            >
              {saving ? 'Salvando…' : initial ? 'Salvar alterações' : 'Criar plano'}
            </button>
            <button className={styles.btnSecondary} onClick={onCancel} disabled={saving}>
              Cancelar
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── ExecutionModal ─────────────────────────────────────────────────────────────

interface ExecutionModalProps {
  plan: Plan;
  state: ExecState;
  onClose: () => void;
}

function ExecutionModal({ plan, state, onClose }: ExecutionModalProps) {
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  // Itens ordenados pela sequência definida no plano
  const sortedItems = [...plan.items].sort((a, b) => a.order - b.order);
  const isDone = !state.running;

  function toggleExpand(id: string) {
    setExpandedItems(prev => {
      const s = new Set(prev);
      if (s.has(id)) s.delete(id); else s.add(id);
      return s;
    });
  }

  function buildSubtitle(): string {
    if (state.running) return `Executando… ${state.progress}% concluído`;
    if (state.fatalError) return 'Execução interrompida com erro';
    if (state.summary) {
      return `Concluído em ${formatDuration(state.summary.duration_seconds)}`;
    }
    return 'Concluído';
  }

  return (
    <div className={styles.modalOverlay}>
      <div className={styles.modal}>

        {/* Cabeçalho */}
        <div className={styles.modalHeader}>
          <div className={styles.modalTitleGroup}>
            <h2 className={styles.modalTitle}>{plan.name}</h2>
            <p className={styles.modalSubtitle}>{buildSubtitle()}</p>
          </div>
          <button
            className={styles.modalClose}
            onClick={onClose}
            disabled={state.running}
            title={state.running ? 'Aguarde a conclusão da execução' : 'Fechar'}
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* Barra de progresso */}
        <div className={styles.modalProgress}>
          <div
            className={`${styles.modalProgressFill} ${isDone && !state.fatalError ? styles.modalProgressDone : ''}`}
            style={{ width: `${state.progress}%` }}
          />
        </div>

        {/* Lista de itens */}
        <div className={styles.execList}>
          {sortedItems.map(item => {
            const itemState = state.items[item.tweak_id] ?? { status: 'pending' as ItemStatus };
            const tweak = TWEAK_MAP[item.tweak_id];
            const isExpanded = expandedItems.has(item.tweak_id);
            const hasDetails = !!(itemState.message || itemState.details || itemState.error);
            const statusClass = styles[`exec_${itemState.status}` as keyof typeof styles] ?? '';

            return (
              <div key={item.tweak_id} className={`${styles.execItem} ${statusClass}`}>
                <div className={styles.execItemRow}>
                  <StatusIcon status={itemState.status} hcStatus={itemState.hcStatus} />

                  <div className={styles.execItemInfo}>
                    <span className={styles.execItemName}>{tweak?.name ?? item.tweak_id}</span>
                    {itemState.message && !isExpanded && (
                      <span className={styles.execItemMsg}>{itemState.message}</span>
                    )}
                  </div>

                  {hasDetails && (
                    <button
                      className={styles.expandBtn}
                      onClick={() => toggleExpand(item.tweak_id)}
                      title={isExpanded ? 'Recolher' : 'Ver detalhes'}
                    >
                      {isExpanded
                        ? <ChevronUp size={13} strokeWidth={2} />
                        : <ChevronDown size={13} strokeWidth={2} />}
                    </button>
                  )}
                </div>

                {/* Painel de detalhes expandível */}
                {isExpanded && hasDetails && (
                  <div className={styles.execItemDetails}>
                    {itemState.error && (
                      <p className={styles.execDetailError}>{itemState.error}</p>
                    )}
                    {itemState.message && (
                      <p className={styles.execDetailMessage}>{itemState.message}</p>
                    )}
                    {itemState.details && (
                      <pre className={styles.execDetailLog}>{itemState.details}</pre>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Resumo final */}
        {isDone && state.summary && (
          <div className={styles.execSummary}>
            <div className={styles.summaryStats}>
              <div className={`${styles.summaryChip} ${styles.summarySuccess}`}>
                <CheckCircle2 size={13} strokeWidth={2.5} />
                {state.summary.completed_count} concluído{state.summary.completed_count !== 1 ? 's' : ''}
              </div>
              {state.summary.failed_count > 0 && (
                <div className={`${styles.summaryChip} ${styles.summaryFailed}`}>
                  <XCircle size={13} strokeWidth={2.5} />
                  {state.summary.failed_count} falhou
                </div>
              )}
              {state.summary.skipped_count > 0 && (
                <div className={`${styles.summaryChip} ${styles.summarySkipped}`}>
                  <MinusCircle size={13} strokeWidth={2.5} />
                  {state.summary.skipped_count} ignorado
                </div>
              )}
              <div className={styles.summaryDuration}>
                {formatDuration(state.summary.duration_seconds)}
              </div>
            </div>
            <button className={styles.btnPrimary} onClick={onClose}>
              Fechar
            </button>
          </div>
        )}

        {/* Erro fatal */}
        {isDone && state.fatalError && (
          <div className={styles.execSummary}>
            <p className={styles.execDetailError}>{state.fatalError}</p>
            <button className={styles.btnSecondary} onClick={onClose}>Fechar</button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── StatusIcon ─────────────────────────────────────────────────────────────────

interface StatusIconProps {
  status: ItemStatus;
  hcStatus?: 'success' | 'warning' | 'error';
}

function StatusIcon({ status, hcStatus }: StatusIconProps) {
  if (status === 'pending')  return <Clock     size={15} strokeWidth={2} className={styles.iconPending}   />;
  if (status === 'running')  return <Loader2   size={15} strokeWidth={2} className={styles.iconRunning}   />;
  if (status === 'failed')   return <XCircle   size={15} strokeWidth={2} className={styles.iconFailed}    />;
  if (status === 'skipped')  return <MinusCircle size={15} strokeWidth={2} className={styles.iconSkipped} />;

  // completed — cor depende do resultado do health check
  if (hcStatus === 'warning') return <CheckCircle2 size={15} strokeWidth={2} className={styles.iconWarning}   />;
  if (hcStatus === 'error')   return <CheckCircle2 size={15} strokeWidth={2} className={styles.iconFailed}    />;
  return <CheckCircle2 size={15} strokeWidth={2} className={styles.iconCompleted} />;
}
