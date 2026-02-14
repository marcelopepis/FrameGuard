import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Dashboard, Optimizations, Cleanup, Settings } from './pages';

// Componente raiz com roteamento
export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/optimizations" element={<Optimizations />} />
        <Route path="/cleanup" element={<Cleanup />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
