// Página "Sobre" do FrameGuard.
//
// Manifesto completo, versão, links (GitHub, autor), verificar atualizações,
// licença GPL v3 e referência sutil à Volt.

import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import {
  Github,
  ExternalLink,
  User,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Download,
  X as XIcon,
  Loader2,
  Crosshair,
  Heart,
  Eye,
  Gem,
} from 'lucide-react';
import styles from './About.module.css';
import voltIcon from '../midia/volt_icon.png';
import FrameGuardIcon from '../components/FrameGuardIcon';

// ── Constantes ──────────────────────────────────────────────────────────────────

const APP_VERSION = '0.2.1';
const GITHUB_URL = 'https://github.com/marcelopepis/FrameGuard';
const RELEASES_URL = 'https://github.com/marcelopepis/FrameGuard/releases';
const AUTHOR_NAME = 'Marcelo Pepis';
const AUTHOR_URL = 'https://github.com/marcelopepis';

// ── Tipos ───────────────────────────────────────────────────────────────────────

type UpdaterState =
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'up-to-date' }
  | { status: 'available'; version: string; body: string | null }
  | { status: 'downloading'; downloaded: number; total: number | null }
  | { status: 'installing' }
  | { status: 'error'; message: string };

// ── Componente ──────────────────────────────────────────────────────────────────

export default function About() {
  const [updaterState, setUpdaterState] = useState<UpdaterState>({ status: 'idle' });

  async function handleOpenUrl(url: string) {
    try {
      await openUrl(url);
    } catch {}
  }

  async function handleCheckUpdate() {
    setUpdaterState({ status: 'checking' });
    try {
      const update = await check();
      if (!update?.available) {
        setUpdaterState({ status: 'up-to-date' });
        return;
      }
      setUpdaterState({
        status: 'available',
        version: update.version,
        body: update.body ?? null,
      });
    } catch (e) {
      setUpdaterState({ status: 'error', message: String(e) });
    }
  }

  async function handleInstallUpdate() {
    if (updaterState.status !== 'available') return;

    try {
      const update = await check();
      if (!update?.available) return;

      setUpdaterState({ status: 'downloading', downloaded: 0, total: null });

      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          setUpdaterState({
            status: 'downloading',
            downloaded: 0,
            total: event.data.contentLength ?? null,
          });
        } else if (event.event === 'Progress') {
          setUpdaterState((prev) =>
            prev.status === 'downloading'
              ? { ...prev, downloaded: prev.downloaded + event.data.chunkLength }
              : prev,
          );
        } else if (event.event === 'Finished') {
          setUpdaterState({ status: 'installing' });
        }
      });

      await relaunch();
    } catch (e) {
      setUpdaterState({ status: 'error', message: String(e) });
    }
  }

  return (
    <div className={styles.page}>
      {/* ── Header ── */}
      <div className={styles.header}>
        <h1 className={styles.title}>Sobre o FrameGuard</h1>
        <p className={styles.subtitle}>Manifesto, versão e informações do projeto</p>
      </div>

      <div className={styles.sections}>
        {/* ════════ IDENTIDADE ════════ */}
        <div className={styles.card}>
          <div className={styles.identity}>
            <div className={styles.logo}>
              <FrameGuardIcon size={28} />
            </div>
            <div className={styles.identityText}>
              <div className={styles.appName}>FrameGuard</div>
              <div className={styles.tagline}>Para quem joga, por quem entende.</div>
              <div className={styles.version}>Versão {APP_VERSION} · GPL v3 · Windows 11</div>
            </div>
          </div>
        </div>

        {/* ════════ MANIFESTO ════════ */}
        <div className={styles.card}>
          {/* Introdução */}
          <p className={styles.manifestoIntro}>
            O FrameGuard existe por um motivo simples: seu PC deveria trabalhar pra você, não contra
            você. Sabemos como é — você só quer jogar depois de um dia longo, mas o Windows decidiu
            que agora é hora de atualizar, indexar, coletar telemetria e rodar doze serviços que
            você nunca pediu.
          </p>

          <div className={styles.divider} />

          {/* O que fazemos */}
          <div className={styles.manifestoSection}>
            <div className={styles.manifestoHeading}>O que fazemos</div>
            <p
              style={{
                margin: 0,
                fontSize: '13px',
                color: 'var(--color-text-secondary)',
                lineHeight: 1.65,
              }}
            >
              Otimizamos, limpamos e cuidamos do seu Windows para que ele saia do caminho e deixe
              você jogar. Cada ajuste é explicado em detalhe, classificado por nível de evidência, e
              pode ser revertido com um clique. Sem mágica. Sem promessa de "200% mais FPS". Só
              configurações reais, documentadas, que fazem diferença mensurável.
            </p>
          </div>

          <div className={styles.divider} />

          {/* O que NÃO fazemos */}
          <div className={styles.manifestoSection}>
            <div className={styles.manifestoHeading}>O que NÃO fazemos</div>
            <ul className={styles.manifestoList}>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>Não coletamos seus dados.</span>{' '}
                  Zero telemetria. O FrameGuard não faz requisição pra servidor nenhum. Suas
                  configurações ficam no seu PC, em arquivos JSON que você pode abrir e ler.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>
                    Não vendemos nada dentro do app.
                  </span>{' '}
                  Sem versão PRO, sem paywall, sem feature bloqueada.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>
                    Não instalamos nada silenciosamente.
                  </span>{' '}
                  O FrameGuard não se coloca pra iniciar com o Windows, não cria serviço oculto, não
                  roda em segundo plano.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>Não enfeitamos snake oil.</span> Se
                  um tweak não tem evidência de que funciona, a gente diz isso na cara. Se a
                  comunidade validar que é lorota, a gente tira do app.
                </span>
              </li>
            </ul>
          </div>

          <div className={styles.divider} />

          {/* Pilares */}
          <div className={styles.pillars}>
            <div className={styles.pillar}>
              <div className={styles.pillarName}>
                <Crosshair size={14} /> No-bullshit
              </div>
              <div className={styles.pillarDesc}>
                Sem promessas vazias. Cada tweak tem uma classificação honesta de evidência:
                comprovado, plausível ou não comprovado. Você decide sabendo o que esperar.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}>
                <Heart size={14} /> Respeito
              </div>
              <div className={styles.pillarDesc}>
                Pela sua inteligência e pela sua privacidade. Você não precisa que a gente decida
                por você. A gente explica, você escolhe.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}>
                <Eye size={14} /> Transparência
              </div>
              <div className={styles.pillarDesc}>
                Tudo que o FrameGuard faz no seu sistema é documentado, reversível e auditável.
                Backups em JSON legível. Código aberto. Sem caixa preta.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}>
                <Gem size={14} /> Craft
              </div>
              <div className={styles.pillarDesc}>
                Software útil não precisa ser feio. O FrameGuard é feito com o mesmo cuidado que
                você tem quando escolhe cada peça do seu setup.
              </div>
            </div>
          </div>

          <div className={styles.divider} />

          <div className={styles.manifestoFooter}>
            FrameGuard é gratuito, open-source, e feito por quem joga pra quem joga.
          </div>
        </div>

        {/* ════════ LINKS E METADADOS ════════ */}
        <div className={styles.card}>
          {/* ── Seção do Updater ── */}
          <div className={styles.updateSection}>
            <div className={styles.updateInfo}>
              <div className={styles.updateLabel}>Atualizações</div>
              <div className={styles.updateDesc}>
                Verifica e instala a versão mais recente automaticamente
              </div>
            </div>
            {updaterState.status === 'checking' ? (
              <span className={styles.actionBusy}>
                <Loader2 size={13} className={styles.spinner} /> Verificando...
              </span>
            ) : updaterState.status === 'downloading' || updaterState.status === 'installing' ? (
              <span className={styles.actionBusy}>
                <Loader2 size={13} className={styles.spinner} />
                {updaterState.status === 'installing' ? 'Instalando...' : 'Baixando...'}
              </span>
            ) : (
              <button className={styles.btnPrimary} onClick={handleCheckUpdate}>
                <RefreshCw size={13} /> Verificar
              </button>
            )}
          </div>

          {/* Resultado da verificação — seção dedicada */}
          {updaterState.status !== 'idle' && updaterState.status !== 'checking' && (
            <div className={styles.updaterSection}>
              {/* Up-to-date */}
              {updaterState.status === 'up-to-date' && (
                <div className={styles.updaterCard}>
                  <div className={`${styles.statusRow} ${styles.successText}`}>
                    <CheckCircle2 size={16} />
                    <span>Você está na versão mais recente (v{APP_VERSION})</span>
                  </div>
                </div>
              )}

              {/* Update available */}
              {updaterState.status === 'available' && (
                <div className={styles.updateAvailableCard}>
                  <div className={styles.updaterTitle}>
                    <Download size={16} />
                    Nova versão v{updaterState.version} disponível
                  </div>
                  <div className={styles.statusRow}>
                    <span>Versão atual: v{APP_VERSION}</span>
                  </div>
                  {updaterState.body && (
                    <div className={styles.changelogText}>{updaterState.body}</div>
                  )}
                  <button className={styles.btnPrimary} onClick={handleInstallUpdate}>
                    <Download size={13} /> Baixar e instalar
                  </button>
                </div>
              )}

              {/* Downloading */}
              {updaterState.status === 'downloading' && (
                <div className={styles.updaterCard}>
                  <div className={styles.statusRow}>
                    <Loader2 size={16} className={styles.spinner} />
                    <span>Baixando atualização...</span>
                  </div>
                  {updaterState.total != null && updaterState.total > 0 && (
                    <div className={styles.progressBar}>
                      <div
                        className={styles.progressFill}
                        style={{
                          width: `${Math.min(100, (updaterState.downloaded / updaterState.total) * 100)}%`,
                        }}
                      />
                    </div>
                  )}
                </div>
              )}

              {/* Installing */}
              {updaterState.status === 'installing' && (
                <div className={styles.updaterCard}>
                  <div className={styles.statusRow}>
                    <Loader2 size={16} className={styles.spinner} />
                    <span>Instalando... o app será reiniciado em instantes</span>
                  </div>
                </div>
              )}

              {/* Error */}
              {updaterState.status === 'error' && (
                <div className={styles.updaterCard}>
                  <div className={`${styles.statusRow} ${styles.errorText}`}>
                    <AlertCircle size={16} />
                    <span>Erro ao verificar: {updaterState.message}</span>
                  </div>
                  <button
                    className={styles.updateLink}
                    onClick={() => handleOpenUrl(RELEASES_URL)}
                  >
                    Ver releases no GitHub
                  </button>
                </div>
              )}
            </div>
          )}

          <div className={styles.divider} />

          {/* GitHub */}
          <button className={styles.linkRow} onClick={() => handleOpenUrl(GITHUB_URL)}>
            <Github size={15} />
            Repositório no GitHub
            <ExternalLink size={11} style={{ marginLeft: 'auto', opacity: 0.5 }} />
          </button>

          <div className={styles.divider} />

          {/* Autor */}
          <button className={styles.linkRowMuted} onClick={() => handleOpenUrl(AUTHOR_URL)}>
            <User size={15} />
            Desenvolvido por {AUTHOR_NAME}
            <ExternalLink size={11} style={{ marginLeft: 'auto', opacity: 0.5 }} />
          </button>

          <div className={styles.divider} />

          {/* Metadados */}
          <div className={styles.metaGrid}>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>Licença</span>
              <span className={styles.metaValue}>GPL v3</span>
            </div>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>Plataforma</span>
              <span className={styles.metaValue}>Windows 11</span>
            </div>
            <div className={styles.metaRow}>
              <span className={styles.metaLabel}>Framework</span>
              <span className={styles.metaValue}>Tauri v2 · React 19 · Rust</span>
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

          {/* Volt reference */}
          <div className={styles.voltRef}>
            <img src={voltIcon} alt="Volt" className={styles.voltIcon} />
            <span>Powered by Volt</span>
          </div>
        </div>
      </div>
    </div>
  );
}
