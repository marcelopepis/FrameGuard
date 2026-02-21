// Página de Privacidade do FrameGuard — placeholder.
//
// Futuramente conterá tweaks de privacidade e debloat do Windows:
// desabilitar telemetria, remover apps pré-instalados, bloquear rastreamento, etc.

import { ShieldAlert } from 'lucide-react';
import styles from '../components/ActionCard/ActionCard.module.css';

export default function Privacy() {
  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Privacidade e Debloat</h1>
          <p className={styles.subtitle}>Controle de telemetria, rastreamento e apps desnecessários</p>
        </div>
      </div>

      <div className={styles.placeholder}>
        <ShieldAlert size={48} strokeWidth={1.2} className={styles.placeholderIcon} />
        <h3 className={styles.placeholderTitle}>Em desenvolvimento</h3>
        <p className={styles.placeholderText}>
          Tweaks de privacidade, desabilitação de telemetria e remoção de bloatware
          serão adicionados em breve.
        </p>
      </div>
    </div>
  );
}
