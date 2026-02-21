// Página de Serviços do FrameGuard — placeholder.
//
// Futuramente conterá gerenciamento de serviços e tarefas agendadas do Windows:
// desabilitar serviços desnecessários, gerenciar startup, tarefas agendadas, etc.

import { Server } from 'lucide-react';
import styles from '../components/ActionCard/ActionCard.module.css';

export default function Services() {
  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Serviços e Tarefas</h1>
          <p className={styles.subtitle}>Gerenciamento de serviços Windows e tarefas agendadas</p>
        </div>
      </div>

      <div className={styles.placeholder}>
        <Server size={48} strokeWidth={1.2} className={styles.placeholderIcon} />
        <h3 className={styles.placeholderTitle}>Em desenvolvimento</h3>
        <p className={styles.placeholderText}>
          Gerenciamento de serviços do Windows, controle de startup e tarefas
          agendadas serão adicionados em breve.
        </p>
      </div>
    </div>
  );
}
