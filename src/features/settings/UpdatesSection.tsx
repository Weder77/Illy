import { useEffect, useState } from 'react';
import { api } from '../../api';
import { useAppStore, type UpdateState } from '../../state/store';
import { Row, Section } from './controls';
import styles from './Settings.module.css';

export function updateStatusText(u: UpdateState): string {
  switch (u.status) {
    case 'idle':
      return '';
    case 'checking':
      return 'Recherche…';
    case 'none':
      return 'Illy est à jour.';
    case 'available':
      return `Illy ${u.version} est disponible.`;
    case 'downloading':
      return u.progress == null ? 'Téléchargement…' : `Téléchargement… ${u.progress} %`;
    case 'error':
      return u.version ? `La mise à jour n'a pas pu être installée : ${u.message}` : `Impossible de vérifier les mises à jour : ${u.message}`;
  }
}

export function UpdatesSection() {
  const update = useAppStore((s) => s.update);
  const checkForUpdate = useAppStore((s) => s.checkForUpdate);
  const installUpdate = useAppStore((s) => s.installUpdate);
  const [version, setVersion] = useState('');

  useEffect(() => {
    api
      .appInfo()
      .then((i) => setVersion(i.version))
      .catch(() => undefined);
  }, []);

  const canInstall = update.status === 'available' || (update.status === 'error' && !!update.version);
  return (
    <Section title="Mises à jour">
      <Row label="Version installée">
        <span className={styles.value}>{version}</span>
      </Row>
      <p className={styles.muted}>
        Illy vérifie au démarrage si une nouvelle version est publiée sur GitHub. Rien ne s'installe sans ton accord.
      </p>
      <div className={styles.inline}>
        <button
          type="button"
          className={styles.secondaryBtn}
          onClick={() => void checkForUpdate()}
          disabled={update.status === 'checking' || update.status === 'downloading'}
        >
          Rechercher des mises à jour
        </button>
        {canInstall && (
          <button type="button" className={styles.primaryBtn} onClick={() => void installUpdate()}>
            Installer
          </button>
        )}
      </div>
      <p className={update.status === 'error' ? styles.errorText : styles.value} aria-live="polite">
        {updateStatusText(update)}
      </p>
    </Section>
  );
}
