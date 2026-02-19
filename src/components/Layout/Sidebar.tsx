import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  Zap,
  Trash2,
  Activity,
  Settings,
  Shield,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import styles from './sidebar.module.css';

const APP_VERSION = '0.1.0';

interface NavItem {
  path: string;
  label: string;
  Icon: LucideIcon;
  end?: boolean;
}

const navItems: NavItem[] = [
  { path: '/',               label: 'Dashboard',       Icon: LayoutDashboard, end: true },
  { path: '/optimizations',  label: 'Otimizações',     Icon: Zap },
  { path: '/cleanup',        label: 'Limpeza',         Icon: Trash2 },
  { path: '/health',         label: 'Saúde do Sistema', Icon: Activity },
  { path: '/settings',       label: 'Configurações',   Icon: Settings },
];

export default function Sidebar() {
  return (
    <aside className={styles.sidebar}>
      {/* Logo */}
      <div className={styles.logo}>
        <Shield className={styles.logoIcon} size={26} strokeWidth={2.5} />
        <span className={styles.logoText}>FrameGuard</span>
      </div>

      {/* Navegação principal */}
      <nav className={styles.nav}>
        {navItems.map(({ path, label, Icon, end }) => (
          <NavLink
            key={path}
            to={path}
            end={end}
            className={({ isActive }) =>
              `${styles.navItem}${isActive ? ` ${styles.navItemActive}` : ''}`
            }
          >
            <Icon size={17} className={styles.navIcon} strokeWidth={2} />
            <span className={styles.navLabel}>{label}</span>
          </NavLink>
        ))}
      </nav>

      {/* Rodapé com versão */}
      <div className={styles.footer}>
        <span className={styles.version}>v{APP_VERSION}</span>
      </div>
    </aside>
  );
}
