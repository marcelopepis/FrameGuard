import { ReactNode, RefObject } from 'react';
import Sidebar from './Sidebar';
import styles from './layout.module.css';

interface LayoutProps {
  children: ReactNode;
  mainRef?: RefObject<HTMLElement | null>;
}

// Wrapper principal: sidebar fixa + área de conteúdo com scroll
export default function Layout({ children, mainRef }: LayoutProps) {
  return (
    <div className={styles.layout}>
      <Sidebar />
      <main ref={mainRef} className={styles.content}>
        {children}
      </main>
    </div>
  );
}
