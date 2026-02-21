// Componente de exibição dos toasts do FrameGuard.
//
// Renderizado via portal em document.body pelo ToastProvider.
// Recebe a lista de toasts ativos e o callback de dismiss.

import { X, CheckCircle2, XCircle, AlertTriangle, Info } from 'lucide-react';
import type { ToastItem } from '../../contexts/ToastContext';
import styles from './Toast.module.css';

interface ToastContainerProps {
  toasts: ToastItem[];
  onDismiss: (id: string) => void;
}

const ICONS = {
  success: CheckCircle2,
  error:   XCircle,
  warning: AlertTriangle,
  info:    Info,
} as const;

export default function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div className={styles.container}>
      {toasts.map(toast => {
        const Icon = ICONS[toast.type];
        return (
          <div
            key={toast.id}
            className={`${styles.toast} ${styles[toast.type]} ${toast.dismissing ? styles.dismissing : ''}`}
            role="alert"
            aria-live="polite"
          >
            <Icon size={16} className={styles.icon} />
            <div className={styles.body}>
              <div className={styles.title}>{toast.title}</div>
              {toast.message && (
                <div className={styles.message}>{toast.message}</div>
              )}
            </div>
            <button
              className={styles.btnClose}
              onClick={() => onDismiss(toast.id)}
              title="Fechar"
            >
              <X size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
