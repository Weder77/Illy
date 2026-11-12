import { useEffect } from 'react';
import { api } from './api';
import { useAppStore } from './state/store';
import { AddAccountWizard } from './features/accounts/AddAccountWizard';
import { UpdatePasswordDialog } from './features/accounts/UpdatePasswordDialog';
import { MessageList } from './features/message-list/MessageList';
import { Reader } from './features/reader/Reader';
import { SendBanner } from './features/outbox/SendBanner';
import { Sidebar } from './features/sidebar/Sidebar';
import { TitleBar } from './ui/TitleBar';
import styles from './App.module.css';

export default function App() {
  const accounts = useAppStore((s) => s.accounts);
  const init = useAppStore((s) => s.init);
  const applyEvent = useAppStore((s) => s.applyEvent);
  const showWizard = useAppStore((s) => s.showWizard);
  const setShowWizard = useAppStore((s) => s.setShowWizard);
  const passwordDialogFor = useAppStore((s) => s.passwordDialogFor);
  const setPasswordDialogFor = useAppStore((s) => s.setPasswordDialogFor);
  const outbox = useAppStore((s) => s.outbox);

  useEffect(() => {
    // Écoute d'abord : un statut émis pendant le chargement ne doit pas être perdu.
    const off = api.onEvent(applyEvent);
    void init().catch((e) => console.error('démarrage', e));
    return off;
  }, [init, applyEvent]);

  return (
    <div className={styles.window} data-testid="app-root">
      <TitleBar accounts={accounts} outbox={outbox} />
      <main className={styles.panels}>
        <section className={styles.panel} data-testid="panel-sidebar" aria-label="Boîtes">
          <Sidebar />
        </section>
        <section className={styles.panel} data-testid="panel-list" aria-label="Liste">
          <MessageList />
        </section>
        <section className={styles.panel} data-testid="panel-reader" aria-label="Lecture">
          <Reader />
        </section>
      </main>
      <SendBanner />
      {showWizard && <AddAccountWizard onClose={() => setShowWizard(false)} />}
      {passwordDialogFor != null && (
        <UpdatePasswordDialog accountId={passwordDialogFor} onClose={() => setPasswordDialogFor(null)} />
      )}
    </div>
  );
}
