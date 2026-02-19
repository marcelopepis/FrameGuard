import { Outlet } from 'react-router-dom';
import Sidebar from './Sidebar';
import styles from './layout.module.css';

// Wrapper principal: sidebar fixa + área de conteúdo com scroll
export default function Layout() {
  return (
    <div className={styles.layout}>
      <Sidebar />
      <main className={styles.content}>
        <Outlet />
      </main>
    </div>
  );
}
