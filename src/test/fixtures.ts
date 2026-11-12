import type { Account, Folder, MessageSummary } from '../api/types';

export const account = (id: number, over: Partial<Account> = {}): Account => ({
  id,
  email: id === 1 ? 'moi@gmail.com' : id === 2 ? 'moi@yahoo.fr' : `compte${id}@exemple.fr`,
  displayName: 'Moi',
  provider: 'custom',
  imap: { host: 'h', port: 993, tls: 'implicit' },
  smtp: { host: 'h', port: 465, tls: 'implicit' },
  color: id === 1 ? '#8B5CF6' : '#14B8A6',
  sortOrder: id,
  status: 'ok',
  lastError: null,
  ...over,
});

export const folder = (id: number, accountId: number, role: Folder['role'], name: string, unread = 0): Folder => ({
  id,
  accountId,
  imapPath: name,
  displayName: name,
  role,
  unreadCount: unread,
  totalCount: unread + 10,
});

export const message = (id: number, over: Partial<MessageSummary> = {}): MessageSummary => ({
  id,
  accountId: 1,
  folderId: 10,
  from: { name: 'Marie Dupont', email: 'marie@exemple.fr' },
  subject: `Sujet ${id}`,
  snippet: `Extrait ${id}`,
  date: 1_794_400_000 - id * 60,
  seen: true,
  hasAttachments: false,
  ...over,
});

/** Donne une taille au conteneur de la liste virtualisée (jsdom n'a pas de mise en page). */
export function fakeLayout() {
  Element.prototype.getBoundingClientRect = function () {
    return { width: 380, height: 800, top: 0, left: 0, right: 380, bottom: 800, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
  };
  for (const [prop, value] of [['offsetHeight', 800], ['offsetWidth', 380], ['clientHeight', 800], ['clientWidth', 380]] as const) {
    Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  }
}
