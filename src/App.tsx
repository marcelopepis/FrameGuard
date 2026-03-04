import { useState } from 'react';
import { BrowserRouter, useLocation, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import { Dashboard, Optimizations, Privacy, Maintenance, Cleanup, Services, Plans, Learn, About, Settings } from './pages';
import { GlobalRunningProvider } from './contexts/RunningContext';
import { ToastProvider } from './contexts/ToastContext';
import WelcomeModal from './components/WelcomeModal/WelcomeModal';

// Todas as rotas declaradas aqui. A ordem determina a renderização no DOM.
const ROUTES = [
  { path: '/',              Page: Dashboard },
  { path: '/optimizations', Page: Optimizations },
  { path: '/privacy',       Page: Privacy },
  { path: '/maintenance',   Page: Maintenance },
  { path: '/cleanup',       Page: Cleanup },
  { path: '/services',      Page: Services },
  { path: '/plans',         Page: Plans },
  { path: '/learn',         Page: Learn },
  { path: '/about',         Page: About },
  { path: '/settings',      Page: Settings },
];

// Estilos para cada wrapper de página — cada um tem seu próprio scroll container.
// Páginas inativas ficam com visibility: hidden + height: 0 para manter o estado
// React montado sem ocupar espaço nem capturar scroll.
const activeStyle: React.CSSProperties = {
  flex: 1,
  overflowY: 'auto',
  overflowX: 'hidden',
  minHeight: 0,
};
const hiddenStyle: React.CSSProperties = {
  height: 0,
  overflow: 'hidden',
  visibility: 'hidden',
};

// Renderiza todas as páginas simultaneamente usando keep-alive:
// páginas inativas ficam ocultas mas permanecem montadas — preservando
// estado React, execuções em andamento e listeners Tauri.
// Cada wrapper é seu próprio scroll container, preservando a posição de scroll
// ao alternar entre páginas.
function Pages() {
  const { pathname } = useLocation();
  const isKnown = ROUTES.some(r => r.path === pathname);

  return (
    <Layout>
      {!isKnown && <Navigate to="/" replace />}
      {ROUTES.map(({ path, Page }) => (
        <div key={path} style={pathname === path ? activeStyle : hiddenStyle}>
          <Page />
        </div>
      ))}
    </Layout>
  );
}

export default function App() {
  const [showWelcome, setShowWelcome] = useState(
    () => localStorage.getItem('fg.firstRunSeen') !== 'true'
  );

  return (
    <BrowserRouter>
      <GlobalRunningProvider>
        <ToastProvider>
          <Pages />
          {showWelcome && <WelcomeModal onClose={() => setShowWelcome(false)} />}
        </ToastProvider>
      </GlobalRunningProvider>
    </BrowserRouter>
  );
}
