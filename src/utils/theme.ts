/**
 * Gerenciamento de tema (dark/light) do FrameGuard.
 *
 * Os tokens CSS são definidos em `src/styles/tokens.css` usando
 * seletores `[data-theme="dark"]` e `[data-theme="light"]`.
 * Este módulo controla qual tema está ativo e persiste a escolha
 * em `localStorage`.
 */

/** Temas suportados pelo FrameGuard. */
export type Theme = 'dark' | 'light';

const STORAGE_KEY = 'fg-theme';
const DEFAULT_THEME: Theme = 'dark';

/**
 * Aplica o tema ao documento e persiste a escolha em `localStorage`.
 *
 * @param theme - Tema a ser aplicado (`'dark'` ou `'light'`).
 */
export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem(STORAGE_KEY, theme);
}

/**
 * Retorna o tema salvo em `localStorage`, ou `'dark'` como padrão.
 *
 * @returns O tema armazenado, ou `'dark'` se nenhum valor válido existir.
 */
export function getStoredTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'dark' || stored === 'light') return stored;
  return DEFAULT_THEME;
}

/**
 * Inicializa o tema no bootstrap do app (antes do primeiro render).
 *
 * Deve ser chamado em `main.tsx` antes de `ReactDOM.createRoot` para
 * evitar flash de tema incorreto.
 */
export function initTheme(): void {
  applyTheme(getStoredTheme());
}
