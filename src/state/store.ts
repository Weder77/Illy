import { create } from 'zustand';
import { api } from '../api';
import type { Account, CoreEvent, Folder, ListScope } from '../api/types';

export interface Selection {
  scope: ListScope;
  messageId: number | null;
}

export interface AppState {
  accounts: Account[];
  folders: Record<number, Folder[]>;
  selection: Selection;
  showWizard: boolean;
  /** Compte dont on met à jour le mot de passe (boîte de dialogue ouverte). */
  passwordDialogFor: number | null;
  /** Compte ajouté à l'instant : sa boîte de réception sera ouverte dès que ses dossiers arrivent. */
  focusAccount: number | null;
  /** Incrémenté quand la liste affichée doit être relue. */
  listVersion: number;
  /** Incrémenté quand un mail change (relecture du mail ouvert). */
  messageVersion: number;
  init(): Promise<void>;
  loadAccounts(): Promise<void>;
  loadFolders(accountId: number): Promise<void>;
  selectScope(scope: ListScope): void;
  selectMessage(id: number | null): void;
  setShowWizard(show: boolean): void;
  setPasswordDialogFor(accountId: number | null): void;
  setFocusAccount(accountId: number | null): void;
  applyEvent(e: CoreEvent): void;
}

const unified: ListScope = { kind: 'unified_inbox' };

/** Pendant la synchro, les événements arrivent en rafale : on regroupe les relectures. */
const REFRESH_WINDOW_MS = 300;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let refreshList = false;
const refreshFolders = new Set<number>();

export const useAppStore = create<AppState>()((set, get) => ({
  accounts: [],
  folders: {},
  selection: { scope: unified, messageId: null },
  showWizard: false,
  passwordDialogFor: null,
  focusAccount: null,
  listVersion: 0,
  messageVersion: 0,

  async init() {
    await get().loadAccounts();
    const accounts = get().accounts;
    await Promise.all(accounts.map((a) => get().loadFolders(a.id)));
    const firstInbox = accounts.map((a) => get().folders[a.id]?.find((f) => f.role === 'inbox')).find(Boolean);
    set({
      selection: { scope: firstInbox ? { kind: 'folder', folderId: firstInbox.id } : unified, messageId: null },
      showWizard: accounts.length === 0,
    });
  },

  async loadAccounts() {
    const accounts = await api.listAccounts();
    set({ accounts });
  },

  async loadFolders(accountId) {
    const list = await api.listFolders(accountId);
    set((s) => ({ folders: { ...s.folders, [accountId]: list } }));
  },

  selectScope(scope) {
    set((s) => ({ selection: { scope, messageId: null }, listVersion: s.listVersion + 1 }));
    if (scope.kind === 'folder') void api.selectFolder(scope.folderId).catch(() => undefined);
  },

  selectMessage(id) {
    set((s) => ({ selection: { ...s.selection, messageId: id } }));
  },

  setShowWizard(show) {
    set({ showWizard: show });
  },

  setPasswordDialogFor(accountId) {
    set({ passwordDialogFor: accountId });
  },

  setFocusAccount(accountId) {
    set({ focusAccount: accountId });
  },

  applyEvent(e) {
    switch (e.type) {
      case 'account_status':
        set((s) => ({
          accounts: s.accounts.map((a) => (a.id === e.accountId ? { ...a, status: e.status, lastError: e.error } : a)),
        }));
        break;
      case 'folder_changed':
      case 'message_updated':
        if (e.type === 'folder_changed') refreshFolders.add(e.accountId);
        refreshList = true;
        if (!refreshTimer) {
          refreshTimer = setTimeout(() => {
            refreshTimer = null;
            const accounts = [...refreshFolders];
            refreshFolders.clear();
            if (refreshList) set((s) => ({ listVersion: s.listVersion + 1, messageVersion: s.messageVersion + 1 }));
            refreshList = false;
            accounts.forEach((id) => void get().loadFolders(id).catch(() => undefined));
          }, REFRESH_WINDOW_MS);
        }
        break;
      case 'folders_changed':
        void get()
          .loadFolders(e.accountId)
          .then(() => {
            if (get().focusAccount !== e.accountId) return;
            const inbox = get().folders[e.accountId]?.find((f) => f.role === 'inbox');
            if (!inbox) return;
            set({ focusAccount: null });
            get().selectScope({ kind: 'folder', folderId: inbox.id });
          })
          .catch(() => undefined);
        break;
      case 'sync_progress':
        break;
    }
  },
}));
