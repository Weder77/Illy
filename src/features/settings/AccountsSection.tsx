import { useState, type DragEvent, type KeyboardEvent } from 'react';
import { api } from '../../api';
import { errorMessage, type Account } from '../../api/types';
import { useAppStore } from '../../state/store';
import { IconPlus } from '../../ui/icons';
import { AccountDetails } from './AccountDetails';
import { Section } from './controls';
import styles from './Settings.module.css';

function moved(ids: number[], from: number, to: number): number[] {
  const next = [...ids];
  const [x] = next.splice(from, 1);
  next.splice(to, 0, x);
  return next;
}

export function AccountsSection({ onError }: { onError(message: string): void }) {
  const accounts = useAppStore((s) => s.accounts);
  const setShowWizard = useAppStore((s) => s.setShowWizard);
  const [selectedId, setSelectedId] = useState<number | null>(accounts[0]?.id ?? null);
  const [dragging, setDragging] = useState<number | null>(null);
  const selected = accounts.find((a) => a.id === selectedId) ?? accounts[0];

  const reorder = (ids: number[]) => {
    if (ids.join() === accounts.map((a) => a.id).join()) return;
    const byId = new Map(accounts.map((a) => [a.id, a]));
    useAppStore.setState({ accounts: ids.map((id) => byId.get(id)!) });
    api
      .reorderAccounts(ids)
      .then((list) => useAppStore.setState({ accounts: list }))
      .catch((e) => {
        onError(errorMessage(e));
        void useAppStore.getState().loadAccounts();
      });
  };

  const onDrop = (e: DragEvent, target: number) => {
    e.preventDefault();
    if (dragging == null) return;
    const ids = accounts.map((a) => a.id);
    reorder(moved(ids, ids.indexOf(dragging), target));
    setDragging(null);
  };

  const onKey = (e: KeyboardEvent, index: number) => {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    e.preventDefault();
    const to = index + (e.key === 'ArrowDown' ? 1 : -1);
    if (to < 0 || to >= accounts.length) return;
    reorder(moved(accounts.map((a) => a.id), index, to));
  };

  return (
    <Section title="Comptes">
      <div className={styles.accounts}>
        <div className={styles.accountList}>
          <ul aria-label="Comptes" className={styles.accountUl}>
            {accounts.map((a: Account, i) => (
              <li
                key={a.id}
                draggable
                className={[styles.accountItem, a.id === selected?.id ? styles.accountItemOn : '', dragging === a.id ? styles.dragging : ''].join(' ')}
                onDragStart={(e) => {
                  setDragging(a.id);
                  e.dataTransfer?.setData('text/plain', String(a.id));
                }}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => onDrop(e, i)}
                onDragEnd={() => setDragging(null)}
              >
                <button
                  type="button"
                  className={styles.accountBtn}
                  onClick={() => setSelectedId(a.id)}
                  onKeyDown={(e) => onKey(e, i)}
                  title="Glisse pour changer l'ordre (ou Alt + ↑ / ↓)"
                >
                  <span className={styles.grip} aria-hidden="true">⋮⋮</span>
                  <span className={styles.dot} style={{ background: a.color }} aria-hidden="true" />
                  <span className={styles.accountEmail}>{a.email}</span>
                </button>
              </li>
            ))}
          </ul>
          <button type="button" className={styles.secondaryBtn} onClick={() => setShowWizard(true)}>
            <IconPlus size={15} />
            Ajouter un compte
          </button>
          <p className={styles.muted}>L'ordre s'applique au panneau de gauche.</p>
        </div>
        {selected ? <AccountDetails key={selected.id} account={selected} onError={onError} /> : <p className={styles.muted}>Aucun compte.</p>}
      </div>
    </Section>
  );
}
