import { ReactNode } from 'react';
import Sidebar from './Sidebar';
import TitleBar from '../TitleBar/TitleBar';
import styles from './layout.module.css';

interface LayoutProps {
  children: ReactNode;
}

// Wrapper principal: titlebar fixa + sidebar fixa + área de conteúdo
export default function Layout({ children }: LayoutProps) {
  return (
    <div className={styles.shell}>
      <TitleBar />
      <div className={styles.layout}>
        <Sidebar />
        <main className={styles.content}>{children}</main>
      </div>
    </div>
  );
}
