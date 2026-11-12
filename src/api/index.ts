import type {
  Account,
  AccountPatch,
  Address,
  AppInfo,
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
  ServerConfig,
  SettingKey,
  Settings,
  StorageUsage,
  UpdateInfo,
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
  // Paramètres
  reorderAccounts(ids: number[]): Promise<Account[]>;
  updateAccount(accountId: number, patch: AccountPatch): Promise<Account>;
  updateServers(accountId: number, imap: ServerConfig, smtp: ServerConfig): Promise<Account>;
  removeAccount(accountId: number): Promise<void>;
  listImageAllowlist(): Promise<string[]>;
  removeFromImageAllowlist(sender: string): Promise<void>;
  storageUsage(): Promise<StorageUsage>;
  clearCache(): Promise<void>;
  openLogsFolder(): Promise<void>;
  appInfo(): Promise<AppInfo>;
  // Mises à jour (rien ne s'installe sans un appel explicite à installUpdate)
  checkUpdate(): Promise<UpdateInfo | null>;
  /** Télécharge (progression en %, ou null si la taille est inconnue), installe, puis relance Illy. */
  installUpdate(onProgress: (percent: number | null) => void): Promise<void>;
  /** Fichiers déposés sur la fenêtre : chemins et point de dépôt (pixels CSS de la fenêtre). */
  onFileDrop(cb: (paths: string[], at: { x: number; y: number }) => void): () => void;
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const api: Api = inTauri ? tauriApi : mockApi;
