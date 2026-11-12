// Miroir TypeScript de crates/illy-core/src/model.rs et events.rs.

export type Provider = 'gmail' | 'yahoo' | 'ovh' | 'custom';
export type TlsMode = 'implicit' | 'start_tls' | 'plain';

export interface ServerConfig {
  host: string;
  port: number;
  tls: TlsMode;
}

export interface NewAccount {
  email: string;
  displayName: string;
  provider: Provider;
  imap: ServerConfig;
  smtp: ServerConfig;
}

export type AccountStatus = 'ok' | 'syncing' | 'auth_error' | 'offline' | 'error';

export interface Account extends NewAccount {
  id: number;
  /** Signature HTML du compte (nettoyée). */
  signatureHtml?: string;
  color: string;
  sortOrder: number;
  status: AccountStatus;
  lastError: string | null;
}

export type FolderRole = 'inbox' | 'drafts' | 'sent' | 'spam' | 'trash' | 'custom';

export interface Folder {
  id: number;
  accountId: number;
  imapPath: string;
  displayName: string;
  role: FolderRole;
  unreadCount: number;
  totalCount: number;
}

export interface Address {
  name: string | null;
  email: string;
}

export interface MessageSummary {
  id: number;
  accountId: number;
  folderId: number;
  from: Address;
  subject: string;
  snippet: string;
  date: number;
  seen: boolean;
  hasAttachments: boolean;
}

export interface Attachment {
  partPath: string;
  filename: string;
  mimeType: string;
  size: number;
  canOpen: boolean;
}

export interface MessageBody {
  kind: 'html' | 'text';
  content: string;
}

export interface MessageDetail {
  summary: MessageSummary;
  to: Address[];
  cc: Address[];
  body: MessageBody;
  attachments: Attachment[];
  imagesBlocked: boolean;
  /** Seul critère d'ouverture de la CSP aux images distantes. */
  remoteImagesAllowed: boolean;
  degraded: boolean;
}

export type ListScope = { kind: 'folder'; folderId: number } | { kind: 'unified_inbox' };

export interface ProviderGuess {
  provider: Provider | null;
  imap: ServerConfig | null;
  smtp: ServerConfig | null;
}

export type DraftKind = 'new' | 'reply' | 'reply_all' | 'forward';
export type DraftState = 'editing' | 'queued' | 'sending' | 'sent' | 'failed';

export type AttachmentSource =
  | { kind: 'file'; path: string }
  | { kind: 'part'; messageId: number; partPath: string };

export interface DraftAttachment {
  filename: string;
  mimeType: string;
  size: number;
  source: AttachmentSource;
}

export interface Draft {
  id: number;
  accountId: number;
  kind: DraftKind;
  sourceMessageId: number | null;
  to: Address[];
  cc: Address[];
  bcc: Address[];
  subject: string;
  bodyHtml: string;
  attachments: DraftAttachment[];
  state: DraftState;
  lastError: string | null;
  /** Échéance d'envoi (secondes Unix) : fin du délai d'annulation. */
  sendAt: number | null;
  remoteUid: number | null;
  updatedAt: number;
}

export interface DraftInput {
  id: number;
  accountId: number;
  to: Address[];
  cc: Address[];
  bcc: Address[];
  subject: string;
  bodyHtml: string;
}

export interface SendTicket {
  sendAt: number;
}

export type RemoteImages = 'block' | 'always';
export type Theme = 'dark' | 'light' | 'system';
export type Accent = 'blue' | 'violet' | 'green' | 'orange' | 'pink' | 'teal';
export type TextSize = 'small' | 'normal' | 'large';
export type Density = 'comfortable' | 'compact';

export interface Settings {
  undoSendSeconds: number;
  remoteImages: RemoteImages;
  theme: Theme;
  accent: Accent;
  textSize: TextSize;
  density: Density;
  showSnippet: boolean;
}

/** Clés acceptées par `setSetting` (noms de la table `settings`). */
export type SettingKey = 'undo_send_seconds' | 'remote_images' | 'theme' | 'accent' | 'text_size' | 'density' | 'show_snippet';

export const defaultSettings: Settings = {
  undoSendSeconds: 10,
  remoteImages: 'block',
  theme: 'dark',
  accent: 'blue',
  textSize: 'normal',
  density: 'comfortable',
  showSnippet: true,
};

export interface AccountPatch {
  displayName?: string;
  color?: string;
  signatureHtml?: string;
}

export interface StorageUsage {
  database: number;
  drafts: number;
  temporary: number;
}

export interface AppInfo {
  version: string;
  logsDir: string;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
}

/** Palette des comptes (même ordre que le cœur). */
export const ACCOUNT_COLORS = ['#8B5CF6', '#14B8A6', '#EC4899', '#F59E0B', '#22C55E', '#38BDF8'];

export type CoreEvent =
  | { type: 'account_status'; accountId: number; status: AccountStatus; error: string | null }
  | { type: 'folders_changed'; accountId: number }
  | { type: 'folder_changed'; accountId: number; folderId: number }
  | { type: 'message_updated'; messageId: number }
  | { type: 'sync_progress'; accountId: number; folderId: number; fetched: number; total: number }
  | { type: 'outbox_changed'; draftId: number; state: DraftState; sendAt: number | null; error: string | null }
  | { type: 'settings_changed'; key: string }
  | { type: 'accounts_changed' };

export interface ApiError {
  kind: string;
  message: string;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === 'object' && e !== null && 'kind' in e && 'message' in e;
}

export function errorMessage(e: unknown): string {
  if (isApiError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

export function sameScope(a: ListScope, b: ListScope): boolean {
  return a.kind === b.kind && (a.kind === 'unified_inbox' || a.folderId === (b as { folderId: number }).folderId);
}
