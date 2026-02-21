// Contexto global de notificações toast do FrameGuard.
//
// Disponibiliza showToast() para qualquer página/componente mostrar
// notificações não-intrusivas no canto superior direito da janela.
// O ToastProvider renderiza o ToastContainer via portal em document.body.

import {
  createContext, useContext, useState, useCallback, useRef, ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import ToastContainer from '../components/Toast/Toast';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface ToastItem {
  id: string;
  type: ToastType;
  title: string;
  message?: string;
  /** Duração em ms. 0 = persistente (sem auto-dismiss). */
  duration: number;
  /** true enquanto a animação de saída está rodando */
  dismissing: boolean;
}

interface ToastCtx {
  /** Exibe um toast. duration=0 cria toast persistente sem auto-dismiss. */
  showToast: (type: ToastType, title: string, message?: string, duration?: number) => void;
}

const ToastContext = createContext<ToastCtx>({ showToast: () => {} });

/** Duração da animação de saída em ms (deve coincidir com o CSS). */
const DISMISS_ANIM_MS = 250;

/** Máximo de toasts visíveis simultaneamente. */
const MAX_TOASTS = 3;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const timers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const dismissToast = useCallback((id: string) => {
    const timer = timers.current.get(id);
    if (timer) { clearTimeout(timer); timers.current.delete(id); }

    // Marca como dismissing para disparar animação de saída
    setToasts(prev => prev.map(t => t.id === id ? { ...t, dismissing: true } : t));

    // Remove do array após a animação
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, DISMISS_ANIM_MS);
  }, []);

  const showToast = useCallback((
    type: ToastType,
    title: string,
    message?: string,
    duration = 4000,
  ) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2)}`;

    setToasts(prev => {
      // Remove o mais antigo caso já esteja no limite
      const list = prev.filter(t => !t.dismissing).length >= MAX_TOASTS
        ? prev.slice(1)
        : prev;
      return [...list, { id, type, title, message, duration, dismissing: false }];
    });

    if (duration > 0) {
      const timer = setTimeout(() => dismissToast(id), duration);
      timers.current.set(id, timer);
    }
  }, [dismissToast]);

  return (
    <ToastContext.Provider value={{ showToast }}>
      {children}
      {createPortal(
        <ToastContainer toasts={toasts} onDismiss={dismissToast} />,
        document.body,
      )}
    </ToastContext.Provider>
  );
}

export function useToast() {
  return useContext(ToastContext);
}
