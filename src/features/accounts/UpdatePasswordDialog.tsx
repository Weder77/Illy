import { useState, type FormEvent } from 'react';
import { api } from '../../api';
import { useAppStore } from '../../state/store';
import { accountErrorText } from './errors';
import styles from './Accounts.module.css';

export function UpdatePasswordDialog({ accountId, onClose }: { accountId: number; onClose: () => void }) {
  const account = useAppStore((s) => s.accounts.find((a) => a.id === accountId));
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ text: string; detail: string | null } | null>(null);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!password) return;
    setBusy(true);
    setError(null);
    try {
      await api.updatePassword(accountId, password);
      onClose();
    } catch (err) {
      setError(accountErrorText(err));
    } finally {
      setBusy(false);
    }
  };

  const isAppPassword = account?.provider === 'gmail' || account?.provider === 'yahoo';

  return (
    <div className={styles.overlay} role="dialog" aria-modal="true" aria-labelledby="pw-title">
      <form className={styles.card} onSubmit={save}>
        <h2 id="pw-title" className={styles.title}>
          Mettre à jour le mot de passe de {account?.email ?? 'ce compte'}
        </h2>
        <p className={styles.lead}>
          {isAppPassword
            ? "Crée un nouveau mot de passe d'application chez ton fournisseur et colle-le ici."
            : 'Saisis le nouveau mot de passe de cette adresse.'}
        </p>
        <label className={styles.field}>
          Nouveau mot de passe
          <input className={styles.input} type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoFocus />
        </label>
        {error && (
          <div className={styles.error} role="alert">
            {error.text}
            {error.detail && <span className={styles.errorDetail}>{error.detail}</span>}
          </div>
        )}
        <div className={styles.actions}>
          <button type="button" className={styles.secondary} onClick={onClose}>
            Annuler
          </button>
          <button type="submit" className={styles.primary} disabled={!password || busy}>
            {busy ? 'Vérification…' : 'Enregistrer'}
          </button>
        </div>
      </form>
    </div>
  );
}
