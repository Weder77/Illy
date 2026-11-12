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
  degraded: boolean;
}

export type ListScope = { kind: 'folder'; folderId: number } | { kind: 'unified_inbox' };

export interface ProviderGuess {
  provider: Provider | null;
  imap: ServerConfig | null;
  smtp: ServerConfig | null;
}

export type CoreEvent =
  | { type: 'account_status'; accountId: number; status: AccountStatus; error: string | null }
  | { type: 'folders_changed'; accountId: number }
  | { type: 'folder_changed'; accountId: number; folderId: number }
  | { type: 'message_updated'; messageId: number }
  | { type: 'sync_progress'; accountId: number; folderId: number; fetched: number; total: number };

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
