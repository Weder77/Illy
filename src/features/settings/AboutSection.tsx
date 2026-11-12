import { useCallback, useEffect, useState } from 'react';
import { api } from '../../api';
import { errorMessage, type AppInfo, type StorageUsage } from '../../api/types';
import { useAppStore } from '../../state/store';
import { formatSize } from '../reader/Attachments';
import { Row, Section } from './controls';
import styles from './Settings.module.css';

export function AboutSection() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const refresh = useCallback(() => {
    api.storageUsage().then(setUsage).catch(() => undefined);
  }, []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => undefined);
    refresh();
  }, [refresh]);

  const clear = async () => {
    if (!window.confirm('Vider le cache local ? Les mails seront retéléchargés depuis les serveurs. Tes comptes, réglages et brouillons sont gardés.')) return;
    setBusy(true);
    setMessage(null);
    try {
      await api.clearCache();
      setMessage({ ok: true, text: 'Cache vidé : les mails se retéléchargent.' });
      refresh();
      await useAppStore.getState().init();
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section title="À propos">
      <Row label="Illy">
        <span className={styles.value}>{info?.version ?? ''}</span>
      </Row>
      <Row label="Journaux" hint="Ils ne contiennent ni le contenu ni l'objet de tes mails.">
        <button type="button" className={styles.secondaryBtn} onClick={() => void api.openLogsFolder().catch((e) => setMessage({ ok: false, text: errorMessage(e) }))}>
          Ouvrir le dossier des journaux
        </button>
      </Row>
      <div className={styles.block}>
        <div className={styles.rowLabel}>Espace utilisé</div>
        {usage && (
          <dl className={styles.usage}>
            <dt>Mails et réglages</dt>
            <dd>{formatSize(usage.database)}</dd>
            <dt>Brouillons</dt>
            <dd>{formatSize(usage.drafts)}</dd>
            <dt>Fichiers temporaires</dt>
            <dd>{formatSize(usage.temporary)}</dd>
          </dl>
        )}
      </div>
      <Row label="Vider le cache local" hint="Les mails sont retéléchargés. Comptes, mots de passe, réglages et brouillons sont gardés.">
        <button type="button" className={styles.secondaryBtn} onClick={() => void clear()} disabled={busy}>
          Vider le cache local
        </button>
      </Row>
      {message && (
        <p className={message.ok ? styles.ok : styles.errorText} role={message.ok ? 'status' : 'alert'}>
          {message.text}
        </p>
      )}
    </Section>
  );
}
