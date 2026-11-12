import { api } from '../api';
import type { Account } from '../api/types';
import { IconClose, IconMail, IconMinus, IconSquare } from './icons';
import styles from './TitleBar.module.css';

export function TitleBar({ accounts }: { accounts: Account[] }) {
  const offline = accounts.length > 0 && accounts.every((a) => a.status === 'offline');
  return (
    <header className={styles.bar} data-tauri-drag-region>
      <div className={styles.brand}>
        <span className={styles.logo}>
          <IconMail size={13} />
        </span>
        <span className={styles.name}>Illy</span>
        {offline && <span className={styles.offline}>Hors ligne</span>}
      </div>
      <div className={styles.spacer} data-tauri-drag-region />
      <div className={styles.buttons}>
        <button className={styles.btn} aria-label="Réduire" onClick={() => void api.windowAction('minimize')}>
          <IconMinus size={16} />
        </button>
        <button className={styles.btn} aria-label="Agrandir" onClick={() => void api.windowAction('toggleMaximize')}>
          <IconSquare size={14} />
        </button>
        <button className={`${styles.btn} ${styles.close}`} aria-label="Fermer" onClick={() => void api.windowAction('close')}>
          <IconClose size={16} />
        </button>
      </div>
    </header>
  );
}
