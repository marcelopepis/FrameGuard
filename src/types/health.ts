// Tipos compartilhados entre as páginas de Saúde do Sistema e Limpeza.

import type { LucideProps } from 'lucide-react';

export interface CommandEvent {
  event_type: 'started' | 'stdout' | 'stderr' | 'completed' | 'error';
  data: string;
  timestamp: string;
}

export interface LockingProcessInfo {
  pid: number;
  name: string;
  file_count: number;
}

export interface HealthCheckResult {
  id: string;
  name: string;
  status: 'success' | 'warning' | 'error';
  message: string;
  details: string;
  duration_seconds: number;
  space_freed_mb: number | null;
  timestamp: string;
  locking_processes?: LockingProcessInfo[];
}

export interface LogLine {
  type: string;
  text: string;
}

export interface ActionState {
  running: boolean;
  log: LogLine[];
  progress: number | null;
  lastResult: HealthCheckResult | null;
  showLog: boolean;
  showDetails: boolean;
}

export interface ActionMeta {
  id: string;
  name: string;
  Icon: React.ComponentType<LucideProps>;
  description: string;
  technicalDetails: string;
  estimatedDuration: string;
  eventChannel: string;
  command: string;
  invokeArgs?: Record<string, unknown>;
  requiresInternet?: boolean;
  requiresRestart?: boolean;
  category: string;
}

export interface Section {
  id: string;
  title: string;
  subtitle: string;
}
