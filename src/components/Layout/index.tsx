import { ReactNode } from 'react';
import Sidebar from './Sidebar';
import styles from './layout.module.css';

// Wrapper principal: sidebar fixa + área de conteúdo com scroll
export default function Layout({ children }: { children: ReactNode }) {
  return (
    <div className={styles.layout}>
      <Sidebar />
      <main className={styles.content}>
        {children}
      </main>
    </div>
  );
}
