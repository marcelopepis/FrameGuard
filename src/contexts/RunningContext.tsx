// Contexto global de execução do FrameGuard.
//
// Rastreia quais chaves de tarefa estão ativas em qualquer parte do app.
// Usado para desabilitar botões "Executar" em todas as páginas enquanto
// um comando de longa duração (DISM, SFC, etc.) estiver em andamento.

import { createContext, useContext, useState, useCallback, ReactNode } from 'react';

interface RunningCtx {
  /** `true` se ao menos uma tarefa estiver em execução em qualquer página */
  isRunning: boolean;
  /** Marca uma tarefa como iniciada; key deve ser única por execução */
  startTask: (key: string) => void;
  /** Marca uma tarefa como finalizada */
  endTask: (key: string) => void;
}

const RunningContext = createContext<RunningCtx>({
  isRunning: false,
  startTask: () => {},
  endTask: () => {},
});

export function GlobalRunningProvider({ children }: { children: ReactNode }) {
  const [tasks, setTasks] = useState<Set<string>>(new Set());

  const startTask = useCallback((key: string) => {
    setTasks(prev => new Set([...prev, key]));
  }, []);

  const endTask = useCallback((key: string) => {
    setTasks(prev => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  }, []);

  return (
    <RunningContext.Provider value={{ isRunning: tasks.size > 0, startTask, endTask }}>
      {children}
    </RunningContext.Provider>
  );
}

export function useGlobalRunning() {
  return useContext(RunningContext);
}
