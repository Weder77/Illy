import { useState } from 'react';
import { api } from '../../api';
import { errorMessage, type Draft } from '../../api/types';
import { useAppStore } from '../../state/store';
import { useNow } from '../../ui/useNow';
import { addressLabel } from '../composer/address';
import styles from './SendBanner.module.css';

function firstRecipient(d: Draft): string {
  const a = d.to[0] ?? d.cc[0] ?? d.bcc[0];
  return a ? addressLabel(a) : '?';
}

/** Bandeau du bas de la fenêtre principale : annulation, envoi en cours, envoyé, échec. */
export function SendBanner() {
  const outbox = useAppStore((s) => s.outbox);
  const recentlySent = useAppStore((s) => s.recentlySent);
  const reopenDraft = useAppStore((s) => s.reopenDraft);
  const [error, setError] = useState<string | null>(null);
  const counting = outbox.some((d) => d.state === 'queued');
  const now = useNow(250, counting);

  const waiting = outbox.filter((d) => d.state === 'queued' && d.sendAt != null && d.sendAt > now);
  const sending = outbox.filter((d) => d.state === 'sending');
  const failed = outbox.filter((d) => d.state === 'failed');

  const cancel = async (d: Draft) => {
    setError(null);
    try {
      if (await api.cancelSend(d.id)) reopenDraft({ ...d, state: 'editing', sendAt: null });
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const retry = (d: Draft) => {
    setError(null);
    api.sendDraft(d.id).catch((e) => setError(errorMessage(e)));
  };

  if (waiting.length + sending.length + failed.length + recentlySent.length === 0 && !error) return null;

  return (
    <div className={styles.stack} aria-live="polite">
      {waiting.map((d) => (
        <div key={d.id} className={styles.banner}>
          <span className={styles.text}>{`Envoi dans ${Math.max(1, (d.sendAt ?? now) - now)} s`}</span>
          <span className={styles.sep}>·</span>
          <button className={styles.action} onClick={() => void cancel(d)}>
            Annuler
          </button>
        </div>
      ))}
      {sending.map((d) => (
        <div key={d.id} className={styles.banner}>
          <span className={styles.text}>Envoi en cours…</span>
        </div>
      ))}
      {recentlySent.map((id) => (
        <div key={`sent-${id}`} className={`${styles.banner} ${styles.ok}`}>
          <span className={styles.text}>Mail envoyé</span>
        </div>
      ))}
      {failed.map((d) => (
        <div key={d.id} className={`${styles.banner} ${styles.failed}`} role="alert">
          <span className={styles.text}>{`Échec de l'envoi à ${firstRecipient(d)} : ${d.lastError ?? 'erreur inconnue'}`}</span>
          <span className={styles.sep}>·</span>
          <button className={styles.action} onClick={() => retry(d)}>
            Réessayer
          </button>
          <span className={styles.sep}>·</span>
          <button className={styles.action} onClick={() => reopenDraft(d)}>
            Modifier
          </button>
        </div>
      ))}
      {error && (
        <div className={`${styles.banner} ${styles.failed}`} role="alert">
          <span className={styles.text}>{error}</span>
          <button className={styles.action} onClick={() => setError(null)}>
            OK
          </button>
        </div>
      )}
    </div>
  );
}
