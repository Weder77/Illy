import { useEffect } from 'react';
import { useAppStore } from '../../state/store';
import { IconUpdate } from '../../ui/icons';
import styles from './UpdateBanner.module.css';

const STARTUP_CHECK_DELAY_MS = 10_000;

/** Bandeau en haut de la fenêtre principale. Rien ne s'installe sans un clic sur « Installer ». */
export function UpdateBanner() {
  const update = useAppStore((s) => s.update);
  const dismissed = useAppStore((s) => s.updateDismissed);
  const checkForUpdate = useAppStore((s) => s.checkForUpdate);
  const installUpdate = useAppStore((s) => s.installUpdate);
  const dismissUpdate = useAppStore((s) => s.dismissUpdate);

  useEffect(() => {
    const t = setTimeout(() => void checkForUpdate(), STARTUP_CHECK_DELAY_MS);
    return () => clearTimeout(t);
  }, [checkForUpdate]);

  if (update.status === 'installing') {
    return (
      <div className={styles.banner} role="status">
        <IconUpdate size={15} />
        <span className={styles.text}>Installation de la mise à jour… Illy va redémarrer.</span>
      </div>
    );
  }
  if (update.status === 'downloading') {
    return (
      <div className={styles.banner} role="status">
        <IconUpdate size={15} />
        <span className={styles.text}>
          {update.progress == null ? 'Téléchargement de la mise à jour…' : `Téléchargement de la mise à jour… ${update.progress} %`}
        </span>
        {update.progress != null && (
          <span className={styles.bar} aria-hidden="true">
            <span style={{ width: `${update.progress}%` }} />
          </span>
        )}
      </div>
    );
  }
  if (dismissed) return null;
  if (update.status === 'available') {
    return (
      <div className={styles.banner} role="status">
        <IconUpdate size={15} />
        <span className={styles.text}>{`Illy ${update.version} est disponible`}</span>
        <span className={styles.sep}>·</span>
        <button className={styles.action} onClick={() => void installUpdate()}>
          Installer
        </button>
        <span className={styles.sep}>·</span>
        <button className={styles.secondary} onClick={dismissUpdate}>
          Plus tard
        </button>
      </div>
    );
  }
  // Échec d'installation seulement : un échec de vérification au démarrage (hors ligne) reste silencieux.
  if (update.status === 'error' && update.version) {
    return (
      <div className={`${styles.banner} ${styles.error}`} role="alert">
        <span className={styles.text}>{`La mise à jour n'a pas pu être installée : ${update.message}`}</span>
        <span className={styles.sep}>·</span>
        <button className={styles.action} onClick={() => void installUpdate()}>
          Réessayer
        </button>
        <span className={styles.sep}>·</span>
        <button className={styles.secondary} onClick={dismissUpdate}>
          Plus tard
        </button>
      </div>
    );
  }
  return null;
}
