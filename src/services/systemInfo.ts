import { invoke } from '@tauri-apps/api/core';

// Espelha SystemSummary do Rust (campos em snake_case conforme serde)
export interface SystemSummary {
  os_version: string;
  hostname: string;
  is_elevated: boolean;
}

// Espelha StaticHwInfo do Rust — CPU e RAM (rápido, <100ms)
export interface StaticHwInfo {
  cpu_name: string;
  cpu_cores: number;
  ram_total_gb: number;
}

// Espelha GpuInfo do Rust — dados de GPU (lento, 2-4s, pre-warmed no setup)
export interface GpuInfo {
  gpu_name: string;
  gpu_vram_gb: number;
}

// Espelha SystemStatus do Rust — lido fresco do registro a cada chamada
export interface SystemStatus {
  game_mode_enabled: boolean;
  hags_enabled: boolean;
  vbs_enabled: boolean;
  game_dvr_status: string;
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

/** Hardware estático — CPU e RAM (rápido, <100ms). Cacheado no backend. */
export async function getStaticHwInfo(): Promise<StaticHwInfo> {
  return invoke<StaticHwInfo>('get_static_hw_info');
}

/** GPU info (nome + VRAM). Pre-warmed no setup — pode já estar pronto quando Dashboard carrega. */
export async function getGpuInfo(): Promise<GpuInfo> {
  return invoke<GpuInfo>('get_gpu_info');
}

/** Status de configurações do Windows (Game Mode, HAGS, VBS, DVR, Power Plan, Timer). Sempre fresco. */
export async function getSystemStatus(): Promise<SystemStatus> {
  return invoke<SystemStatus>('get_system_status');
}

/** Retorna uso atual de CPU e RAM (para polling periódico). */
export async function getSystemUsage(): Promise<SystemUsage> {
  return invoke<SystemUsage>('get_system_usage');
}

// Espelha DetectedVendors do Rust — vendors de GPU e CPU detectados
export interface DetectedVendors {
  gpu_vendor: string;
  cpu_vendor: string;
}

/** Vendors de hardware detectados. Usa caches do backend (instantâneo se pre-warmed). */
export async function getDetectedVendors(): Promise<DetectedVendors> {
  return invoke<DetectedVendors>('get_detected_vendors');
}
