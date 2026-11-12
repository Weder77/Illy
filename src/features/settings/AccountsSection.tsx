import { useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
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
  // Glisser au pointeur : le glisser-déposer HTML5 n'arrive pas jusqu'à la page sous Windows
  // (Tauri garde le dépôt de fichiers pour les pièces jointes).
  const drag = useRef<{ id: number; startY: number; active: boolean; over: number } | null>(null);
  const items = useRef<(HTMLLIElement | null)[]>([]);
  const suppressClick = useRef(false);
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

  const indexAt = (y: number) => {
    const rects = items.current.map((el) => el?.getBoundingClientRect());
    const i = rects.findIndex((r) => r && y < r.top + r.height / 2);
    return i < 0 ? accounts.length - 1 : i;
  };

  const onPointerDown = (e: PointerEvent, id: number) => {
    if (e.button !== 0) return;
    drag.current = { id, startY: e.clientY, active: false, over: accounts.findIndex((a) => a.id === id) };
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  };

  const onPointerMove = (e: PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    if (!d.active && Math.abs(e.clientY - d.startY) < 4) return;
    d.active = true;
    d.over = indexAt(e.clientY);
    setDragging(d.id);
  };

  const onPointerUp = () => {
    const d = drag.current;
    drag.current = null;
    setDragging(null);
    if (!d?.active) return;
    // Le clic qui suit la fin du glisser ne doit pas sélectionner ; s'il ne vient pas, on oublie.
    suppressClick.current = true;
    setTimeout(() => (suppressClick.current = false), 0);
    const ids = accounts.map((a) => a.id);
    reorder(moved(ids, ids.indexOf(d.id), d.over));
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
                ref={(el) => {
                  items.current[i] = el;
                }}
                className={[styles.accountItem, a.id === selected?.id ? styles.accountItemOn : '', dragging === a.id ? styles.dragging : ''].join(' ')}
                onPointerDown={(e) => onPointerDown(e, a.id)}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={() => {
                  drag.current = null;
                  setDragging(null);
                }}
              >
                <button
                  type="button"
                  className={styles.accountBtn}
                  onClick={() => {
                    if (suppressClick.current) {
                      suppressClick.current = false;
                      return;
                    }
                    setSelectedId(a.id);
                  }}
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
