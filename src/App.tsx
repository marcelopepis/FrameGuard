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

// Renderiza apenas a página ativa — quando o usuário navega, a página
// anterior desmonta e a nova monta, evitando carga simultânea de todas.
function Pages() {
  const { pathname } = useLocation();
  const isKnown = ROUTES.some(r => r.path === pathname);

  return (
    <Layout>
      {!isKnown && <Navigate to="/" replace />}
      {ROUTES.map(({ path, Page }) => (
        pathname === path ? <Page key={path} /> : null
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
