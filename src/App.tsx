import { useEffect } from 'react';
import { api } from './api';
import { useAppStore } from './state/store';
import { MessageList } from './features/message-list/MessageList';
import { Sidebar } from './features/sidebar/Sidebar';
import { TitleBar } from './ui/TitleBar';
import styles from './App.module.css';

export default function App() {
  const accounts = useAppStore((s) => s.accounts);
  const init = useAppStore((s) => s.init);
  const applyEvent = useAppStore((s) => s.applyEvent);

  useEffect(() => {
    void init().catch((e) => console.error('démarrage', e));
    return api.onEvent(applyEvent);
  }, [init, applyEvent]);

  return (
    <div className={styles.window} data-testid="app-root">
      <TitleBar accounts={accounts} />
      <main className={styles.panels}>
        <section className={styles.panel} data-testid="panel-sidebar" aria-label="Boîtes">
          <Sidebar />
        </section>
        <section className={styles.panel} data-testid="panel-list" aria-label="Liste">
          <MessageList />
        </section>
        <section className={styles.panel} data-testid="panel-reader" aria-label="Lecture" />
      </main>
    </div>
  );
}
