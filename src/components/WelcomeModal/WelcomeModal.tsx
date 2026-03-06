// Modal de primeiro uso — exibe versão resumida do manifesto.
// Aparece apenas na primeira execução (flag localStorage: fg.firstRunSeen).

import { CheckCircle2, ArrowRight, Crosshair, Heart, Eye, Gem } from 'lucide-react';
import FrameGuardIcon from '../FrameGuardIcon';
import styles from './WelcomeModal.module.css';

const APP_VERSION = '0.2.0';

interface WelcomeModalProps {
  onClose: () => void;
}

export default function WelcomeModal({ onClose }: WelcomeModalProps) {
  function handleDismiss() {
    try {
      localStorage.setItem('fg.firstRunSeen', 'true');
    } catch {}
    onClose();
  }

  return (
    <div className={styles.overlay}>
      <div className={styles.modal}>
        {/* Header */}
        <div className={styles.header}>
          <div className={styles.logo}>
            <FrameGuardIcon size={26} />
          </div>
          <div className={styles.headerText}>
            <div className={styles.appName}>Bem-vindo ao FrameGuard</div>
            <div className={styles.version}>Versão {APP_VERSION}</div>
          </div>
        </div>

        {/* Body */}
        <div className={styles.body}>
          <p className={styles.quote}>"Seu PC deveria trabalhar pra você, não contra você."</p>

          <p className={styles.desc}>
            O FrameGuard otimiza, limpa e cuida do seu Windows para que ele saia do caminho e deixe
            você jogar. Cada ajuste é explicado, classificado por nível de evidência, e pode ser
            revertido com um clique.
          </p>

          {/* Pilares */}
          <div className={styles.pillars}>
            <span className={styles.pill}>
              <Crosshair size={12} /> No-bullshit
            </span>
            <span className={styles.pill}>
              <Heart size={12} /> Respeito
            </span>
            <span className={styles.pill}>
              <Eye size={12} /> Transparência
            </span>
            <span className={styles.pill}>
              <Gem size={12} /> Craft
            </span>
          </div>

          {/* Destaques */}
          <div className={styles.highlights}>
            <div className={styles.highlight}>
              <CheckCircle2 size={14} />
              Zero telemetria — seus dados ficam no seu PC
            </div>
            <div className={styles.highlight}>
              <CheckCircle2 size={14} />
              Sem versão PRO, sem paywall, sem feature bloqueada
            </div>
            <div className={styles.highlight}>
              <CheckCircle2 size={14} />
              Código aberto — GPL v3
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className={styles.footer}>
          <button className={styles.btnGo} onClick={handleDismiss}>
            Entendi, vamos lá <ArrowRight size={15} />
          </button>
        </div>
      </div>
    </div>
  );
}
