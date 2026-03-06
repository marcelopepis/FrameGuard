import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, Copy, X } from 'lucide-react';
import FrameGuardIcon from '../FrameGuardIcon';
import styles from './TitleBar.module.css';

export default function TitleBar() {
  const [maximized, setMaximized] = useState(false);

  // Escuta mudanças de estado da janela (maximize/unmaximize)
  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setMaximized);

    const unlisten = win.onResized(() => {
      win.isMaximized().then(setMaximized);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleMinimize = useCallback(() => invoke('minimize_window'), []);
  const handleMaximize = useCallback(() => {
    invoke('maximize_window');
  }, []);
  const handleClose = useCallback(() => invoke('close_window'), []);

  const handleDragStart = useCallback((e: React.MouseEvent) => {
    // Ignora se clicou em um botão de controle
    if ((e.target as HTMLElement).closest('button')) return;
    getCurrentWindow().startDragging();
  }, []);

  return (
    <div className={styles.titlebar} onMouseDown={handleDragStart}>
      <div className={styles.dragRegion}>
        <FrameGuardIcon size={16} />
        <span className={styles.appName}>FrameGuard</span>
      </div>

      <div className={styles.controls}>
        <button className={styles.controlBtn} onClick={handleMinimize} aria-label="Minimizar">
          <Minus size={14} />
        </button>
        <button className={styles.controlBtn} onClick={handleMaximize} aria-label="Maximizar">
          {maximized ? <Copy size={14} /> : <Square size={14} />}
        </button>
        <button
          className={`${styles.controlBtn} ${styles.closeBtn}`}
          onClick={handleClose}
          aria-label="Fechar"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
