import type {
  Account,
  Address,
  CoreEvent,
  Draft,
  DraftInput,
  DraftKind,
  Folder,
  ListScope,
  MessageDetail,
  MessageSummary,
  NewAccount,
  ProviderGuess,
  SendTicket,
  SettingKey,
  Settings,
} from './types';
import { tauriApi } from './tauri';
import { mockApi } from './mock';

export type WindowAction = 'minimize' | 'toggleMaximize' | 'close';

/** Seul point de contact entre l'interface et le cœur. */
export interface Api {
  detectProvider(email: string): Promise<ProviderGuess>;
  addAccount(input: NewAccount, password: string): Promise<Account>;
  updatePassword(accountId: number, password: string): Promise<void>;
  listAccounts(): Promise<Account[]>;
  listFolders(accountId: number): Promise<Folder[]>;
  listMessages(scope: ListScope, offset: number, limit: number): Promise<MessageSummary[]>;
  countMessages(scope: ListScope): Promise<number>;
  selectFolder(folderId: number): Promise<void>;
  openMessage(id: number, showImages: boolean): Promise<MessageDetail>;
  allowImagesFrom(sender: string): Promise<void>;
  openAttachment(id: number, partPath: string): Promise<void>;
  saveAttachment(id: number, partPath: string): Promise<void>;
  openUrl(url: string): Promise<void>;
  windowAction(action: WindowAction): Promise<void>;
  onEvent(cb: (e: CoreEvent) => void): () => void;
  // Rédaction
  searchContacts(query: string): Promise<Address[]>;
  createDraft(kind: DraftKind, accountId?: number, sourceMessageId?: number): Promise<Draft>;
  draftForMessage(messageId: number): Promise<Draft | null>;
  draftFromMessage(messageId: number): Promise<Draft>;
  getDraft(draftId: number): Promise<Draft>;
  updateDraft(input: DraftInput): Promise<Draft>;
  pickAttachments(draftId: number): Promise<Draft>;
  addAttachments(draftId: number, paths: string[]): Promise<Draft>;
  removeAttachment(draftId: number, index: number): Promise<Draft>;
  closeDraft(draftId: number): Promise<void>;
  discardDraft(draftId: number): Promise<void>;
  sendDraft(draftId: number): Promise<SendTicket>;
  cancelSend(draftId: number): Promise<boolean>;
  listOutbox(): Promise<Draft[]>;
  getSettings(): Promise<Settings>;
  setSetting(key: SettingKey, value: unknown): Promise<Settings>;
  openComposeWindow(draftId: number): Promise<void>;
  /** Fenêtre « Nouveau mail » : appelé avant sa fermeture (le brouillon doit être enregistré). */
  onWindowCloseRequested(cb: () => Promise<void>): () => void;
  /** Fichiers déposés sur la fenêtre (chemins), hors zone de texte. */
  onFileDrop(cb: (paths: string[]) => void): () => void;
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const api: Api = inTauri ? tauriApi : mockApi;
