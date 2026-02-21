// Página de Configurações do FrameGuard.
//
// Seções: Geral (preferências de UI), Backup e Dados (export/import/backups),
// Sobre (versão, licença, repositório).

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { openPath } from '@tauri-apps/plugin-opener';
import { dataDir as getDataDir } from '@tauri-apps/api/path';
import {
  Download, Upload, FolderOpen, Shield, Github,
  ChevronDown, ChevronUp,
  Loader2, RefreshCw, MonitorCog, Database, Info,
} from 'lucide-react';
import styles from './Settings.module.css';
import { useToast } from '../contexts/ToastContext';

// ── Tipos ──────────────────────────────────────────────────────────────────────

interface ExportResult {
  file_path: string;
  file_size_bytes: number;
  backup_count: number;
  plan_count: number;
  exported_at: string;
}

interface FgFileInfo {
  version: string;
  app_version: string;
  exported_at: string;
  machine_info: { hostname: string; os_version: string };
  backup_count: number;
  plan_count: number;
}

interface ImportResult {
  mode: string;
  backups_imported: number;
  plans_imported: number;
  warnings: string[];
}

interface TweakInfo {
  id: string;
  name: string;
  is_applied: boolean;
  has_backup: boolean;
  last_applied: string | null;
}

// ── Constantes ─────────────────────────────────────────────────────────────────

const APP_VERSION = '0.1.0';
const GITHUB_URL = 'https://github.com/marcesengel/frameguard';

const TWEAK_INFO_COMMANDS = [
  'get_wallpaper_compression_info',
  'get_reserved_storage_info',
  'get_delivery_optimization_info',
];

// ── Utilitários ────────────────────────────────────────────────────────────────

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('pt-BR', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  });
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Subcomponentes auxiliares ──────────────────────────────────────────────────

interface SectionHeadingProps {
  Icon: React.ComponentType<{ size?: number }>;
  title: string;
  description?: string;
}

function SectionHeading({ Icon, title, description }: SectionHeadingProps) {
  return (
    <div className={styles.sectionHeading}>
      <div className={styles.sectionIcon}><Icon size={15} /></div>
      <div>
        <div className={styles.sectionTitle}>{title}</div>
        {description && <div className={styles.sectionSubtitle}>{description}</div>}
      </div>
    </div>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}

function Toggle({ checked, onChange, disabled }: ToggleProps) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={`${styles.toggle} ${checked ? styles.toggleOn : ''} ${disabled ? styles.toggleDisabled : ''}`}
      onClick={() => !disabled && onChange(!checked)}
    >
      <span className={styles.toggleThumb} />
    </button>
  );
}

interface SettingRowProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

function SettingRow({ label, description, children }: SettingRowProps) {
  return (
    <div className={styles.settingRow}>
      <div className={styles.settingLabel}>
        <span className={styles.settingName}>{label}</span>
        {description && <span className={styles.settingDesc}>{description}</span>}
      </div>
      <div className={styles.settingControl}>{children}</div>
    </div>
  );
}

// ── Componente principal ───────────────────────────────────────────────────────

export default function Settings() {

  // ── Estado: Geral (localStorage)
  const [language,         setLanguageState        ] = useState(() => localStorage.getItem('fg.language')      || 'pt-BR');
  const [minimizeToTray,   setMinimizeToTrayState  ] = useState(() => localStorage.getItem('fg.minimizeTray')  === 'true');
  const [startWithWindows, setStartWithWindowsState] = useState(() => localStorage.getItem('fg.startWindows')  === 'true');

  // ── Estado: Pasta de dados
  const [dataDirPath, setDataDirPath] = useState('');

  // ── Estado: Exportar
  const [exportLoading, setExportLoading] = useState(false);

  // ── Estado: Importar
  type ImportStep = 'idle' | 'selecting' | 'preview' | 'importing';
  const [importStep, setImportStep] = useState<ImportStep>('idle');
  const [importInfo, setImportInfo] = useState<FgFileInfo | null>(null);
  const [importMode, setImportMode] = useState<'replace' | 'merge'>('merge');

  // ── Estado: Backups
  const [showBackups,    setShowBackups   ] = useState(false);
  const [backups,        setBackups       ] = useState<TweakInfo[]>([]);
  const [backupsLoading, setBackupsLoading] = useState(false);

  const { showToast } = useToast();

  // ── Inicialização: carrega caminho da pasta de dados
  useEffect(() => {
    getDataDir()
      .then(dir => setDataDirPath(dir.replace(/[/\\]$/, '') + '\\FrameGuard'))
      .catch(() => setDataDirPath('%APPDATA%\\FrameGuard'));
  }, []);

  // ── Helpers: Geral (persiste no localStorage)
  function setLanguage(v: string)       { setLanguageState(v);         localStorage.setItem('fg.language',     v); }
  function setMinimizeTray(v: boolean)  { setMinimizeToTrayState(v);   localStorage.setItem('fg.minimizeTray', String(v)); }
  function setStartWindows(v: boolean)  { setStartWithWindowsState(v); localStorage.setItem('fg.startWindows', String(v)); }

  // ── Exportar
  async function handleExport() {
    setExportLoading(true);
    try {
      const result = await invoke<ExportResult>('export_config');
      showToast('success', 'Exportado com sucesso!',
        `${result.backup_count} backup(s) · ${result.plan_count} plano(s) · ${formatBytes(result.file_size_bytes)}`);
    } catch (e) {
      const msg = String(e);
      if (msg.toLowerCase().includes('cancelad')) return;
      showToast('error', 'Erro ao exportar', msg);
    } finally {
      setExportLoading(false);
    }
  }

  // ── Importar: passo 1 — selecionar arquivo para preview via diálogo frontend
  async function handleSelectFile() {
    setImportStep('selecting');
    setImportInfo(null);
    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [{ name: 'Configuração FrameGuard', extensions: ['fg'] }],
      });
      if (!selected) { setImportStep('idle'); return; }

      // Em Tauri v2, plugin-dialog retorna o path como string diretamente
      const filePath = typeof selected === 'string' ? selected : String(selected);
      const info = await invoke<FgFileInfo>('validate_fg_file', { filePath });
      setImportInfo(info);
      setImportStep('preview');
    } catch (e) {
      showToast('error', 'Erro ao carregar arquivo', String(e));
      setImportStep('idle');
    }
  }

  // ── Importar: passo 2 — confirmar (abre diálogo nativo do Rust para selecionar novamente)
  async function handleConfirmImport() {
    setImportStep('importing');
    try {
      const result = await invoke<ImportResult>('import_config', { mode: importMode });
      const detail = `${result.backups_imported} backup(s) · ${result.plans_imported} plano(s) · modo: ${result.mode}`;
      showToast('success', 'Importação concluída!', detail);
      if (result.warnings.length > 0) {
        showToast('warning', 'Avisos na importação', result.warnings.join('; '));
      }
      resetImport();
    } catch (e) {
      const msg = String(e);
      if (msg.toLowerCase().includes('cancelad')) { setImportStep('preview'); return; }
      showToast('error', 'Erro ao importar', msg);
      resetImport();
    }
  }

  function resetImport() {
    setImportStep('idle');
    setImportInfo(null);
  }

  // ── Ver Backups
  async function loadBackups() {
    setBackupsLoading(true);
    try {
      const results = await Promise.all(
        TWEAK_INFO_COMMANDS.map(cmd => invoke<TweakInfo>(cmd))
      );
      setBackups(results);
    } catch (e) {
      console.error('Erro ao carregar backups:', e);
    } finally {
      setBackupsLoading(false);
    }
  }

  function toggleBackups() {
    const next = !showBackups;
    setShowBackups(next);
    if (next && backups.length === 0) loadBackups();
  }

  // ── Abrir pasta de dados no Explorer
  async function handleOpenDataDir() {
    try { await openPath(dataDirPath); } catch {}
  }

  // ── Abrir URL externa
  async function handleOpenUrl(url: string) {
    try { await openPath(url); } catch {}
  }

  // ── Render ────────────────────────────────────────────────────────────────────

  return (
    <div className={styles.page}>

      {/* ── Header ── */}
      <div className={styles.header}>
        <h1 className={styles.title}>Configurações</h1>
        <p className={styles.subtitle}>Preferências e gerenciamento de dados</p>
      </div>

      <div className={styles.sections}>

        {/* ════════ SEÇÃO: GERAL ════════ */}
        <section className={styles.section}>
          <SectionHeading
            Icon={MonitorCog}
            title="Geral"
            description="Comportamento e aparência do aplicativo"
          />

          <div className={styles.card}>
            <SettingRow label="Idioma" description="Idioma da interface do FrameGuard">
              <select
                className={styles.select}
                value={language}
                onChange={e => setLanguage(e.target.value)}
                disabled
              >
                <option value="pt-BR">Português (Brasil)</option>
                <option value="en-US">English (US)</option>
              </select>
              <span className={styles.badgeSoon}>Em breve</span>
            </SettingRow>

            <div className={styles.divider} />

            <SettingRow
              label="Minimizar para bandeja"
              description="Ao fechar a janela, manter ativo na área de notificação"
            >
              <Toggle checked={minimizeToTray} onChange={setMinimizeTray} disabled />
              <span className={styles.badgeSoon}>Em breve</span>
            </SettingRow>

            <div className={styles.divider} />

            <SettingRow
              label="Iniciar com o Windows"
              description="Abrir automaticamente durante a inicialização do sistema"
            >
              <Toggle checked={startWithWindows} onChange={setStartWindows} disabled />
              <span className={styles.badgeSoon}>Em breve</span>
            </SettingRow>

            <div className={styles.comingSoonNote}>
              As integrações com o sistema operacional serão ativadas em uma versão futura.
            </div>
          </div>
        </section>

        {/* ════════ SEÇÃO: BACKUP E DADOS ════════ */}
        <section className={styles.section}>
          <SectionHeading
            Icon={Database}
            title="Backup e Dados"
            description="Exportação, importação e gerenciamento dos dados do FrameGuard"
          />

          {/* ── Exportar Configurações ── */}
          <div className={styles.card}>
            <div className={styles.cardRow}>
              <div className={styles.rowInfo}>
                <span className={styles.rowName}>Exportar Configurações</span>
                <span className={styles.rowDesc}>
                  Salva todos os backups de tweaks e planos em um arquivo{' '}
                  <code className={styles.code}>.fg</code> auditável
                </span>
              </div>
              <div className={styles.rowAction}>
                {exportLoading ? (
                  <span className={styles.actionBusy}>
                    <Loader2 size={13} className={styles.spinner} /> Exportando...
                  </span>
                ) : (
                  <button className={styles.btnPrimary} onClick={handleExport}>
                    <Download size={13} /> Exportar
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* ── Importar Configurações ── */}
          <div className={styles.card}>
            <div className={styles.cardRow}>
              <div className={styles.rowInfo}>
                <span className={styles.rowName}>Importar Configurações</span>
                <span className={styles.rowDesc}>
                  Restaura backups e planos de um arquivo{' '}
                  <code className={styles.code}>.fg</code> exportado anteriormente
                </span>
              </div>
              <div className={styles.rowAction}>
                {importStep === 'idle' && (
                  <button className={styles.btnSecondary} onClick={handleSelectFile}>
                    <Upload size={13} /> Selecionar arquivo
                  </button>
                )}
                {importStep === 'selecting' && (
                  <span className={styles.actionBusy}>
                    <Loader2 size={13} className={styles.spinner} /> Aguardando...
                  </span>
                )}
              </div>
            </div>

            {/* ── Preview + seleção de modo ── */}
            {(importStep === 'preview' || importStep === 'importing') && importInfo && (
              <div className={styles.importPreview}>
                <div className={styles.previewTitle}>Prévia do arquivo selecionado</div>

                <div className={styles.previewGrid}>
                  <div className={styles.previewField}>
                    <span className={styles.previewLabel}>Computador</span>
                    <span className={styles.previewValue}>{importInfo.machine_info.hostname}</span>
                  </div>
                  <div className={styles.previewField}>
                    <span className={styles.previewLabel}>Sistema</span>
                    <span className={styles.previewValue}>{importInfo.machine_info.os_version}</span>
                  </div>
                  <div className={styles.previewField}>
                    <span className={styles.previewLabel}>Exportado em</span>
                    <span className={styles.previewValue}>{formatDate(importInfo.exported_at)}</span>
                  </div>
                  <div className={styles.previewField}>
                    <span className={styles.previewLabel}>Conteúdo</span>
                    <span className={styles.previewValue}>
                      {importInfo.backup_count} backup(s) · {importInfo.plan_count} plano(s)
                    </span>
                  </div>
                  <div className={styles.previewField}>
                    <span className={styles.previewLabel}>Versão</span>
                    <span className={styles.previewValue}>
                      formato v{importInfo.version} · app {importInfo.app_version}
                    </span>
                  </div>
                </div>

                <div className={styles.modeSection}>
                  <div className={styles.modeTitle}>Modo de importação</div>
                  <div className={styles.modeOptions}>
                    <label className={`${styles.modeOption} ${importMode === 'merge' ? styles.modeSelected : ''}`}>
                      <input
                        type="radio"
                        name="importMode"
                        value="merge"
                        checked={importMode === 'merge'}
                        onChange={() => setImportMode('merge')}
                      />
                      <div>
                        <span className={styles.modeName}>Mesclar</span>
                        <span className={styles.modeDesc}>
                          Adiciona apenas backups e planos que ainda não existem localmente
                        </span>
                      </div>
                    </label>

                    <label className={`${styles.modeOption} ${importMode === 'replace' ? styles.modeDanger : ''}`}>
                      <input
                        type="radio"
                        name="importMode"
                        value="replace"
                        checked={importMode === 'replace'}
                        onChange={() => setImportMode('replace')}
                      />
                      <div>
                        <span className={styles.modeName}>Substituir tudo</span>
                        <span className={styles.modeDesc}>
                          Sobrescreve completamente os dados atuais — ação irreversível
                        </span>
                      </div>
                    </label>
                  </div>
                </div>

                <div className={styles.importFooter}>
                  <span className={styles.importNote}>
                    Um novo diálogo de arquivo será aberto para confirmar a seleção.
                  </span>
                  <div className={styles.importButtons}>
                    <button
                      className={styles.btnGhost}
                      onClick={resetImport}
                      disabled={importStep === 'importing'}
                    >
                      Cancelar
                    </button>
                    <button
                      className={importMode === 'replace' ? styles.btnDanger : styles.btnPrimary}
                      onClick={handleConfirmImport}
                      disabled={importStep === 'importing'}
                    >
                      {importStep === 'importing' ? (
                        <><Loader2 size={13} className={styles.spinner} /> Importando...</>
                      ) : importMode === 'replace' ? (
                        <><Upload size={13} /> Substituir e importar</>
                      ) : (
                        <><Upload size={13} /> Mesclar e importar</>
                      )}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* ── Backups de Tweaks ── */}
          <div className={styles.card}>
            <div className={styles.cardRow}>
              <div className={styles.rowInfo}>
                <span className={styles.rowName}>Backups de Tweaks</span>
                <span className={styles.rowDesc}>
                  Estado dos backups das otimizações aplicadas
                </span>
              </div>
              <div className={styles.rowAction}>
                <button className={styles.btnSecondary} onClick={toggleBackups}>
                  {showBackups ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                  {showBackups ? 'Ocultar' : 'Ver backups'}
                </button>
              </div>
            </div>

            {showBackups && (
              <div className={styles.backupsPanel}>
                {backupsLoading ? (
                  <div className={styles.backupsLoading}>
                    <Loader2 size={14} className={styles.spinner} />
                    <span>Carregando...</span>
                  </div>
                ) : (
                  <>
                    <table className={styles.backupsTable}>
                      <thead>
                        <tr>
                          <th>Tweak</th>
                          <th>Status</th>
                          <th>Backup</th>
                          <th>Última aplicação</th>
                        </tr>
                      </thead>
                      <tbody>
                        {backups.map(t => (
                          <tr key={t.id}>
                            <td className={styles.tweakCell}>{t.name}</td>
                            <td>
                              <span className={`${styles.pill} ${t.is_applied ? styles.pillGreen : styles.pillGray}`}>
                                {t.is_applied ? 'Ativo' : 'Inativo'}
                              </span>
                            </td>
                            <td>
                              <span className={`${styles.pill} ${t.has_backup ? styles.pillCyan : styles.pillGray}`}>
                                {t.has_backup ? 'Disponível' : 'Sem backup'}
                              </span>
                            </td>
                            <td className={styles.dateCell}>
                              {t.last_applied ? formatDate(t.last_applied) : '—'}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    <div className={styles.backupsNote}>
                      Para reverter backups individualmente, acesse a página de Otimizações.
                    </div>
                  </>
                )}
              </div>
            )}
          </div>

          {/* ── Pasta de Dados ── */}
          <div className={styles.card}>
            <div className={styles.cardRow}>
              <div className={styles.rowInfo}>
                <span className={styles.rowName}>Pasta de Dados</span>
                <span className={styles.rowDesc}>
                  Backups e planos são salvos como JSON auditáveis neste diretório
                </span>
              </div>
              <div className={styles.rowAction}>
                <button className={styles.btnSecondary} onClick={handleOpenDataDir}>
                  <FolderOpen size={13} /> Abrir pasta
                </button>
              </div>
            </div>
            <div className={styles.pathRow}>
              <code className={styles.pathCode}>{dataDirPath || 'Carregando...'}</code>
            </div>
          </div>
        </section>

        {/* ════════ SEÇÃO: SOBRE ════════ */}
        <section className={styles.section}>
          <SectionHeading Icon={Info} title="Sobre" />

          <div className={styles.card}>

            <div className={styles.aboutHeader}>
              <div className={styles.aboutLogo}>
                <Shield size={22} />
              </div>
              <div>
                <div className={styles.aboutName}>FrameGuard</div>
                <div className={styles.aboutVersion}>Versão {APP_VERSION}</div>
              </div>
            </div>

            <div className={styles.divider} />

            <button
              className={styles.githubLink}
              onClick={() => handleOpenUrl(GITHUB_URL)}
            >
              <Github size={14} />
              Repositório no GitHub
            </button>

            <div className={styles.divider} />

            <div className={styles.metaGrid}>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>Licença</span>
                <span className={styles.metaValue}>MIT</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>Plataforma</span>
                <span className={styles.metaValue}>Windows 11</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>Framework</span>
                <span className={styles.metaValue}>Tauri v2 · React 18 · Rust</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>Formato de exportação</span>
                <span className={styles.metaValue}>.fg (JSON legível por humanos)</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>Dados de configuração</span>
                <span className={styles.metaValue}>%APPDATA%\FrameGuard</span>
              </div>
            </div>

            <div className={styles.divider} />

            <p className={styles.aboutNote}>
              FrameGuard é um utilitário de manutenção e otimização para Windows 11
              focado em gamers. Todas as modificações são reversíveis e documentadas
              em JSON legível por humanos — total transparência sobre o que é alterado no sistema.
            </p>
          </div>
        </section>

      </div>
    </div>
  );
}
