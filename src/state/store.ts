import { create } from 'zustand';
import { api } from '../api';
import type { Account, CoreEvent, Draft, DraftKind, Folder, ListScope, Settings } from '../api/types';
import { defaultSettings } from '../api/types';

export interface Selection {
  scope: ListScope;
  messageId: number | null;
}

/** Réponse ouverte sous le mail affiché. */
export interface ReplyTarget {
  messageId: number;
  draftId: number;
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
  /** Envois en attente, en cours ou en échec. */
  outbox: Draft[];
  /** Brouillons envoyés il y a moins de 3 s (« Mail envoyé »). */
  recentlySent: number[];
  settings: Settings;
  replyTarget: ReplyTarget | null;
  init(): Promise<void>;
  loadSettings(): Promise<void>;
  openReply(kind: Exclude<DraftKind, 'new'>): Promise<void>;
  /** Rouvre un brouillon : sous le mail affiché s'il y répond, sinon dans une fenêtre. */
  reopenDraft(draft: Draft): void;
  closeReply(): void;
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

const SENT_NOTICE_MS = 3000;
const inOutbox = (state: Draft['state']) => state === 'queued' || state === 'sending' || state === 'failed';

export const useAppStore = create<AppState>()((set, get) => ({
  accounts: [],
  folders: {},
  selection: { scope: unified, messageId: null },
  showWizard: false,
  passwordDialogFor: null,
  focusAccount: null,
  listVersion: 0,
  messageVersion: 0,
  outbox: [],
  recentlySent: [],
  settings: defaultSettings,
  replyTarget: null,

  async init() {
    const extras = Promise.all([
      get().loadSettings().catch(() => undefined),
      api
        .listOutbox()
        .then((outbox) => set({ outbox }))
        .catch(() => undefined),
    ]);
    await get().loadAccounts();
    const accounts = get().accounts;
    await Promise.all(accounts.map((a) => get().loadFolders(a.id)));
    const firstInbox = accounts.map((a) => get().folders[a.id]?.find((f) => f.role === 'inbox')).find(Boolean);
    set({
      selection: { scope: firstInbox ? { kind: 'folder', folderId: firstInbox.id } : unified, messageId: null },
      showWizard: accounts.length === 0,
    });
    await extras;
  },

  async loadSettings() {
    set({ settings: await api.getSettings() });
  },

  async openReply(kind) {
    const messageId = get().selection.messageId;
    if (messageId == null) return;
    const d = await api.createDraft(kind, undefined, messageId);
    if (get().selection.messageId === messageId) set({ replyTarget: { messageId, draftId: d.id } });
  },

  reopenDraft(draft) {
    const messageId = get().selection.messageId;
    if (draft.sourceMessageId != null && draft.sourceMessageId === messageId) {
      set({ replyTarget: { messageId, draftId: draft.id } });
    } else {
      void api.openComposeWindow(draft.id).catch(() => undefined);
    }
  },

  closeReply() {
    set({ replyTarget: null });
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
    set((s) => ({ selection: { scope, messageId: null }, listVersion: s.listVersion + 1, replyTarget: null }));
    if (scope.kind === 'folder') void api.selectFolder(scope.folderId).catch(() => undefined);
  },

  selectMessage(id) {
    // Changer de mail ferme la réponse en cours (l'éditeur enregistre le brouillon en se fermant).
    set((s) => ({
      selection: { ...s.selection, messageId: id },
      replyTarget: s.replyTarget && s.replyTarget.messageId !== id ? null : s.replyTarget,
    }));
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
      case 'outbox_changed': {
        const { draftId, state, sendAt, error } = e;
        const known = get().outbox.some((d) => d.id === draftId);
        if (inOutbox(state)) {
          if (known) {
            set((s) => ({
              outbox: s.outbox.map((d) => (d.id === draftId ? { ...d, state, sendAt, lastError: error } : d)),
            }));
          } else {
            void api
              .getDraft(draftId)
              .then((d) => {
                const fresh = { ...d, state, sendAt, lastError: error };
                set((s) => ({
                  outbox: s.outbox.some((x) => x.id === draftId)
                    ? s.outbox.map((x) => (x.id === draftId ? fresh : x))
                    : [...s.outbox, fresh],
                }));
              })
              .catch(() => undefined);
          }
          break;
        }
        set((s) => ({
          outbox: s.outbox.filter((d) => d.id !== draftId),
          recentlySent: state === 'sent' ? [...s.recentlySent.filter((id) => id !== draftId), draftId] : s.recentlySent,
        }));
        if (state === 'sent') {
          setTimeout(() => set((s) => ({ recentlySent: s.recentlySent.filter((id) => id !== draftId) })), SENT_NOTICE_MS);
        }
        break;
      }
      case 'settings_changed':
        void get()
          .loadSettings()
          .catch(() => undefined);
        break;
    }
  },
}));
