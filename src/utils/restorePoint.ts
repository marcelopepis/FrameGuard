// Utilitário para criação de pontos de restauração do Windows.
//
// Centraliza a lógica de verificação de preferência do usuário e invocação
// do comando Tauri, para uso compartilhado entre Optimizations, Privacy e Plans.

import { invoke } from '@tauri-apps/api/core';

interface RestorePointResponse {
  status: 'created' | 'skipped' | 'disabled' | 'failed';
  message: string;
}

/** Verifica se o usuário habilitou a criação automática de restore points. */
export function isRestorePointEnabled(): boolean {
  return localStorage.getItem('fg.restorePoint') !== 'false';
}

/**
 * Tenta criar um ponto de restauração antes de aplicar um tweak.
 *
 * Retorna o resultado para que o chamador possa exibir toast adequado.
 * Retorna `null` se a preferência estiver desabilitada.
 */
export async function ensureRestorePoint(
  description: string,
): Promise<RestorePointResponse | null> {
  if (!isRestorePointEnabled()) return null;

  try {
    return await invoke<RestorePointResponse>('create_restore_point', { description });
  } catch (e) {
    return { status: 'failed', message: String(e) };
  }
}

/**
 * Exibe toast apropriado baseado no resultado do restore point.
 *
 * @param result - Resultado de `ensureRestorePoint`
 * @param showToast - Função de toast do contexto
 */
export function showRestorePointToast(
  result: RestorePointResponse | null,
  showToast: (
    type: 'success' | 'warning' | 'error' | 'info',
    title: string,
    message?: string,
    duration?: number,
  ) => void,
) {
  if (!result) return;

  switch (result.status) {
    case 'created':
      showToast('success', 'Ponto de restauração criado', undefined, 3000);
      break;
    case 'skipped':
      // Silencioso — já existe restore point recente
      break;
    case 'disabled':
      showToast('warning', 'Proteção do Sistema desabilitada', result.message, 0);
      break;
    case 'failed':
      showToast('warning', 'Falha ao criar ponto de restauração', result.message);
      break;
  }
}
