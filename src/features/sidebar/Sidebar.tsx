import { useState, type ReactNode } from 'react';
import type { Folder, FolderRole, ListScope } from '../../api/types';
import { useAppStore } from '../../state/store';
import {
  IconChevron,
  IconDraft,
  IconFolder,
  IconGear,
  IconInbox,
  IconLayers,
  IconPen,
  IconSend,
  IconSpam,
  IconTrash,
  IconWarn,
} from '../../ui/icons';
import styles from './Sidebar.module.css';

const ROLE_ICON: Record<FolderRole, (p: { size?: number }) => ReactNode> = {
  inbox: IconInbox,
  drafts: IconDraft,
  sent: IconSend,
  spam: IconSpam,
  trash: IconTrash,
  custom: IconFolder,
};

const STORAGE_KEY = 'illy.collapsed';

function readCollapsed(): number[] {
  try {
    const v = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    return Array.isArray(v) ? v.filter((x) => typeof x === 'number') : [];
  } catch {
    return [];
  }
}

function writeCollapsed(ids: number[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
  } catch {
    /* stockage indisponible : l'état reste en mémoire */
  }
}

function Entry(props: {
  label: string;
  count: number;
  active: boolean;
  icon: ReactNode;
  top?: boolean;
  onClick: () => void;
}) {
  const cls = [styles.item, props.top ? styles.top : '', props.active ? styles.active : ''].join(' ');
  return (
    <button className={cls} aria-current={props.active ? 'true' : undefined} onClick={props.onClick}>
      <span className={styles.icon}>{props.icon}</span>
      <span className={styles.label}>{props.label}</span>
      {props.count > 0 && <span className={styles.pill}>{props.count}</span>}
    </button>
  );
}

const isActive = (scope: ListScope, f: Folder) => scope.kind === 'folder' && scope.folderId === f.id;

export function Sidebar() {
  const accounts = useAppStore((s) => s.accounts);
  const folders = useAppStore((s) => s.folders);
  const scope = useAppStore((s) => s.selection.scope);
  const selectScope = useAppStore((s) => s.selectScope);
  const setShowWizard = useAppStore((s) => s.setShowWizard);
  const composeNew = useAppStore((s) => s.composeNew);
  const setPasswordDialogFor = useAppStore((s) => s.setPasswordDialogFor);
  const [collapsed, setCollapsed] = useState<number[]>(readCollapsed);
  const [problemFor, setProblemFor] = useState<number | null>(null);

  const unifiedUnread = accounts.reduce(
    (sum, a) => sum + (folders[a.id]?.filter((f) => f.role === 'inbox').reduce((n, f) => n + f.unreadCount, 0) ?? 0),
    0,
  );

  const toggle = (id: number) => {
    const next = collapsed.includes(id) ? collapsed.filter((x) => x !== id) : [...collapsed, id];
    setCollapsed(next);
    writeCollapsed(next);
  };

  return (
    <nav className={styles.sidebar} aria-label="Boîtes mail">
      <div className={styles.scroll}>
        <Entry
          top
          label="Toutes les boîtes de réception"
          count={unifiedUnread}
          active={scope.kind === 'unified_inbox'}
          icon={<IconLayers size={16} />}
          onClick={() => selectScope({ kind: 'unified_inbox' })}
        />
        <div className={styles.sep} />
        <div className={styles.section}>Comptes</div>
        {accounts.map((a) => {
          const isCollapsed = collapsed.includes(a.id);
          return (
            <div key={a.id} className={`${styles.account} ${isCollapsed ? styles.collapsed : ''}`}>
              <div className={styles.accountHeader}>
                <button
                  className={styles.toggle}
                  aria-expanded={!isCollapsed}
                  aria-label={`${isCollapsed ? 'Déplier' : 'Replier'} ${a.email}`}
                  onClick={() => toggle(a.id)}
                >
                  <span className={styles.chevron}>
                    <IconChevron size={14} />
                  </span>
                  <span className={styles.dot} style={{ background: a.color }} />
                  <span className={styles.label}>{a.email}</span>
                </button>
                {a.status === 'auth_error' && (
                  <button
                    className={styles.warnBtn}
                    aria-label={`Problème de connexion : ${a.email}`}
                    title="Mot de passe refusé"
                    onClick={() => setProblemFor(problemFor === a.id ? null : a.id)}
                  >
                    <IconWarn size={15} />
                  </button>
                )}
                {a.status === 'offline' && <span className={styles.statusDot} title={`Hors ligne${a.lastError ? ` : ${a.lastError}` : ''}`} />}
                {a.status === 'syncing' && <span className={`${styles.statusDot} ${styles.syncing}`} title="Synchronisation…" />}
              </div>
              {problemFor === a.id && a.status === 'auth_error' && (
                <div className={styles.problem} role="alert">
                  <strong>Mot de passe refusé</strong>
                  {a.lastError && <p>{a.lastError}</p>}
                  <button className={styles.problemBtn} onClick={() => setPasswordDialogFor(a.id)}>
                    Mettre à jour
                  </button>
                </div>
              )}
              {!isCollapsed &&
                (folders[a.id] ?? []).map((f) => {
                  const Icon = ROLE_ICON[f.role];
                  return (
                    <Entry
                      key={f.id}
                      label={f.displayName}
                      count={f.role === 'sent' || f.role === 'drafts' || f.role === 'trash' ? 0 : f.unreadCount}
                      active={isActive(scope, f)}
                      icon={<Icon size={16} />}
                      onClick={() => selectScope({ kind: 'folder', folderId: f.id })}
                    />
                  );
                })}
            </div>
          );
        })}
      </div>
      <div className={styles.footer}>
        <button
          className={styles.compose}
          disabled={accounts.length === 0}
          onClick={() => void composeNew().catch((e) => console.error('nouveau mail', e))}
        >
          <IconPen size={16} />
          Nouveau mail
        </button>
        <button className={styles.gear} aria-label="Ajouter un compte" title="Ajouter un compte" onClick={() => setShowWizard(true)}>
          <IconGear size={17} />
        </button>
      </div>
    </nav>
  );
}
