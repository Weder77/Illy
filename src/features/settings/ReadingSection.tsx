import { useEffect, useState } from 'react';
import { api } from '../../api';
import { errorMessage, type RemoteImages } from '../../api/types';
import { useAppStore } from '../../state/store';
import { Choice, Row, Section } from './controls';
import styles from './Settings.module.css';

export function ReadingSection({ onError }: { onError(message: string): void }) {
  const settings = useAppStore((s) => s.settings);
  const setSetting = useAppStore((s) => s.setSetting);
  const [senders, setSenders] = useState<string[] | null>(null);

  useEffect(() => {
    api
      .listImageAllowlist()
      .then(setSenders)
      .catch((e) => onError(errorMessage(e)));
  }, [onError]);

  const save = (key: Parameters<typeof setSetting>[0], value: unknown) => void setSetting(key, value).catch((e) => onError(errorMessage(e)));

  const remove = (sender: string) => {
    api
      .removeFromImageAllowlist(sender)
      .then(() => setSenders((list) => (list ?? []).filter((x) => x !== sender)))
      .catch((e) => onError(errorMessage(e)));
  };

  return (
    <Section title="Lecture et rédaction">
      <Row label="Images distantes" hint="Bloquer les images protège ta vie privée : l'expéditeur ne sait pas quand tu ouvres son mail.">
        <Choice<RemoteImages>
          label="Images distantes"
          value={settings.remoteImages}
          options={[
            { value: 'block', label: 'Bloquées (recommandé)' },
            { value: 'always', label: 'Toujours afficher' },
          ]}
          onChange={(v) => save('remote_images', v)}
        />
      </Row>
      <div className={styles.block}>
        <div className={styles.rowLabel}>Expéditeurs dont les images s'affichent toujours</div>
        {senders === null ? null : senders.length === 0 ? (
          <p className={styles.muted}>Aucun pour l'instant (bouton « Toujours pour cet expéditeur » d'un mail).</p>
        ) : (
          <ul className={styles.senders}>
            {senders.map((s) => (
              <li key={s} className={styles.sender}>
                <span>{s}</span>
                <button type="button" className={styles.linkBtn} aria-label={`Retirer ${s}`} onClick={() => remove(s)}>
                  Retirer
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <Row label="Annulation d'envoi" hint="Le temps pendant lequel « Annuler » reste possible après « Envoyer ».">
        <Choice<number>
          label="Annulation d'envoi"
          value={settings.undoSendSeconds}
          options={[5, 10, 30].map((n) => ({ value: n, label: `${n} s` }))}
          onChange={(v) => save('undo_send_seconds', v)}
        />
      </Row>
    </Section>
  );
}
