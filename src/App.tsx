import { useState, useEffect } from 'react';
import { BrowserRouter, useLocation, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import {
  Dashboard,
  Optimizations,
  Privacy,
  Maintenance,
  Cleanup,
  Services,
  Plans,
  Learn,
  About,
  Settings,
} from './pages';
import { GlobalRunningProvider, useGlobalRunning } from './contexts/RunningContext';
import { ToastProvider } from './contexts/ToastContext';
import WelcomeModal from './components/WelcomeModal/WelcomeModal';

// Todas as rotas declaradas aqui. A ordem determina a renderização no DOM.
const ROUTES = [
  { path: '/', Page: Dashboard },
  { path: '/optimizations', Page: Optimizations },
  { path: '/privacy', Page: Privacy },
  { path: '/maintenance', Page: Maintenance },
  { path: '/cleanup', Page: Cleanup },
  { path: '/services', Page: Services },
  { path: '/plans', Page: Plans },
  { path: '/learn', Page: Learn },
  { path: '/about', Page: About },
  { path: '/settings', Page: Settings },
];

// Renderiza apenas a página ativa — quando o usuário navega, a página
// anterior desmonta e a nova monta, evitando carga simultânea de todas.
//
// Exceção: páginas com tarefas ativas (busyPages) são mantidas montadas
// com display:none para preservar estado e event listeners. Quando a tarefa
// termina, a página é "retida" até o usuário revisitá-la (para que veja
// o resultado), e só então pode ser desmontada ao sair novamente.
function Pages() {
  const { pathname } = useLocation();
  const { busyPages } = useGlobalRunning();
  const isKnown = ROUTES.some((r) => r.path === pathname);

  // Páginas retidas: foram busy e ainda não revisitadas após término da tarefa
  const [retained, setRetained] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    setRetained((prev) => {
      const next = new Set(prev);
      // Adiciona páginas recém-busy ao retained
      for (const page of busyPages) next.add(page);
      // Se o usuário está visitando uma página retida que não é mais busy, libera
      if (next.has(pathname) && !busyPages.has(pathname)) next.delete(pathname);
      // Evita re-render desnecessário se nada mudou
      if (next.size === prev.size && [...next].every((p) => prev.has(p))) return prev;
      return next;
    });
  }, [pathname, busyPages]);

  return (
    <Layout>
      {!isKnown && <Navigate to="/" replace />}
      {ROUTES.map(({ path, Page }) => {
        const isActive = pathname === path;
        const shouldRetain = retained.has(path);
        if (!isActive && !shouldRetain) return null;
        return (
          <div
            key={path}
            style={{
              flex: isActive ? 1 : undefined,
              display: isActive ? undefined : 'none',
              overflowY: isActive ? 'auto' : undefined,
              overflowX: 'hidden',
              minHeight: 0,
            }}
          >
            <Page />
          </div>
        );
      })}
    </Layout>
  );
}

export default function App() {
  const [showWelcome, setShowWelcome] = useState(
    () => localStorage.getItem('fg.firstRunSeen') !== 'true',
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
