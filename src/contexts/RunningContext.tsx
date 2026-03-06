// Contexto global de execução do FrameGuard.
//
// Rastreia quais chaves de tarefa estão ativas em qualquer parte do app.
// Usado para desabilitar botões "Executar" em todas as páginas enquanto
// um comando de longa duração (DISM, SFC, etc.) estiver em andamento.
//
// Cada tarefa pode opcionalmente estar associada a uma página (rota).
// `busyPages` expõe quais páginas têm tarefas ativas, permitindo que
// App.tsx mantenha essas páginas montadas (ocultas) ao invés de desmontá-las.

import { createContext, useContext, useState, useCallback, useMemo, ReactNode } from 'react';

interface RunningCtx {
  /** `true` se ao menos uma tarefa estiver em execução em qualquer página */
  isRunning: boolean;
  /** Conjunto de rotas que possuem tarefas ativas (ex: '/cleanup') */
  busyPages: ReadonlySet<string>;
  /** Marca uma tarefa como iniciada; key deve ser única por execução */
  startTask: (key: string, pagePath?: string) => void;
  /** Marca uma tarefa como finalizada */
  endTask: (key: string) => void;
}

const RunningContext = createContext<RunningCtx>({
  isRunning: false,
  busyPages: new Set(),
  startTask: () => {},
  endTask: () => {},
});

export function GlobalRunningProvider({ children }: { children: ReactNode }) {
  // Map<taskKey, pagePath | undefined>
  const [tasks, setTasks] = useState<Map<string, string | undefined>>(() => new Map());

  const startTask = useCallback((key: string, pagePath?: string) => {
    setTasks((prev) => {
      const next = new Map(prev);
      next.set(key, pagePath);
      return next;
    });
  }, []);

  const endTask = useCallback((key: string) => {
    setTasks((prev) => {
      const next = new Map(prev);
      next.delete(key);
      return next;
    });
  }, []);

  const isRunning = tasks.size > 0;

  const busyPages = useMemo(() => {
    const pages = new Set<string>();
    for (const page of tasks.values()) {
      if (page) pages.add(page);
    }
    return pages;
  }, [tasks]);

  return (
    <RunningContext.Provider value={{ isRunning, busyPages, startTask, endTask }}>
      {children}
    </RunningContext.Provider>
  );
}

export function useGlobalRunning() {
  return useContext(RunningContext);
}
