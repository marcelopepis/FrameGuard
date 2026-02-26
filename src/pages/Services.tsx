// Página de Serviços e Tarefas Agendadas do FrameGuard.
//
// UI diferente das outras páginas — lista gerenciável com checkboxes em tabela,
// categorias colapsáveis, e operações em batch (desabilitar/restaurar).

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2, XCircle, RefreshCw, AlertTriangle, ChevronDown } from 'lucide-react';
import styles from './Services.module.css';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';

// ── Tipos (espelham os structs Rust) ─────────────────────────────────────────

interface ServiceItem {
  id: string;
  display_name: string;
  description: string;
  category: string;
  status: string;
  startup_type: string;
  is_conditional: boolean;
  conditional_note: string | null;
  has_backup: boolean;
}

interface TaskItem {
  id: string;
  display_name: string;
  description: string;
  category: string;
  state: string;
  task_path: string;
  task_name: string;
  has_backup: boolean;
}

interface BatchResult {
  succeeded: string[];
  failed: { id: string; error: string }[];
}

// ── Definição das categorias ─────────────────────────────────────────────────

const SERVICE_CATEGORIES = [
  {
    id: 'telemetry',
    title: 'Telemetria e Diagnósticos',
    hint: 'Coleta de dados para a Microsoft',
  },
  {
    id: 'hardware',
    title: 'Hardware não utilizado',
    hint: 'Contém serviços condicionais (!)',
  },
  {
    id: 'remote',
    title: 'Acesso remoto',
    hint: 'Administração remota',
  },
  {
    id: 'enterprise',
    title: 'Enterprise / Não utilizado',
    hint: 'Desnecessários para uso pessoal',
  },
];

const TASK_CATEGORIES = [
  {
    id: 'telemetry',
    title: 'Telemetria',
    hint: 'Maior impacto',
  },
  {
    id: 'ceip',
    title: 'Customer Experience Improvement Program',
    hint: 'Programa CEIP',
  },
  {
    id: 'diagnostics',
    title: 'Diagnósticos',
    hint: 'Coleta de dados e feedback',
  },
];

// ── Helpers de exibição ──────────────────────────────────────────────────────

function statusDotClass(status: string): string {
  switch (status) {
    case 'Running': return styles.statusRunning;
    case 'Stopped': return styles.statusStopped;
    case 'Disabled': return styles.statusDisabled;
    default: return styles.statusNotFound;
  }
}

function statusLabelClass(status: string): string {
  switch (status) {
    case 'Running': return styles.statusLabelRunning;
    case 'Stopped': return styles.statusLabelStopped;
    case 'Disabled': return styles.statusLabelDisabled;
    default: return styles.statusLabelNotFound;
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case 'Running': return 'Rodando';
    case 'Stopped': return 'Parado';
    case 'Disabled': return 'Desabilitado';
    case 'NotFound': return 'N/A';
    default: return status;
  }
}

function startupLabel(type: string): string {
  switch (type) {
    case 'Automatic': return 'Automático';
    case 'Manual': return 'Manual';
    case 'Disabled': return 'Desabilitado';
    case 'Boot': return 'Boot';
    case 'System': return 'Sistema';
    case 'NotFound': return 'N/A';
    default: return type;
  }
}

function taskStateClass(state: string): string {
  switch (state) {
    case 'Ready': return styles.stateReady;
    case 'Disabled': return styles.stateDisabled;
    case 'Running': return styles.stateRunning;
    default: return styles.stateNotFound;
  }
}

function taskStateLabel(state: string): string {
  switch (state) {
    case 'Ready': return 'Ativa';
    case 'Disabled': return 'Desabilitada';
    case 'Running': return 'Executando';
    case 'NotFound': return 'N/A';
    default: return state;
  }
}

function taskStateDotClass(state: string): string {
  switch (state) {
    case 'Ready': return styles.statusRunning;
    case 'Disabled': return styles.statusDisabled;
    case 'Running': return styles.statusRunning;
    default: return styles.statusNotFound;
  }
}

// ── Componente principal ─────────────────────────────────────────────────────

export default function Services() {
  // Estado da página
  const [services, setServices] = useState<ServiceItem[]>([]);
  const [tasks, setTasks] = useState<TaskItem[]>([]);
  const [pageLoading, setPageLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);

  // Seleção (checkboxes)
  const [selectedServices, setSelectedServices] = useState<Set<string>>(new Set());
  const [selectedTasks, setSelectedTasks] = useState<Set<string>>(new Set());

  // Categorias colapsadas
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  // Loading de operações batch
  const [svcLoading, setSvcLoading] = useState(false);
  const [taskLoading, setTaskLoading] = useState(false);

  const { isRunning } = useGlobalRunning();
  const { showToast } = useToast();

  const anyLoading = svcLoading || taskLoading;
  const globalDisabled = isRunning || anyLoading;

  // ── Carregamento ──

  const loadData = useCallback(async () => {
    setPageLoading(true);
    setPageError(null);
    try {
      const [svc, tsk] = await Promise.all([
        invoke<ServiceItem[]>('get_services_status'),
        invoke<TaskItem[]>('get_tasks_status'),
      ]);
      setServices(svc);
      setTasks(tsk);
      setSelectedServices(new Set());
      setSelectedTasks(new Set());
    } catch (e) {
      setPageError(`Erro ao carregar dados: ${e}`);
    } finally {
      setPageLoading(false);
    }
  }, []);

  useEffect(() => { loadData(); }, [loadData]);

  // ── Toggle de categoria ──

  function toggleCategory(catId: string) {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(catId)) next.delete(catId);
      else next.add(catId);
      return next;
    });
  }

  // ── Toggle de checkbox (serviços) ──

  function toggleService(id: string) {
    setSelectedServices(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleTask(id: string) {
    setSelectedTasks(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  // ── Ações batch: Serviços ──

  async function handleDisableServices() {
    const ids = Array.from(selectedServices);
    if (ids.length === 0) return;

    setSvcLoading(true);
    try {
      const result = await invoke<BatchResult>('disable_services', { ids });
      if (result.succeeded.length > 0) {
        showToast('success', 'Serviços desabilitados', `${result.succeeded.length} serviço(s) desabilitado(s) com sucesso.`);
        invoke('log_tweak_activity', {
          name: `Desabilitar ${result.succeeded.length} serviço(s)`,
          applied: true, success: true,
        }).catch(() => {});
      }
      for (const f of result.failed) {
        showToast('error', `Falha: ${f.id}`, f.error);
      }
      if (result.failed.length > 0 && result.succeeded.length === 0) {
        invoke('log_tweak_activity', {
          name: `Desabilitar serviços (${result.failed.length} falha(s))`,
          applied: true, success: false,
        }).catch(() => {});
      }
      await loadData();
    } catch (e) {
      showToast('error', 'Erro ao desabilitar serviços', String(e));
      invoke('log_tweak_activity', { name: 'Desabilitar serviços', applied: true, success: false }).catch(() => {});
    } finally {
      setSvcLoading(false);
    }
  }

  async function handleRestoreServices() {
    const ids = services.filter(s => s.has_backup).map(s => s.id);
    if (ids.length === 0) {
      showToast('warning', 'Nenhum backup', 'Nenhum serviço foi modificado pelo FrameGuard.');
      return;
    }

    setSvcLoading(true);
    try {
      const result = await invoke<BatchResult>('restore_services', { ids });
      if (result.succeeded.length > 0) {
        showToast('success', 'Serviços restaurados', `${result.succeeded.length} serviço(s) restaurado(s) ao padrão original.`);
        invoke('log_tweak_activity', {
          name: `Restaurar ${result.succeeded.length} serviço(s)`,
          applied: false, success: true,
        }).catch(() => {});
      }
      for (const f of result.failed) {
        showToast('error', `Falha: ${f.id}`, f.error);
      }
      await loadData();
    } catch (e) {
      showToast('error', 'Erro ao restaurar serviços', String(e));
      invoke('log_tweak_activity', { name: 'Restaurar serviços', applied: false, success: false }).catch(() => {});
    } finally {
      setSvcLoading(false);
    }
  }

  // ── Ações batch: Tarefas ──

  async function handleDisableTasks() {
    const ids = Array.from(selectedTasks);
    if (ids.length === 0) return;

    setTaskLoading(true);
    try {
      const result = await invoke<BatchResult>('disable_tasks', { ids });
      if (result.succeeded.length > 0) {
        showToast('success', 'Tarefas desabilitadas', `${result.succeeded.length} tarefa(s) desabilitada(s) com sucesso.`);
        invoke('log_tweak_activity', {
          name: `Desabilitar ${result.succeeded.length} tarefa(s)`,
          applied: true, success: true,
        }).catch(() => {});
      }
      for (const f of result.failed) {
        showToast('error', `Falha: ${f.id}`, f.error);
      }
      if (result.failed.length > 0 && result.succeeded.length === 0) {
        invoke('log_tweak_activity', {
          name: `Desabilitar tarefas (${result.failed.length} falha(s))`,
          applied: true, success: false,
        }).catch(() => {});
      }
      await loadData();
    } catch (e) {
      showToast('error', 'Erro ao desabilitar tarefas', String(e));
      invoke('log_tweak_activity', { name: 'Desabilitar tarefas', applied: true, success: false }).catch(() => {});
    } finally {
      setTaskLoading(false);
    }
  }

  async function handleRestoreTasks() {
    const ids = tasks.filter(t => t.has_backup).map(t => t.id);
    if (ids.length === 0) {
      showToast('warning', 'Nenhum backup', 'Nenhuma tarefa foi modificada pelo FrameGuard.');
      return;
    }

    setTaskLoading(true);
    try {
      const result = await invoke<BatchResult>('restore_tasks', { ids });
      if (result.succeeded.length > 0) {
        showToast('success', 'Tarefas restauradas', `${result.succeeded.length} tarefa(s) restaurada(s).`);
        invoke('log_tweak_activity', {
          name: `Restaurar ${result.succeeded.length} tarefa(s)`,
          applied: false, success: true,
        }).catch(() => {});
      }
      for (const f of result.failed) {
        showToast('error', `Falha: ${f.id}`, f.error);
      }
      await loadData();
    } catch (e) {
      showToast('error', 'Erro ao restaurar tarefas', String(e));
      invoke('log_tweak_activity', { name: 'Restaurar tarefas', applied: false, success: false }).catch(() => {});
    } finally {
      setTaskLoading(false);
    }
  }

  // ── Render: Loading ──

  if (pageLoading) {
    return (
      <div className={styles.page}>
        <div className={styles.pageLoading}>
          <Loader2 size={20} className={styles.spinner} />
          <span>Consultando serviços e tarefas agendadas...</span>
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
          <button className={styles.btnRetry} onClick={loadData}>
            Tentar novamente
          </button>
        </div>
      </div>
    );
  }

  const svcWithBackup = services.filter(s => s.has_backup).length;
  const taskWithBackup = tasks.filter(t => t.has_backup).length;

  return (
    <div className={styles.page}>

      {/* ── Header ── */}
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Serviços e Tarefas</h1>
          <p className={styles.subtitle}>
            Gerencie serviços do Windows e tarefas agendadas para reduzir uso de recursos
          </p>
        </div>
        <button
          className={styles.btnRefresh}
          onClick={loadData}
          disabled={anyLoading}
          title="Recarregar status"
        >
          <RefreshCw size={14} />
        </button>
      </div>

      {/* ── Banner de aviso ── */}
      <div className={styles.warningBanner}>
        <AlertTriangle size={18} />
        <span>
          Desabilitar serviços incorretos pode causar instabilidade.
          Os serviços listados aqui foram curados para serem seguros em PCs de gaming.
          Serviços marcados com <strong>(!)</strong> dependem do seu hardware — leia a descrição antes.
        </span>
      </div>

      <div className={styles.sections}>

        {/* ── Seção: Serviços do Windows ── */}
        <div className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <span className={styles.sectionTitle}>Serviços do Windows</span>
            <span className={styles.sectionSubtitle}>
              {services.length} serviços curados
            </span>
          </div>

          {SERVICE_CATEGORIES.map(cat => {
            const catServices = services.filter(s => s.category === cat.id);
            if (catServices.length === 0) return null;
            const catKey = `svc_${cat.id}`;
            const isCollapsed = collapsed.has(catKey);

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
                  <span className={styles.categoryHint}>{cat.hint}</span>
                </div>

                {!isCollapsed && (
                  <table className={styles.table}>
                    <thead>
                      <tr className={styles.tableHead}>
                        <th></th>
                        <th>Nome</th>
                        <th>Descrição</th>
                        <th className={styles.colStatus}>Status</th>
                        <th className={styles.colStartup}>Tipo</th>
                      </tr>
                    </thead>
                    <tbody>
                      {catServices.map(svc => {
                        const isNotFound = svc.status === 'NotFound';
                        return (
                          <tr
                            key={svc.id}
                            className={`${styles.tableRow} ${isNotFound ? styles.tableRowNotFound : ''}`}
                          >
                            <td>
                              <input
                                type="checkbox"
                                className={styles.checkbox}
                                checked={selectedServices.has(svc.id)}
                                onChange={() => toggleService(svc.id)}
                                disabled={globalDisabled || isNotFound}
                              />
                            </td>
                            <td>
                              <div className={styles.nameCell}>
                                <span className={styles.displayName}>
                                  {svc.display_name}
                                  {svc.is_conditional && (
                                    <span className={styles.conditionalBadge} title={svc.conditional_note || ''}>
                                      !
                                      {svc.conditional_note && (
                                        <span className={styles.conditionalTip}>
                                          {svc.conditional_note}
                                        </span>
                                      )}
                                    </span>
                                  )}
                                </span>
                                <span className={styles.techName}>{svc.id}</span>
                              </div>
                            </td>
                            <td className={styles.descCell}>{svc.description}</td>
                            <td>
                              <div className={styles.statusCell}>
                                <span className={`${styles.statusDot} ${statusDotClass(svc.status)}`} />
                                <span className={statusLabelClass(svc.status)}>
                                  {statusLabel(svc.status)}
                                </span>
                              </div>
                            </td>
                            <td className={styles.startupCell}>
                              {startupLabel(svc.startup_type)}
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

          {/* Barra de ações dos serviços */}
          <div className={styles.actionBar}>
            <span className={styles.selectionCount}>
              {selectedServices.size > 0
                ? `${selectedServices.size} selecionado(s)`
                : svcWithBackup > 0
                  ? `${svcWithBackup} modificado(s) pelo FrameGuard`
                  : 'Nenhum selecionado'
              }
            </span>
            <button
              className={styles.btnPrimary}
              onClick={handleDisableServices}
              disabled={globalDisabled || selectedServices.size === 0}
            >
              {svcLoading && <Loader2 size={14} className={styles.spinner} />}
              Aplicar Seleção
            </button>
            <button
              className={styles.btnSecondary}
              onClick={handleRestoreServices}
              disabled={globalDisabled || svcWithBackup === 0}
            >
              Restaurar Todos
            </button>
          </div>
        </div>

        {/* ── Seção: Tarefas Agendadas ── */}
        <div className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <span className={styles.sectionTitle}>Tarefas Agendadas</span>
            <span className={styles.sectionSubtitle}>
              {tasks.length} tarefas curadas
            </span>
          </div>

          {TASK_CATEGORIES.map(cat => {
            const catTasks = tasks.filter(t => t.category === cat.id);
            if (catTasks.length === 0) return null;
            const catKey = `task_${cat.id}`;
            const isCollapsed = collapsed.has(catKey);

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
                  <span className={styles.categoryHint}>{cat.hint}</span>
                </div>

                {!isCollapsed && (
                  <table className={styles.table}>
                    <thead>
                      <tr className={styles.tableHead}>
                        <th></th>
                        <th>Nome</th>
                        <th>Descrição</th>
                        <th className={styles.colStatus}>Estado</th>
                      </tr>
                    </thead>
                    <tbody>
                      {catTasks.map(task => {
                        const isNotFound = task.state === 'NotFound';
                        return (
                          <tr
                            key={task.id}
                            className={`${styles.tableRow} ${isNotFound ? styles.tableRowNotFound : ''}`}
                          >
                            <td>
                              <input
                                type="checkbox"
                                className={styles.checkbox}
                                checked={selectedTasks.has(task.id)}
                                onChange={() => toggleTask(task.id)}
                                disabled={globalDisabled || isNotFound}
                              />
                            </td>
                            <td>
                              <div className={styles.nameCell}>
                                <span className={styles.displayName}>
                                  {task.display_name}
                                </span>
                                <span className={styles.techName}>{task.task_name}</span>
                              </div>
                            </td>
                            <td className={styles.descCell}>{task.description}</td>
                            <td>
                              <div className={styles.statusCell}>
                                <span className={`${styles.statusDot} ${taskStateDotClass(task.state)}`} />
                                <span className={taskStateClass(task.state)}>
                                  {taskStateLabel(task.state)}
                                </span>
                              </div>
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

          {/* Barra de ações das tarefas */}
          <div className={styles.actionBar}>
            <span className={styles.selectionCount}>
              {selectedTasks.size > 0
                ? `${selectedTasks.size} selecionada(s)`
                : taskWithBackup > 0
                  ? `${taskWithBackup} modificada(s) pelo FrameGuard`
                  : 'Nenhuma selecionada'
              }
            </span>
            <button
              className={styles.btnPrimary}
              onClick={handleDisableTasks}
              disabled={globalDisabled || selectedTasks.size === 0}
            >
              {taskLoading && <Loader2 size={14} className={styles.spinner} />}
              Desabilitar Selecionadas
            </button>
            <button
              className={styles.btnSecondary}
              onClick={handleRestoreTasks}
              disabled={globalDisabled || taskWithBackup === 0}
            >
              Restaurar Todas
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
