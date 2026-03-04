// Página de Planos de Execução do FrameGuard.
//
// Permite criar rotinas de manutenção personalizadas combinando qualquer
// subconjunto dos tweaks disponíveis, definindo a ordem de execução e
// rodando tudo com um clique — com feedback em tempo real via eventos Tauri.

import { useState, useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import {
  Plus, Play, Pencil, Trash2, GripVertical,
  CheckCircle2, XCircle, Clock, MinusCircle,
  Loader2, X, ChevronDown, ChevronUp,
  ClipboardList, Shield, Copy, CalendarClock, Users,
  BookOpen, HeartPulse, Gamepad2, Wrench, Lock, ArrowRight, Info,
} from 'lucide-react';
import styles from './Plans.module.css';
import { useToast } from '../contexts/ToastContext';
import { usePlanExecution, useHardwareFilter } from '../hooks';
import { TWEAK_HARDWARE_MAP } from '../hooks/useHardwareFilter';
import type { Plan, PlanItem, ExecState, ItemStatus } from '../hooks';
import { showRestorePointToast } from '../utils/restorePoint';

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
  // Gamer
  {
    id: 'enable_hags',
    name: 'Hardware-Accelerated GPU Scheduling (HAGS)',
    description: 'Permite que a GPU gerencie sua própria memória, reduzindo latência de frames',
    categoryKey: 'gamer',
  },
  {
    id: 'enable_game_mode',
    name: 'Windows Game Mode',
    description: 'Prioriza CPU e GPU para o jogo em execução, reduzindo interferência em background',
    categoryKey: 'gamer',
  },
  {
    id: 'disable_vbs',
    name: 'Desabilitar VBS (Virtualização Baseada em Segurança)',
    description: 'Remove overhead de virtualização que pode reduzir FPS em 5–15%',
    categoryKey: 'gamer',
  },
  // GPU & Display
  {
    id: 'disable_game_dvr',
    name: 'Desabilitar Game DVR',
    description: 'Desabilita gravação em segundo plano, liberando GPU (encoder) e CPU',
    categoryKey: 'gpu_display',
  },
  {
    id: 'disable_xbox_overlay',
    name: 'Desabilitar Xbox Game Bar Overlay',
    description: 'Remove overlay da Xbox Game Bar (Win+G) que pode interferir em jogos',
    categoryKey: 'gpu_display',
  },
  {
    id: 'enable_msi_mode_gpu',
    name: 'Habilitar MSI Mode para GPU',
    description: 'Message Signaled Interrupts reduz latência de DPC — benefício maior em GPUs RTX 30 e anteriores',
    categoryKey: 'gpu_display',
  },
  {
    id: 'disable_mpo',
    name: 'Desabilitar Multiplane Overlay (MPO)',
    description: 'Remove stuttering/flickering em setups multi-monitor com refresh rates diferentes',
    categoryKey: 'gpu_display',
  },
  {
    id: 'disable_nvidia_telemetry',
    name: 'Desabilitar Telemetria NVIDIA',
    description: 'Remove coleta de telemetria do driver NVIDIA sem afetar funcionalidade',
    categoryKey: 'gpu_display',
  },
  // Gaming
  {
    id: 'enable_timer_resolution',
    name: 'Timer de Alta Resolução (1 ms)',
    description: 'Timer resolution de 1 ms melhora frame pacing e reduz input lag em monitores 144Hz+',
    categoryKey: 'gaming',
  },
  {
    id: 'disable_mouse_acceleration',
    name: 'Desabilitar Aceleração do Mouse',
    description: 'Remove curva não-linear do mouse — essencial para mira 1:1 em jogos FPS',
    categoryKey: 'gaming',
  },
  {
    id: 'disable_fullscreen_optimizations',
    name: 'Desabilitar Fullscreen Optimizations',
    description: 'Força fullscreen exclusivo em vez do modo otimizado — beneficia jogos DX9/DX11',
    categoryKey: 'gaming',
  },
  // Energia & CPU
  {
    id: 'enable_ultimate_performance',
    name: 'Plano Ultimate Performance',
    description: 'Mantém o processador em frequência máxima, eliminando latência de boost',
    categoryKey: 'energy_cpu',
  },
  {
    id: 'disable_power_throttling',
    name: 'Desabilitar Power Throttling',
    description: 'Impede redução de frequência de CPU para processos em background',
    categoryKey: 'energy_cpu',
  },
  // Armazenamento
  {
    id: 'disable_hibernation',
    name: 'Desabilitar Hibernação',
    description: 'Remove hiberfil.sys liberando 8-16 GB e desabilita Fast Startup',
    categoryKey: 'storage',
  },
  {
    id: 'disable_ntfs_last_access',
    name: 'Desabilitar Timestamp de Último Acesso NTFS',
    description: 'Reduz operações de escrita no disco ao não atualizar timestamps de acesso',
    categoryKey: 'storage',
  },
  // Rede
  {
    id: 'disable_nagle',
    name: 'Desabilitar Algoritmo de Nagle',
    description: 'Reduz latência TCP em 10-20 ms — beneficia jogos que usam TCP (MMOs, LoL)',
    categoryKey: 'network',
  },
  // Visual & Experiência
  {
    id: 'disable_sticky_keys',
    name: 'Desabilitar Teclas de Aderência',
    description: 'Remove atalho 5x Shift que interrompe sessões de jogo acidentalmente',
    categoryKey: 'visual',
  },
  {
    id: 'disable_bing_search',
    name: 'Desabilitar Busca Bing no Menu Iniciar',
    description: 'Buscas ficam apenas locais — mais rápidas e sem envio de dados',
    categoryKey: 'visual',
  },
  // Privacidade
  {
    id: 'disable_telemetry_registry',
    name: 'Desabilitar Telemetria do Windows',
    description: 'Bloqueia coleta e envio de dados de diagnóstico e uso para a Microsoft',
    categoryKey: 'privacy',
  },
  {
    id: 'disable_copilot',
    name: 'Desabilitar Copilot / Cortana',
    description: 'Remove integração do Copilot e Cortana do Windows',
    categoryKey: 'privacy',
  },
  {
    id: 'disable_content_delivery',
    name: 'Desabilitar Content Delivery Manager',
    description: 'Remove sugestões de apps e instalações automáticas de bloatware',
    categoryKey: 'privacy',
  },
  {
    id: 'disable_background_apps',
    name: 'Desabilitar Apps em Background',
    description: 'Impede apps da Microsoft Store de rodarem em segundo plano',
    categoryKey: 'privacy',
  },
  // Manutenção — Limpeza
  {
    id: 'flush_dns',
    name: 'Flush DNS',
    description: 'Limpa o cache de resolução DNS para corrigir problemas de conectividade',
    categoryKey: 'maintenance-clean',
  },
  {
    id: 'temp_cleanup',
    name: 'Limpeza de Arquivos Temporários',
    description: 'Remove arquivos de %TEMP%, Windows\\Temp e SoftwareDistribution\\Download',
    categoryKey: 'maintenance-clean',
  },
  // Manutenção — DISM
  {
    id: 'dism_checkhealth',
    name: 'DISM — CheckHealth',
    description: 'Verificação rápida de integridade do componente store (sem internet)',
    categoryKey: 'maintenance-dism',
  },
  {
    id: 'dism_scanhealth',
    name: 'DISM — ScanHealth',
    description: 'Varredura completa do componente store — mais lento e mais preciso',
    categoryKey: 'maintenance-dism',
  },
  {
    id: 'dism_restorehealth',
    name: 'DISM — RestoreHealth',
    description: 'Repara o componente store baixando arquivos de referência da Microsoft',
    categoryKey: 'maintenance-dism',
  },
  {
    id: 'dism_cleanup',
    name: 'DISM — StartComponentCleanup',
    description: 'Remove componentes obsoletos do Windows Component Store, liberando espaço',
    categoryKey: 'maintenance-dism',
  },
  // Manutenção — Verificação de Disco
  {
    id: 'sfc_scannow',
    name: 'SFC — System File Checker',
    description: 'Verifica e repara arquivos de sistema protegidos do Windows',
    categoryKey: 'maintenance-verify',
  },
  {
    id: 'chkdsk',
    name: 'Check Disk (C:)',
    description: 'Verifica e agenda reparo de erros lógicos e físicos no disco C:',
    categoryKey: 'maintenance-verify',
  },
  {
    id: 'ssd_trim',
    name: 'TRIM de SSDs',
    description: 'Executa otimização em todos os SSDs conectados para manter performance',
    categoryKey: 'maintenance-verify',
  },
];

const TWEAK_MAP: Record<string, TweakMeta> = Object.fromEntries(
  TWEAKS.map(t => [t.id, t]),
);

const CATEGORIES = [
  { key: 'optimization',       label: 'Otimizações'                        },
  { key: 'gamer',              label: 'Gamer'                              },
  { key: 'gpu_display',        label: 'GPU & Display'                      },
  { key: 'gaming',             label: 'Gaming'                             },
  { key: 'energy_cpu',         label: 'Energia & CPU'                      },
  { key: 'storage',            label: 'Armazenamento'                      },
  { key: 'network',            label: 'Rede'                               },
  { key: 'visual',             label: 'Visual & Experiência'               },
  { key: 'privacy',            label: 'Privacidade'                        },
  { key: 'maintenance-clean',  label: 'Manutenção — Limpeza'               },
  { key: 'maintenance-dism',   label: 'Manutenção — DISM Component Store'  },
  { key: 'maintenance-verify', label: 'Manutenção — Verificação de Disco'  },
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

  const [searchParams, setSearchParams] = useSearchParams();
  const { showToast } = useToast();
  const { executingPlan, execState, execute, closeModal, cleanup } = usePlanExecution();
  const { isCompatible, getVendorBadge } = useHardwareFilter();

  // Toast para status do ponto de restauração durante execução de plano
  useEffect(() => {
    if (execState?.restorePoint) {
      showRestorePointToast(execState.restorePoint, showToast);
    }
  }, [execState?.restorePoint, showToast]);

  // Guia colapsável
  const [guideOpen, setGuideOpen] = useState(() => {
    try { return localStorage.getItem('fg_planGuideOpen') !== 'false'; }
    catch { return true; }
  });

  function toggleGuide() {
    setGuideOpen(prev => {
      const next = !prev;
      try { localStorage.setItem('fg_planGuideOpen', String(next)); } catch {}
      return next;
    });
  }

  // Modal de visualização
  const [viewingPlan, setViewingPlan] = useState<Plan | null>(null);

  useEffect(() => {
    loadPlans();
    return () => { cleanup(); };
  }, [cleanup]);

  // Abre PlanViewModal quando navegado com ?viewPlan=<id> (ex: Dashboard → Planos)
  useEffect(() => {
    const viewPlanId = searchParams.get('viewPlan');
    if (!viewPlanId || plans.length === 0) return;

    const plan = plans.find(p => p.id === viewPlanId);
    if (plan) {
      setViewingPlan(plan);
      // Limpa o param para não reabrir ao voltar
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, plans, setSearchParams]);

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
    const planName = plans.find(p => p.id === planId)?.name;
    setConfirmDeleteId(null);
    try {
      await invoke('delete_plan', { planId });
      await loadPlans();
      showToast('success', 'Plano excluído', planName);
    } catch (err) {
      showToast('error', 'Erro ao excluir plano', String(err));
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
        showToast('success', 'Plano atualizado', name);
      } else {
        await invoke('create_plan', { name, description, items });
        showToast('success', 'Plano criado', name);
      }
      await loadPlans();
      setView('list');
    } catch (err) {
      showToast('error', 'Erro ao salvar plano', String(err));
      console.error('[Plans] Erro ao salvar plano:', err);
    }
  }

  async function handleDuplicatePlan(planId: string) {
    try {
      const duplicated = await invoke<Plan>('duplicate_plan', { planId });
      await loadPlans();
      showToast('success', 'Plano duplicado', duplicated.name);
      // Abre o editor com o plano duplicado para personalização
      setEditingPlan(duplicated);
      setView('edit');
    } catch (err) {
      showToast('error', 'Erro ao duplicar plano', String(err));
      console.error('[Plans] Erro ao duplicar plano:', err);
    }
  }

  async function handleExecutePlan(plan: Plan) {
    execute(plan, () => loadPlans());
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

      {/* Guia de planos */}
      <PlanGuide open={guideOpen} onToggle={toggleGuide} />

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
              onView={() => setViewingPlan(plan)}
              onEdit={() => handleEditPlan(plan)}
              onDelete={() => handleDeletePlan(plan.id)}
              onDeleteCancel={() => setConfirmDeleteId(null)}
              onRun={() => handleExecutePlan(plan)}
              onDuplicate={() => handleDuplicatePlan(plan.id)}
              isCompatible={isCompatible}
            />
          ))}
        </div>
      )}

      {/* Modal de visualização */}
      {viewingPlan && (
        <PlanViewModal
          plan={viewingPlan}
          onClose={() => setViewingPlan(null)}
          onRun={() => { setViewingPlan(null); handleExecutePlan(viewingPlan); }}
          onDuplicate={() => { setViewingPlan(null); handleDuplicatePlan(viewingPlan.id); }}
          isCompatible={isCompatible}
          getVendorBadge={getVendorBadge}
        />
      )}

      {/* Modal de execução */}
      {executingPlan && execState && (
        <ExecutionModal
          plan={executingPlan}
          state={execState}
          onClose={closeModal}
        />
      )}
    </div>
  );
}

// ── PlanGuide ──────────────────────────────────────────────────────────────────

const GUIDE_PLANS = [
  {
    Icon: HeartPulse,
    name: 'Saúde Completa',
    freq: '1x por mês ou quando o Windows parecer instável',
    desc: 'Verifica a integridade do sistema, repara arquivos corrompidos e limpa componentes obsoletos.',
    time: '5–15 min',
  },
  {
    Icon: Gamepad2,
    name: 'Otimização Gaming',
    freq: '1x após instalar ou reinstalar o Windows',
    desc: 'Configura GPU, CPU, rede e timers para máximo desempenho em jogos. Reinicie após executar.',
    time: '1–2 min',
  },
  {
    Icon: Wrench,
    name: 'Manutenção Básica',
    freq: 'Semanalmente ou quando notar lentidão',
    desc: 'Limpeza rápida de temporários e flush de DNS para manter o sistema leve.',
    time: '1–2 min',
  },
  {
    Icon: Lock,
    name: 'Privacidade e Debloat',
    freq: '1x após instalar o Windows; re-executar após grandes updates',
    desc: 'Remove telemetria, Copilot, Cortana, bloatware e apps em background indesejados.',
    time: '1–2 min',
  },
];

function PlanGuide({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  return (
    <div className={`${styles.guide} ${open ? styles.guideOpen : ''}`}>
      <button className={styles.guideToggle} onClick={onToggle}>
        <BookOpen size={15} strokeWidth={2} className={styles.guideIcon} />
        <span className={styles.guideTitle}>Como usar os Planos</span>
        <ChevronDown
          size={14}
          strokeWidth={2}
          className={`${styles.guideChevron} ${open ? styles.guideChevronOpen : ''}`}
        />
      </button>

      {open && (
        <div className={styles.guideContent}>
          <div className={styles.guidePlans}>
            {GUIDE_PLANS.map(({ Icon, name, freq, desc, time }) => (
              <div key={name} className={styles.guidePlan}>
                <div className={styles.guidePlanIcon}>
                  <Icon size={15} strokeWidth={2} />
                </div>
                <div className={styles.guidePlanBody}>
                  <div className={styles.guidePlanHeader}>
                    <span className={styles.guidePlanName}>{name}</span>
                    <span className={styles.guidePlanTime}>
                      <Clock size={10} strokeWidth={2} />
                      {time}
                    </span>
                  </div>
                  <p className={styles.guidePlanDesc}>{desc}</p>
                  <p className={styles.guidePlanFreq}>{freq}</p>
                </div>
              </div>
            ))}
          </div>

          <div className={styles.guideTip}>
            <Info size={13} strokeWidth={2} className={styles.guideTipIcon} />
            <p>
              <strong>Ordem recomendada:</strong>{' '}
              Manutenção Básica <ArrowRight size={10} strokeWidth={2.5} className={styles.guideTipArrow} />{' '}
              Saúde Completa <ArrowRight size={10} strokeWidth={2.5} className={styles.guideTipArrow} />{' '}
              Gaming / Privacidade
            </p>
          </div>
        </div>
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
  onView: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onDeleteCancel: () => void;
  onRun: () => void;
  onDuplicate: () => void;
  isCompatible: (tweakId: string) => boolean;
}

function PlanCard({ plan, confirmDeleteId, onView, onEdit, onDelete, onDeleteCancel, onRun, onDuplicate, isCompatible }: PlanCardProps) {
  const isConfirmingDelete = confirmDeleteId === plan.id;
  const enabledCount = plan.items.filter(i => i.enabled && isCompatible(i.tweak_id)).length;
  const isBuiltin = plan.builtin;

  return (
    <div className={styles.planCard}>
      <div className={styles.cardBody} onClick={onView} role="button" tabIndex={0}>
        <div className={styles.cardTitleRow}>
          <h3 className={styles.planName}>{plan.name}</h3>
          {isBuiltin && (
            <span className={styles.builtinBadgeCard}>
              <Shield size={11} strokeWidth={2.5} />
              Oficial
            </span>
          )}
        </div>
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
        {isBuiltin ? (
          <button className={styles.btnDuplicate} onClick={onDuplicate} title="Duplicar e personalizar">
            <Copy size={13} strokeWidth={2} />
            Duplicar
          </button>
        ) : (
          <>
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
          </>
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
                  {tweaksInCat.map(tweak => {
                    const vendorLabel = TWEAK_HARDWARE_MAP[tweak.id]
                      ? (TWEAK_HARDWARE_MAP[tweak.id].gpu_vendor ?? TWEAK_HARDWARE_MAP[tweak.id].cpu_vendor ?? '').toUpperCase()
                      : '';
                    return (
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
                          <span className={styles.tweakName}>
                            {tweak.name}
                            {vendorLabel && (
                              <span className={styles.vendorBadge}>{vendorLabel}</span>
                            )}
                          </span>
                          <span className={styles.tweakDesc}>{tweak.description}</span>
                        </div>
                      </label>
                    );
                  })}
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

// ── PlanViewModal ─────────────────────────────────────────────────────────────

interface PlanViewModalProps {
  plan: Plan;
  onClose: () => void;
  onRun: () => void;
  onDuplicate: () => void;
  isCompatible: (tweakId: string) => boolean;
  getVendorBadge: (tweakId: string) => string | null;
}

function PlanViewModal({ plan, onClose, onRun, onDuplicate, isCompatible, getVendorBadge }: PlanViewModalProps) {
  const isBuiltin = plan.builtin;
  const sortedItems = [...plan.items].sort((a, b) => a.order - b.order);
  const enabledCount = plan.items.filter(i => i.enabled && isCompatible(i.tweak_id)).length;

  return (
    <div className={styles.modalOverlay} onClick={onClose}>
      <div className={styles.modal} onClick={e => e.stopPropagation()}>

        {/* Cabeçalho */}
        <div className={styles.modalHeader}>
          <div className={styles.modalTitleGroup}>
            <div className={styles.viewTitleRow}>
              <h2 className={styles.modalTitle}>{plan.name}</h2>
              {isBuiltin && (
                <span className={styles.builtinBadge}>
                  <Shield size={11} strokeWidth={2.5} />
                  Plano oficial
                </span>
              )}
            </div>
            {plan.description && (
              <p className={styles.modalSubtitle}>{plan.description}</p>
            )}
          </div>
          <button className={styles.modalClose} onClick={onClose} title="Fechar">
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* Badges de metadata */}
        {(plan.recommended_frequency || plan.target_audience) && (
          <div className={styles.viewMetaBadges}>
            {plan.recommended_frequency && (
              <span className={styles.viewMetaBadge}>
                <CalendarClock size={12} />
                {plan.recommended_frequency}
              </span>
            )}
            {plan.target_audience && (
              <span className={styles.viewMetaBadge}>
                <Users size={12} />
                {plan.target_audience}
              </span>
            )}
          </div>
        )}

        {/* Descrição expandida */}
        {plan.long_description && (
          <div className={styles.viewLongDesc}>
            {plan.long_description}
          </div>
        )}

        {/* Lista de tarefas */}
        <div className={styles.viewList}>
          {sortedItems.map((item, idx) => {
            const tweak = TWEAK_MAP[item.tweak_id];
            const compatible = isCompatible(item.tweak_id);
            const vendorLabel = getVendorBadge(item.tweak_id);
            return (
              <div
                key={item.tweak_id}
                className={`${styles.viewItem} ${!compatible ? styles.viewItemIncompatible : ''}`}
              >
                <span className={styles.viewNum}>{idx + 1}</span>
                <div className={styles.viewItemInfo}>
                  <span className={styles.viewItemName}>
                    {tweak?.name ?? item.tweak_id}
                    {vendorLabel && (
                      <span className={styles.vendorBadge}>{vendorLabel}</span>
                    )}
                  </span>
                  {!compatible && (
                    <span className={styles.incompatibleLabel}>
                      Hardware incompatível — será ignorado
                    </span>
                  )}
                  {compatible && tweak?.description && (
                    <span className={styles.viewItemDesc}>{tweak.description}</span>
                  )}
                </div>
              </div>
            );
          })}
        </div>

        {/* Rodapé */}
        <div className={styles.viewFooter}>
          <span className={styles.viewMeta}>
            {plan.last_executed
              ? `Última execução: ${formatDate(plan.last_executed)}`
              : 'Nunca executado'}
          </span>
          <div className={styles.viewActions}>
            {isBuiltin && (
              <button className={styles.btnDuplicate} onClick={onDuplicate} title="Duplicar e personalizar">
                <Copy size={13} strokeWidth={2} />
                Duplicar e personalizar
              </button>
            )}
            <button
              className={styles.btnRun}
              onClick={onRun}
              disabled={enabledCount === 0}
            >
              <Play size={13} strokeWidth={2.5} fill="currentColor" />
              Executar
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
