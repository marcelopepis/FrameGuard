import { ReactNode } from 'react';
import Sidebar from './Sidebar';
import styles from './layout.module.css';

interface LayoutProps {
  children: ReactNode;
}

// Wrapper principal: sidebar fixa + área de conteúdo (scroll delegado a cada página)
export default function Layout({ children }: LayoutProps) {
  return (
    <div className={styles.layout}>
      <Sidebar />
      <main className={styles.content}>
        {children}
      </main>
    </div>
  );
}
