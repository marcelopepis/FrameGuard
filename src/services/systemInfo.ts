import { invoke } from '@tauri-apps/api/core';

// Espelha SystemSummary do Rust (campos em snake_case conforme serde)
export interface SystemSummary {
  os_version: string;
  hostname: string;
  is_elevated: boolean;
}

// Espelha SystemInfo do Rust
export interface SystemInfo {
  cpu_name: string;
  cpu_cores: number;
  cpu_usage_percent: number;
  ram_total_gb: number;
  ram_used_gb: number;
  ram_usage_percent: number;
  gpu_name: string;
  gpu_vram_gb: number;
  game_mode_enabled: boolean;
  hags_enabled: boolean;
  vbs_enabled: boolean;
  game_dvr_disabled: boolean;
  power_plan_name: string;
  power_plan_tier: string;
  timer_resolution_optimized: boolean;
}

// Espelha SystemUsage do Rust
export interface SystemUsage {
  cpu_usage_percent: number;
  ram_usage_percent: number;
}

export async function getSystemSummary(): Promise<SystemSummary> {
  return invoke<SystemSummary>('get_system_summary');
}

/** Coleta informações completas de hardware e status (uso CPU+RAM com delta de 200 ms). */
export async function getSystemInfo(): Promise<SystemInfo> {
  return invoke<SystemInfo>('get_system_info');
}

/** Retorna uso atual de CPU e RAM (para polling periódico). */
export async function getSystemUsage(): Promise<SystemUsage> {
  return invoke<SystemUsage>('get_system_usage');
}
