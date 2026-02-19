import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import { Dashboard, Optimizations, Cleanup, HealthCheck, Settings } from './pages';

// Componente raiz: Layout como rota pai, páginas como filhas (Outlet)
export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/"              element={<Dashboard />} />
          <Route path="/optimizations" element={<Optimizations />} />
          <Route path="/cleanup"       element={<Cleanup />} />
          <Route path="/health"        element={<HealthCheck />} />
          <Route path="/settings"      element={<Settings />} />
          <Route path="*"              element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
