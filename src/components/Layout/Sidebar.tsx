import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, Zap, ShieldAlert, Wrench, Eraser, Server, ClipboardList, Settings,
  Shield,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import SearchBar from '../SearchBar/SearchBar';
import styles from './sidebar.module.css';

const APP_VERSION = '0.1.0';

type NavItemDef =
  | { kind: 'link'; path: string; label: string; Icon: LucideIcon; end?: boolean }
  | { kind: 'sep' };

const ITEMS: NavItemDef[] = [
  { kind: 'link', path: '/',              label: 'Dashboard',     Icon: LayoutDashboard, end: true },
  { kind: 'link', path: '/optimizations', label: 'Otimizações',   Icon: Zap },
  { kind: 'link', path: '/privacy',       label: 'Privacidade',   Icon: ShieldAlert },
  { kind: 'link', path: '/maintenance',   label: 'Manutenção',    Icon: Wrench },
  { kind: 'link', path: '/cleanup',       label: 'Limpeza',       Icon: Eraser },
  { kind: 'link', path: '/services',      label: 'Serviços',      Icon: Server },
  { kind: 'link', path: '/plans',         label: 'Planos',        Icon: ClipboardList },
  { kind: 'sep' },
  { kind: 'link', path: '/settings',      label: 'Configurações', Icon: Settings },
];

export default function Sidebar() {
  return (
    <aside className={styles.sidebar}>
      {/* Logo */}
      <div className={styles.logo}>
        <Shield className={styles.logoIcon} size={26} strokeWidth={2.5} />
        <span className={styles.logoText}>FrameGuard</span>
      </div>

      {/* Busca global */}
      <SearchBar />

      {/* Navegação principal */}
      <nav className={styles.nav}>
        {ITEMS.map((item, i) => {
          if (item.kind === 'sep') {
            return <div key={i} className={styles.navSeparator} />;
          }
          const { path, label, Icon, end } = item;
          return (
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
          );
        })}
      </nav>

      {/* Rodapé com versão */}
      <div className={styles.footer}>
        <span className={styles.version}>v{APP_VERSION}</span>
      </div>
    </aside>
  );
}
