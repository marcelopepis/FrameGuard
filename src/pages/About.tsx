// Página "Sobre" do FrameGuard.
//
// Manifesto completo, versão, links (GitHub, autor), verificar atualizações,
// licença GPL v3 e referência sutil à Volt.

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  Github, ExternalLink, User,
  RefreshCw, CheckCircle2, AlertCircle, X as XIcon,
  Loader2, Crosshair, Heart, Eye, Gem,
} from 'lucide-react';
import styles from './About.module.css';
import voltIcon from '../midia/volt_icon.png';
import FrameGuardIcon from '../components/FrameGuardIcon';

// ── Constantes ──────────────────────────────────────────────────────────────────

const APP_VERSION = '0.1.2';
const GITHUB_URL = 'https://github.com/marcelopepis/FrameGuard';
const AUTHOR_NAME = 'Marcelo Pepis';
const AUTHOR_URL = 'https://github.com/marcelopepis';

// ── Tipos ───────────────────────────────────────────────────────────────────────

interface UpdateCheckResult {
  current_version: string;
  latest_version: string;
  is_update_available: boolean;
  release_url: string;
  release_notes: string;
}

// ── Componente ──────────────────────────────────────────────────────────────────

export default function About() {
  const [updateCheck, setUpdateCheck] = useState<
    { loading: true } | { loading: false; result: UpdateCheckResult } | { loading: false; error: string } | null
  >(null);

  async function handleOpenUrl(url: string) {
    try { await openUrl(url); } catch {}
  }

  async function handleCheckUpdates() {
    setUpdateCheck({ loading: true });
    try {
      const result = await invoke<UpdateCheckResult>('check_for_updates');
      setUpdateCheck({ loading: false, result });
    } catch (e) {
      setUpdateCheck({ loading: false, error: String(e) });
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
            O FrameGuard existe por um motivo simples: seu PC deveria trabalhar pra você,
            não contra você. Sabemos como é — você só quer jogar depois de um dia longo,
            mas o Windows decidiu que agora é hora de atualizar, indexar, coletar telemetria
            e rodar doze serviços que você nunca pediu.
          </p>

          <div className={styles.divider} />

          {/* O que fazemos */}
          <div className={styles.manifestoSection}>
            <div className={styles.manifestoHeading}>O que fazemos</div>
            <p style={{ margin: 0, fontSize: '13px', color: 'var(--color-text-secondary)', lineHeight: 1.65 }}>
              Otimizamos, limpamos e cuidamos do seu Windows para que ele saia do caminho e
              deixe você jogar. Cada ajuste é explicado em detalhe, classificado por nível de
              evidência, e pode ser revertido com um clique. Sem mágica. Sem promessa de
              "200% mais FPS". Só configurações reais, documentadas, que fazem diferença
              mensurável.
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
                  Zero telemetria. O FrameGuard não faz requisição pra servidor nenhum.
                  Suas configurações ficam no seu PC, em arquivos JSON que você pode abrir e ler.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>Não vendemos nada dentro do app.</span>{' '}
                  Sem versão PRO, sem paywall, sem feature bloqueada.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>Não instalamos nada silenciosamente.</span>{' '}
                  O FrameGuard não se coloca pra iniciar com o Windows, não cria serviço
                  oculto, não roda em segundo plano.
                </span>
              </li>
              <li className={styles.manifestoListItem}>
                <XIcon size={14} />
                <span>
                  <span className={styles.manifestoListItemBold}>Não enfeitamos snake oil.</span>{' '}
                  Se um tweak não tem evidência de que funciona, a gente diz isso na cara.
                  Se a comunidade validar que é lorota, a gente tira do app.
                </span>
              </li>
            </ul>
          </div>

          <div className={styles.divider} />

          {/* Pilares */}
          <div className={styles.pillars}>
            <div className={styles.pillar}>
              <div className={styles.pillarName}><Crosshair size={14} /> No-bullshit</div>
              <div className={styles.pillarDesc}>
                Sem promessas vazias. Cada tweak tem uma classificação honesta de evidência:
                comprovado, plausível ou não comprovado. Você decide sabendo o que esperar.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}><Heart size={14} /> Respeito</div>
              <div className={styles.pillarDesc}>
                Pela sua inteligência e pela sua privacidade. Você não precisa que a gente
                decida por você. A gente explica, você escolhe.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}><Eye size={14} /> Transparência</div>
              <div className={styles.pillarDesc}>
                Tudo que o FrameGuard faz no seu sistema é documentado, reversível e
                auditável. Backups em JSON legível. Código aberto. Sem caixa preta.
              </div>
            </div>
            <div className={styles.pillar}>
              <div className={styles.pillarName}><Gem size={14} /> Craft</div>
              <div className={styles.pillarDesc}>
                Software útil não precisa ser feio. O FrameGuard é feito com o mesmo cuidado
                que você tem quando escolhe cada peça do seu setup.
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

          {/* Verificar atualizações */}
          <div className={styles.updateSection}>
            <div className={styles.updateInfo}>
              <div className={styles.updateLabel}>Verificar Atualizações</div>
              <div className={styles.updateDesc}>
                Confere a versão mais recente publicada no GitHub
              </div>
            </div>
            {updateCheck?.loading ? (
              <span className={styles.actionBusy}>
                <Loader2 size={13} className={styles.spinner} /> Verificando...
              </span>
            ) : (
              <button className={styles.btnPrimary} onClick={handleCheckUpdates}>
                <RefreshCw size={13} /> Verificar
              </button>
            )}
          </div>

          {/* Resultado da verificação */}
          {updateCheck && !updateCheck.loading && (
            'error' in updateCheck ? (
              <div className={styles.updateError}>
                <AlertCircle size={15} />
                <span>Erro ao verificar: {updateCheck.error}</span>
              </div>
            ) : updateCheck.result.is_update_available ? (
              <div className={styles.updateAvailable}>
                <RefreshCw size={15} />
                <span>
                  Nova versão <strong>{updateCheck.result.latest_version}</strong> disponível!
                  (atual: {updateCheck.result.current_version})
                </span>
                <button
                  className={styles.updateLink}
                  onClick={() => handleOpenUrl(updateCheck.result.release_url)}
                >
                  Baixar
                </button>
              </div>
            ) : (
              <div className={styles.updateUpToDate}>
                <CheckCircle2 size={15} />
                <span>Você está na versão mais recente ({updateCheck.result.current_version})</span>
              </div>
            )
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
            <img
              src={voltIcon}
              alt="Volt"
              className={styles.voltIcon}
            />
            <span>Powered by Volt</span>
          </div>
        </div>

      </div>
    </div>
  );
}
