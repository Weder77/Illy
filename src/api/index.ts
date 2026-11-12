import type {
  Account,
  CoreEvent,
  Folder,
  ListScope,
  MessageDetail,
  MessageSummary,
  NewAccount,
  ProviderGuess,
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
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const api: Api = inTauri ? tauriApi : mockApi;
