import { invoke } from '@tauri-apps/api/core';

// Espelha o struct SystemSummary do Rust (campos em snake_case conforme serde)
export interface SystemSummary {
  os_version: string;
  hostname: string;
  is_elevated: boolean;
}

/// Chama o comando Rust `get_system_summary` e retorna os dados do sistema.
export async function getSystemSummary(): Promise<SystemSummary> {
  return invoke<SystemSummary>('get_system_summary');
}
