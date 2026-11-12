import { api } from '../api';
import type { Account, Draft } from '../api/types';
import { IconClose, IconMail, IconMinus, IconSquare } from './icons';
import styles from './TitleBar.module.css';
import { useNow } from './useNow';

export function TitleBar({ accounts, outbox = [] }: { accounts: Account[]; outbox?: Draft[] }) {
  const offline = accounts.length > 0 && accounts.every((a) => a.status === 'offline');
  const now = useNow(5000, outbox.length > 0);
  // Envois sortis du délai d'annulation mais pas encore partis (réseau coupé, nouvelle tentative…).
  const pending = outbox.filter((d) => d.state === 'queued' && d.sendAt != null && d.sendAt <= now).length;
  return (
    <header className={styles.bar} data-tauri-drag-region>
      <div className={styles.brand}>
        <span className={styles.logo}>
          <IconMail size={13} />
        </span>
        <span className={styles.name}>Illy</span>
        {offline && <span className={styles.offline}>Hors ligne</span>}
        {pending > 0 && <span className={styles.offline}>{`${pending} envoi${pending > 1 ? 's' : ''} en attente`}</span>}
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
