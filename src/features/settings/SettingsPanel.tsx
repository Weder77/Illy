import { useCallback, useEffect, useState } from 'react';
import { useAppStore, type SettingsSection } from '../../state/store';
import { IconClose, IconGear, IconImage, IconInfo, IconMail, IconUpdate, IconPalette } from '../../ui/icons';
import { AboutSection } from './AboutSection';
import { AccountsSection } from './AccountsSection';
import { AppearanceSection } from './AppearanceSection';
import { ReadingSection } from './ReadingSection';
import styles from './Settings.module.css';
import { UpdatesSection } from './UpdatesSection';

const SECTIONS: { id: SettingsSection; label: string; Icon: (p: { size?: number }) => React.ReactNode }[] = [
  { id: 'accounts', label: 'Comptes', Icon: IconMail },
  { id: 'appearance', label: 'Apparence', Icon: IconPalette },
  { id: 'reading', label: 'Lecture et rédaction', Icon: IconImage },
  { id: 'updates', label: 'Mises à jour', Icon: IconUpdate },
  { id: 'about', label: 'À propos', Icon: IconInfo },
];

export function SettingsPanel() {
  const section = useAppStore((s) => s.settingsSection);
  const openSettings = useAppStore((s) => s.openSettings);
  const closeSettings = useAppStore((s) => s.closeSettings);
  const [error, setError] = useState<string | null>(null);
  const onError = useCallback((m: string) => setError(m), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useAppStore.getState();
      // L'assistant et la boîte « mot de passe » s'ouvrent par-dessus : Échap est pour eux.
      if (e.key === 'Escape' && !s.showWizard && s.passwordDialogFor == null) closeSettings();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [closeSettings]);

  return (
    <div className={styles.overlay} onMouseDown={(e) => e.target === e.currentTarget && closeSettings()}>
      <div className={styles.panel} role="dialog" aria-modal="true" aria-label="Paramètres">
        <nav className={styles.nav}>
          <div className={styles.navTitle}>
            <IconGear size={16} />
            Paramètres
          </div>
          {SECTIONS.map(({ id, label, Icon }) => (
            <button
              key={id}
              type="button"
              className={id === section ? `${styles.navBtn} ${styles.navOn}` : styles.navBtn}
              aria-current={id === section ? 'page' : undefined}
              onClick={() => {
                setError(null);
                openSettings(id);
              }}
            >
              <Icon size={16} />
              {label}
            </button>
          ))}
        </nav>
        <main className={styles.content}>
          <button type="button" className={styles.close} aria-label="Fermer les paramètres" onClick={closeSettings}>
            <IconClose size={16} />
          </button>
          {error && (
            <p className={styles.errorBanner} role="alert">
              {error}
              <button type="button" className={styles.linkBtn} onClick={() => setError(null)}>
                OK
              </button>
            </p>
          )}
          {section === 'accounts' && <AccountsSection onError={onError} />}
          {section === 'appearance' && <AppearanceSection onError={onError} />}
          {section === 'reading' && <ReadingSection onError={onError} />}
          {section === 'updates' && <UpdatesSection />}
          {section === 'about' && <AboutSection />}
        </main>
      </div>
    </div>
  );
}
