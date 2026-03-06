// Página de Limpeza — escaneamento e remoção de arquivos temporários e caches.
// Substitui a necessidade de CCleaner com scan por categorias e limpeza granular.

import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Search,
  Trash2,
  Loader2,
  CheckCircle,
  AlertTriangle,
  ChevronDown,
  HardDrive,
  Monitor,
  Globe,
  Package,
  Settings2,
  RefreshCw,
  ShieldAlert,
} from 'lucide-react';
import { useGlobalRunning } from '../contexts/RunningContext';
import { useToast } from '../contexts/ToastContext';
import { formatBytes, formatDuration } from '../utils/formatters';
import type {
  CleanupScanResult,
  CleanupCategory,
  CleanupProgressEvent,
  CleanupResult,
  BrowserCleanOptions,
} from '../types/cleanup';
import styles from './Cleanup.module.css';

type Phase = 'initial' | 'scanning' | 'results' | 'cleaning' | 'report';

// Ícone por categoria
function CategoryIcon({ categoryId }: { categoryId: string }) {
  const size = 18;
  const props = { size, className: styles.categoryIcon, strokeWidth: 1.8 };
  switch (categoryId) {
    case 'sistema_windows':
      return <Monitor {...props} />;
    case 'gpu_shader_cache':
      return <HardDrive {...props} />;
    case 'browsers':
      return <Globe {...props} />;
    case 'aplicativos':
      return <Package {...props} />;
    case 'avancado':
      return <Settings2 {...props} />;
    default:
      return <HardDrive {...props} />;
  }
}

// Badge de risco
function RiskBadge({ risk }: { risk: string }) {
  const label = risk === 'safe' ? 'Seguro' : risk === 'moderate' ? 'Atenção' : 'Cuidado';
  const cls =
    risk === 'safe'
      ? styles.riskSafe
      : risk === 'moderate'
        ? styles.riskModerate
        : styles.riskCaution;
  return <span className={`${styles.riskBadge} ${cls}`}>{label}</span>;
}

export default function Cleanup() {
  const [phase, setPhase] = useState<Phase>('initial');
  const [scanResult, setScanResult] = useState<CleanupScanResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<CleanupProgressEvent | null>(null);
  const [cleanupResult, setCleanupResult] = useState<CleanupResult | null>(null);
  const [scanProgress, setScanProgress] = useState<{
    categoryName: string;
    categoryIndex: number;
    totalCategories: number;
  } | null>(null);

  // Opções granulares de limpeza de browsers — padrão seguro
  const [browserOptions, setBrowserOptions] = useState<BrowserCleanOptions>({
    cache: true,
    cookies: false,
    history: false,
    sessions: true,
  });

  const { isRunning, startTask, endTask } = useGlobalRunning();
  const { showToast } = useToast();

  // ── Scan ──────────────────────────────────────────────────
  const handleScan = useCallback(async () => {
    setPhase('scanning');
    setScanProgress(null);
    startTask('cleanup_scan', '/cleanup');

    const unlisten = await listen<{
      category_name: string;
      category_index: number;
      total_categories: number;
    }>('scan_progress', (event) => {
      setScanProgress({
        categoryName: event.payload.category_name,
        categoryIndex: event.payload.category_index,
        totalCategories: event.payload.total_categories,
      });
    });

    try {
      const result = await invoke<CleanupScanResult>('scan_cleanup');
      setScanResult(result);

      // Inicializar seleção baseado em default_selected dos items
      const defaults = new Set<string>();
      for (const cat of result.categories) {
        for (const item of cat.items) {
          if (item.default_selected) {
            defaults.add(item.id);
          }
        }
      }
      setSelected(defaults);
      setExpanded(new Set());
      setPhase('results');
    } catch (e) {
      showToast('error', 'Erro ao escanear', String(e));
      setPhase('initial');
    } finally {
      endTask('cleanup_scan');
      unlisten();
      setScanProgress(null);
    }
  }, [showToast, startTask, endTask]);

  // ── Cleanup ───────────────────────────────────────────────
  const handleClean = useCallback(async () => {
    if (selected.size === 0) return;
    setPhase('cleaning');
    setProgress(null);
    startTask('cleanup', '/cleanup');

    const unlisten = await listen<CleanupProgressEvent>('cleanup_progress', (event) => {
      setProgress(event.payload);
    });

    try {
      const result = await invoke<CleanupResult>('execute_cleanup', {
        itemIds: Array.from(selected),
        browserOptions,
      });
      setCleanupResult(result);
      setPhase('report');
      showToast(
        'success',
        'Limpeza concluída',
        `${formatBytes(result.total_freed_bytes)} liberados`,
      );
      invoke('log_tweak_activity', {
        name: 'Limpeza de Sistema',
        applied: true,
        success: true,
      }).catch(() => {});
    } catch (e) {
      showToast('error', 'Erro durante limpeza', String(e));
      setPhase('results');
      invoke('log_tweak_activity', {
        name: 'Limpeza de Sistema',
        applied: true,
        success: false,
      }).catch(() => {});
    } finally {
      endTask('cleanup');
      unlisten();
    }
  }, [selected, browserOptions, startTask, endTask, showToast]);

  // ── Seleção ───────────────────────────────────────────────
  const toggleItem = useCallback((itemId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(itemId)) next.delete(itemId);
      else next.add(itemId);
      return next;
    });
  }, []);

  const toggleCategory = useCallback((cat: CleanupCategory) => {
    setSelected((prev) => {
      const next = new Set(prev);
      const allSelected = cat.items.every((i) => next.has(i.id));
      for (const item of cat.items) {
        if (allSelected) next.delete(item.id);
        else next.add(item.id);
      }
      return next;
    });
  }, []);

  const toggleExpand = useCallback((catId: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(catId)) next.delete(catId);
      else next.add(catId);
      return next;
    });
  }, []);

  // Calcular total selecionado
  const selectedBytes =
    scanResult?.categories.reduce((sum, cat) => {
      return (
        sum +
        cat.items.reduce((s, item) => {
          return s + (selected.has(item.id) ? item.size_bytes : 0);
        }, 0)
      );
    }, 0) ?? 0;

  // ── Render ────────────────────────────────────────────────
  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1 className={styles.title}>Limpeza</h1>
        <p className={styles.subtitle}>Libere espaço removendo arquivos temporários e caches</p>
      </div>

      {phase === 'initial' && (
        <div className={styles.initialState}>
          <div className={styles.initialIcon}>
            <HardDrive size={48} strokeWidth={1.2} />
          </div>
          <p className={styles.initialText}>
            Escaneie o sistema para identificar arquivos temporários, caches de GPU, browsers e
            aplicativos que podem ser removidos com segurança.
          </p>
          <button className={styles.scanBtn} onClick={handleScan} disabled={isRunning}>
            <Search size={16} />
            Escanear sistema
          </button>
        </div>
      )}

      {phase === 'scanning' && (
        <div className={styles.scanningState}>
          <Loader2 size={28} className={styles.spinner} />
          {scanProgress ? (
            <>
              <span>Escaneando: {scanProgress.categoryName}</span>
              <span className={styles.scanCounter}>
                {scanProgress.categoryIndex} de {scanProgress.totalCategories}
              </span>
              <div className={styles.progressBar} style={{ width: 200, marginTop: 8 }}>
                <div
                  className={styles.progressFill}
                  style={{
                    width: `${(scanProgress.categoryIndex / scanProgress.totalCategories) * 100}%`,
                  }}
                />
              </div>
            </>
          ) : (
            <span>Iniciando escaneamento...</span>
          )}
        </div>
      )}

      {phase === 'results' && scanResult && (
        <ResultsView
          scanResult={scanResult}
          selected={selected}
          expanded={expanded}
          selectedBytes={selectedBytes}
          isRunning={isRunning}
          browserOptions={browserOptions}
          onBrowserOptionsChange={setBrowserOptions}
          onToggleItem={toggleItem}
          onToggleCategory={toggleCategory}
          onToggleExpand={toggleExpand}
          onClean={handleClean}
          onNewScan={handleScan}
        />
      )}

      {phase === 'cleaning' && (
        <div className={styles.cleaningState}>
          <div className={styles.progressSection}>
            <div className={styles.progressBar}>
              <div
                className={styles.progressFill}
                style={{ width: `${progress?.progress_percent ?? 0}%` }}
              />
            </div>
            <span className={styles.progressPercent}>
              {Math.round(progress?.progress_percent ?? 0)}%
            </span>
          </div>
          <div className={styles.cleaningInfo}>
            <Loader2 size={14} className={styles.spinner} />
            <span>{progress?.message ?? 'Iniciando limpeza...'}</span>
          </div>
          <span className={styles.cleaningFreed}>
            {formatBytes(progress?.freed_bytes_so_far ?? 0)} liberados
          </span>
        </div>
      )}

      {phase === 'report' && cleanupResult && scanResult && (
        <ReportView
          result={cleanupResult}
          onNewScan={() => {
            setPhase('initial');
          }}
        />
      )}
    </div>
  );
}

// ─── Subcomponente: Resultados ──────────────────────────────────────────────

interface ResultsViewProps {
  scanResult: CleanupScanResult;
  selected: Set<string>;
  expanded: Set<string>;
  selectedBytes: number;
  isRunning: boolean;
  browserOptions: BrowserCleanOptions;
  onBrowserOptionsChange: (opts: BrowserCleanOptions) => void;
  onToggleItem: (id: string) => void;
  onToggleCategory: (cat: CleanupCategory) => void;
  onToggleExpand: (catId: string) => void;
  onClean: () => void;
  onNewScan: () => void;
}

function ResultsView({
  scanResult,
  selected,
  expanded,
  selectedBytes,
  isRunning,
  browserOptions,
  onBrowserOptionsChange,
  onToggleItem,
  onToggleCategory,
  onToggleExpand,
  onClean,
  onNewScan,
}: ResultsViewProps) {
  return (
    <>
      <div className={styles.resultsHeader}>
        <div className={styles.totalBanner}>
          <span className={styles.totalSize}>{formatBytes(scanResult.total_size_bytes)}</span>
          <span className={styles.totalLabel}>de arquivos removíveis encontrados</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className={styles.scanDuration}>
            Scan em {formatDuration(scanResult.scan_duration_seconds)}
          </span>
          <button className={styles.btnNewScan} onClick={onNewScan} disabled={isRunning}>
            <RefreshCw size={13} />
            Novo scan
          </button>
        </div>
      </div>

      <div className={styles.categoryList}>
        {scanResult.categories.map((cat) => (
          <CategoryCard
            key={cat.id}
            category={cat}
            selected={selected}
            isExpanded={expanded.has(cat.id)}
            browserOptions={cat.id === 'browsers' ? browserOptions : undefined}
            onBrowserOptionsChange={cat.id === 'browsers' ? onBrowserOptionsChange : undefined}
            onToggleItem={onToggleItem}
            onToggleCategory={() => onToggleCategory(cat)}
            onToggleExpand={() => onToggleExpand(cat.id)}
          />
        ))}
      </div>

      <div className={styles.resultsFooter}>
        <span className={styles.totalSelected}>
          Total selecionado:{' '}
          <span className={styles.totalSelectedValue}>{formatBytes(selectedBytes)}</span>
        </span>
        <button
          className={styles.cleanBtn}
          onClick={onClean}
          disabled={isRunning || selected.size === 0}
        >
          <Trash2 size={14} />
          Limpar selecionados ({formatBytes(selectedBytes)})
        </button>
      </div>
    </>
  );
}

// ─── Subcomponente: Card de categoria ───────────────────────────────────────

interface CategoryCardProps {
  category: CleanupCategory;
  selected: Set<string>;
  isExpanded: boolean;
  browserOptions?: BrowserCleanOptions;
  onBrowserOptionsChange?: (opts: BrowserCleanOptions) => void;
  onToggleItem: (id: string) => void;
  onToggleCategory: () => void;
  onToggleExpand: () => void;
}

function CategoryCard({
  category,
  selected,
  isExpanded,
  browserOptions,
  onBrowserOptionsChange,
  onToggleItem,
  onToggleCategory,
  onToggleExpand,
}: CategoryCardProps) {
  const checkRef = useRef<HTMLInputElement>(null);

  const selectedCount = category.items.filter((i) => selected.has(i.id)).length;
  const allSelected = selectedCount === category.items.length;
  const someSelected = selectedCount > 0 && !allSelected;

  // Setar indeterminate via ref (não pode ser feito via atributo JSX)
  useEffect(() => {
    if (checkRef.current) {
      checkRef.current.indeterminate = someSelected;
    }
  }, [someSelected]);

  const catSelectedSize = category.items.reduce(
    (sum, item) => sum + (selected.has(item.id) ? item.size_bytes : 0),
    0,
  );

  return (
    <div className={styles.categoryCard}>
      <div className={styles.categoryHeader}>
        <input
          ref={checkRef}
          type="checkbox"
          className={styles.checkbox}
          checked={allSelected}
          onChange={onToggleCategory}
        />
        <CategoryIcon categoryId={category.id} />
        <div className={styles.categoryInfo}>
          <span className={styles.categoryName}>{category.name}</span>
          <span className={styles.categoryDesc}>{category.description}</span>
        </div>
        <span className={styles.categorySize}>
          {catSelectedSize > 0 && catSelectedSize !== category.total_size_bytes
            ? `${formatBytes(catSelectedSize)} / ${formatBytes(category.total_size_bytes)}`
            : formatBytes(category.total_size_bytes)}
        </span>
        <RiskBadge risk={category.risk} />
        <ChevronDown
          size={16}
          className={`${styles.chevron} ${isExpanded ? styles.chevronOpen : ''}`}
          onClick={(e) => {
            e.stopPropagation();
            onToggleExpand();
          }}
        />
      </div>

      {isExpanded && (
        <div className={styles.categoryItems}>
          {browserOptions && onBrowserOptionsChange && (
            <BrowserOptionsPanel options={browserOptions} onChange={onBrowserOptionsChange} />
          )}
          {category.items.map((item) => (
            <div className={styles.itemRow} key={item.id}>
              <input
                type="checkbox"
                className={styles.checkbox}
                checked={selected.has(item.id)}
                onChange={() => onToggleItem(item.id)}
              />
              <span className={styles.itemName}>{item.name}</span>
              {!item.default_selected && (
                <span
                  className={styles.itemWarning}
                  title="Dados pessoais — não incluído por padrão"
                >
                  <ShieldAlert size={11} />
                </span>
              )}
              <span className={styles.itemPath}>{item.path_display}</span>
              <span className={styles.itemSize}>{formatBytes(item.size_bytes)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Subcomponente: Opções de browser ────────────────────────────────────────

function BrowserOptionsPanel({
  options,
  onChange,
}: {
  options: BrowserCleanOptions;
  onChange: (opts: BrowserCleanOptions) => void;
}) {
  const toggle = (key: keyof BrowserCleanOptions) => {
    onChange({ ...options, [key]: !options[key] });
  };

  return (
    <div className={styles.browserOptions}>
      <span className={styles.browserOptionsLabel}>Tipos de dados a limpar:</span>
      <div className={styles.browserOptionsGrid}>
        <label className={styles.browserOption}>
          <input
            type="checkbox"
            className={styles.checkbox}
            checked={options.cache}
            onChange={() => toggle('cache')}
          />
          <span>Cache</span>
        </label>
        <label className={styles.browserOption}>
          <input
            type="checkbox"
            className={styles.checkbox}
            checked={options.sessions}
            onChange={() => toggle('sessions')}
          />
          <span>Sessões</span>
        </label>
        <label className={styles.browserOption}>
          <input
            type="checkbox"
            className={styles.checkbox}
            checked={options.cookies}
            onChange={() => toggle('cookies')}
          />
          <span>Cookies</span>
        </label>
        <label className={styles.browserOption}>
          <input
            type="checkbox"
            className={styles.checkbox}
            checked={options.history}
            onChange={() => toggle('history')}
          />
          <span>Histórico</span>
        </label>
      </div>
      {options.cookies && (
        <div className={styles.browserCookieWarning}>
          <AlertTriangle size={12} />
          Você será desconectado de todos os sites nos browsers selecionados.
        </div>
      )}
    </div>
  );
}

// ─── Subcomponente: Relatório final ─────────────────────────────────────────

function ReportView({ result, onNewScan }: { result: CleanupResult; onNewScan: () => void }) {
  // Filtrar items com resultado > 0
  const visibleItems = result.item_results.filter(
    (r) => r.freed_bytes > 0 || r.files_removed > 0 || r.errors.length > 0,
  );

  return (
    <div className={styles.reportState}>
      <div className={styles.reportSummary}>
        <CheckCircle size={36} className={styles.reportIcon} />
        <h2 className={styles.reportTitle}>{formatBytes(result.total_freed_bytes)} liberados</h2>
        <p className={styles.reportSubtitle}>
          {result.total_files_removed} arquivo(s) removidos em{' '}
          {formatDuration(result.duration_seconds)}
        </p>
        {result.total_files_skipped > 0 && (
          <p className={styles.skippedNote}>
            {result.total_files_skipped} arquivo(s) ignorado(s) por estarem em uso
          </p>
        )}
      </div>

      {visibleItems.length > 0 && (
        <div className={styles.reportDetails}>
          {visibleItems.map((item) => (
            <div className={styles.reportItem} key={item.id}>
              <span className={styles.reportItemName}>{item.name}</span>
              <span className={styles.reportItemFreed}>{formatBytes(item.freed_bytes)}</span>
              <span className={styles.reportItemFiles}>
                {item.files_removed} removidos
                {item.files_skipped > 0 && `, ${item.files_skipped} ignorados`}
              </span>
            </div>
          ))}
        </div>
      )}

      {result.locking_processes.length > 0 && (
        <div className={styles.lockingPanel}>
          <div className={styles.lockingTitle}>
            <AlertTriangle size={14} />
            Processos que impediram remoção
          </div>
          {result.locking_processes.map((proc) => (
            <div key={proc.pid} className={styles.lockingProcess}>
              <span className={styles.lockingName}>{proc.name}</span>
              <span className={styles.lockingPid}>PID {proc.pid}</span>
              <span className={styles.lockingCount}>{proc.file_count} arquivo(s)</span>
            </div>
          ))}
        </div>
      )}

      <div className={styles.reportActions}>
        <button className={styles.scanBtn} onClick={onNewScan}>
          <RefreshCw size={14} />
          Novo escaneamento
        </button>
      </div>
    </div>
  );
}
