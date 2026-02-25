import { useRef, useEffect } from 'react';
import { BrowserRouter, useLocation, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import { Dashboard, Optimizations, Privacy, Maintenance, Services, Plans, Settings } from './pages';
import { GlobalRunningProvider } from './contexts/RunningContext';
import { ToastProvider } from './contexts/ToastContext';

// Todas as rotas declaradas aqui. A ordem determina a renderização no DOM.
const ROUTES = [
  { path: '/',              Page: Dashboard },
  { path: '/optimizations', Page: Optimizations },
  { path: '/privacy',       Page: Privacy },
  { path: '/maintenance',   Page: Maintenance },
  { path: '/services',      Page: Services },
  { path: '/plans',         Page: Plans },
  { path: '/settings',      Page: Settings },
];

// Renderiza todas as páginas simultaneamente usando keep-alive:
// páginas inativas ficam ocultas via CSS (display: none), mas permanecem
// montadas — preservando estado React, execuções em andamento e listeners Tauri.
function Pages() {
  const { pathname } = useLocation();
  const isKnown = ROUTES.some(r => r.path === pathname);
  const mainRef = useRef<HTMLElement>(null);

  // Reseta o scroll do <main> ao trocar de página
  useEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = 0;
  }, [pathname]);

  return (
    <Layout mainRef={mainRef}>
      {!isKnown && <Navigate to="/" replace />}
      {ROUTES.map(({ path, Page }) => (
        <div key={path} style={{ display: pathname === path ? 'contents' : 'none' }}>
          <Page />
        </div>
      ))}
    </Layout>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <GlobalRunningProvider>
        <ToastProvider>
          <Pages />
        </ToastProvider>
      </GlobalRunningProvider>
    </BrowserRouter>
  );
}
