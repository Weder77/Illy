import { useEffect } from 'react';
import { api } from '../../api';
import { useAppStore } from '../../state/store';
import { useAppearance } from '../../ui/appearance';
import { IconClose, IconMinus, IconPen, IconSquare } from '../../ui/icons';
import titleStyles from '../../ui/TitleBar.module.css';
import { Composer } from '../composer/Composer';
import styles from './ComposeWindow.module.css';

/** Fenêtre « Nouveau mail » (`index.html#compose/<id>`) : barre de titre d'Illy et éditeur. */
export function ComposeWindow({ draftId }: { draftId: number }) {
  const loadAccounts = useAppStore((s) => s.loadAccounts);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const settings = useAppStore((s) => s.settings);
  useAppearance(settings);

  useEffect(() => {
    document.title = 'Nouveau mail';
    void loadAccounts().catch(() => undefined);
    void loadSettings().catch(() => undefined);
    // Thème, accent… changés dans la fenêtre principale : appliqués ici aussi.
    return api.onEvent((e) => {
      if (e.type === 'settings_changed') void loadSettings().catch(() => undefined);
      if (e.type === 'accounts_changed' || e.type === 'account_status') void loadAccounts().catch(() => undefined);
    });
  }, [loadAccounts, loadSettings]);

  return (
    <div className={styles.window}>
      <header className={titleStyles.bar} data-tauri-drag-region>
        <div className={titleStyles.brand}>
          <span className={titleStyles.logo}>
            <IconPen size={12} />
          </span>
          <span className={titleStyles.name}>Nouveau mail</span>
        </div>
        <div className={titleStyles.spacer} data-tauri-drag-region />
        <div className={titleStyles.buttons}>
          <button className={titleStyles.btn} aria-label="Réduire" onClick={() => void api.windowAction('minimize')}>
            <IconMinus size={16} />
          </button>
          <button className={titleStyles.btn} aria-label="Agrandir" onClick={() => void api.windowAction('toggleMaximize')}>
            <IconSquare size={14} />
          </button>
          <button
            className={`${titleStyles.btn} ${titleStyles.close}`}
            aria-label="Fermer (le brouillon est gardé)"
            title="Fermer (le brouillon est gardé)"
            onClick={() => void api.windowAction('close')}
          >
            <IconClose size={16} />
          </button>
        </div>
      </header>
      <main className={styles.body}>
        <Composer draftId={draftId} variant="window" onClosed={() => void api.windowAction('close')} />
      </main>
    </div>
  );
}

/** Numéro de brouillon d'une adresse `#compose/<id>`, sinon `null` (fenêtre principale). */
export function composeRoute(hash: string): number | null {
  const m = /^#compose\/(\d+)$/.exec(hash);
  return m ? Number(m[1]) : null;
}
