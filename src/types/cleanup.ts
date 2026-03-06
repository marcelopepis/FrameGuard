// Tipos para a página de Limpeza — espelham structs de cleanup.rs

import type { LockingProcessInfo } from './health';

export type CleanupRisk = 'safe' | 'moderate' | 'caution';

export interface CleanupItem {
  id: string;
  name: string;
  path_display: string;
  size_bytes: number;
  file_count: number;
  default_selected: boolean;
}

export interface CleanupCategory {
  id: string;
  name: string;
  description: string;
  risk: CleanupRisk;
  default_selected: boolean;
  items: CleanupItem[];
  total_size_bytes: number;
  total_file_count: number;
}

export interface CleanupScanResult {
  categories: CleanupCategory[];
  total_size_bytes: number;
  total_file_count: number;
  scan_duration_seconds: number;
}

export interface CleanupProgressEvent {
  current_category: string;
  current_item: string;
  progress_percent: number;
  freed_bytes_so_far: number;
  message: string;
}

export interface CleanupItemResult {
  id: string;
  name: string;
  freed_bytes: number;
  files_removed: number;
  files_skipped: number;
  errors: string[];
}

export interface BrowserCleanOptions {
  cache: boolean;
  cookies: boolean;
  history: boolean;
  sessions: boolean;
}

export interface CleanupResult {
  total_freed_bytes: number;
  total_files_removed: number;
  total_files_skipped: number;
  duration_seconds: number;
  item_results: CleanupItemResult[];
  locking_processes: LockingProcessInfo[];
}
