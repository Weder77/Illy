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
  applyEvent(e: CoreEvent): void;
}

const unified: ListScope = { kind: 'unified_inbox' };

export const useAppStore = create<AppState>()((set, get) => ({
  accounts: [],
  folders: {},
  selection: { scope: unified, messageId: null },
  showWizard: false,
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

  applyEvent(e) {
    switch (e.type) {
      case 'account_status':
        set((s) => ({
          accounts: s.accounts.map((a) => (a.id === e.accountId ? { ...a, status: e.status, lastError: e.error } : a)),
        }));
        break;
      case 'folders_changed':
        void get().loadFolders(e.accountId).catch(() => undefined);
        break;
      case 'folder_changed':
        set((s) => ({ listVersion: s.listVersion + 1 }));
        void get().loadFolders(e.accountId).catch(() => undefined);
        break;
      case 'message_updated':
        set((s) => ({ listVersion: s.listVersion + 1, messageVersion: s.messageVersion + 1 }));
        break;
      case 'sync_progress':
        break;
    }
  },
}));
