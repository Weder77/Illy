// Données fictives pour développer l'interface dans un navigateur (npm run dev).
import type { Api } from './index';
import type { Account, Folder, MessageDetail, MessageSummary } from './types';

const delay = <T>(v: T, ms = 50) => new Promise<T>((r) => setTimeout(() => r(v), ms));
const server = { host: 'imap.exemple.fr', port: 993, tls: 'implicit' as const };

const accounts: Account[] = [
  { id: 1, email: 'moi@gmail.com', displayName: 'Moi', provider: 'gmail', imap: server, smtp: { ...server, port: 465 }, color: '#8B5CF6', sortOrder: 0, status: 'ok', lastError: null },
  { id: 2, email: 'moi@yahoo.fr', displayName: 'Moi', provider: 'yahoo', imap: server, smtp: { ...server, port: 465 }, color: '#14B8A6', sortOrder: 1, status: 'auth_error', lastError: 'NO [AUTHENTICATIONFAILED] Invalid credentials' },
  { id: 3, email: 'contact@mondomaine.fr', displayName: 'Contact', provider: 'ovh', imap: server, smtp: { ...server, port: 465 }, color: '#EC4899', sortOrder: 2, status: 'ok', lastError: null },
];

const names: [Folder['role'], string][] = [
  ['inbox', 'Boîte de réception'],
  ['drafts', 'Brouillons'],
  ['sent', 'Envoyés'],
  ['spam', 'Spam'],
  ['trash', 'Corbeille'],
];
const folders: Folder[] = accounts.flatMap((a) => [
  ...names.map(([role, displayName], i) => ({
    id: a.id * 100 + i,
    accountId: a.id,
    imapPath: displayName,
    displayName,
    role,
    unreadCount: role === 'inbox' ? a.id + 2 : 0,
    totalCount: role === 'inbox' ? 100 : 0,
  })),
  ...(a.id === 1
    ? [
        { id: 190, accountId: 1, imapPath: 'Factures', displayName: 'Factures', role: 'custom' as const, unreadCount: 1, totalCount: 12 },
        { id: 191, accountId: 1, imapPath: 'Travaux maison', displayName: 'Travaux maison', role: 'custom' as const, unreadCount: 0, totalCount: 4 },
      ]
    : []),
]);

const senders = [
  ['Marie Dupont', 'marie@exemple.fr', 'Devis cuisine', 'Bonjour, voici le devis pour la cuisine comme convenu.'],
  ['Banque Horizon', 'info@banque-horizon.example', 'Votre relevé de novembre', 'Votre relevé de compte est disponible.'],
  ['Atelier Bois & Cie', 'contact@atelier-bois.example', 'Planning de pose', 'Nous pouvons intervenir la semaine prochaine.'],
  ['Électricité Verte', 'factures@elec-verte.example', 'Votre facture', 'Votre facture du mois est disponible dans votre espace.'],
  ['Clément Martin', 'clement@exemple.fr', 'Re: Week-end', 'Ça marche pour samedi !'],
];
const now = Math.floor(Date.now() / 1000);
const messages: MessageSummary[] = Array.from({ length: 300 }, (_, i) => {
  const s = senders[i % senders.length];
  const accountId = (i % 3) + 1;
  return {
    id: i + 1,
    accountId,
    folderId: accountId * 100,
    from: { name: s[0], email: s[1] },
    subject: s[2],
    snippet: s[3],
    date: now - i * 3 * 3600,
    seen: i % 4 !== 0,
    hasAttachments: i % 5 === 0,
  };
});

const pixel =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

function detail(id: number, showImages: boolean): MessageDetail {
  const summary = messages.find((m) => m.id === id) ?? messages[0];
  const imagesBlocked = !showImages;
  const img = imagesBlocked
    ? '<img data-blocked-src="https://t.example/banner.png" alt="Bannière" width="560" height="120">'
    : `<img src="${pixel}" alt="Bannière" width="560" height="120" style="background:#e7eefc">`;
  return {
    summary: { ...summary, seen: true },
    to: [{ name: 'Moi', email: 'moi@gmail.com' }],
    cc: [],
    body: {
      kind: 'html',
      content: `<div style="font-family:Arial,sans-serif;max-width:600px">${img}<h2 style="color:#1d2939">Devis cuisine</h2><p>Bonjour,</p><p>Veuillez trouver ci-joint le devis pour l'aménagement de votre cuisine. <a href="https://exemple.fr/devis">Voir en ligne</a>.</p><p><img src="${pixel}" alt="logo" width="40" height="40"> Atelier Bois &amp; Cie</p></div>`,
    },
    attachments: [
      { partPath: '2', filename: 'devis-cuisine.pdf', mimeType: 'application/pdf', size: 253_952, canOpen: true },
      { partPath: '3', filename: 'outil.exe', mimeType: 'application/octet-stream', size: 4096, canOpen: false },
    ],
    imagesBlocked,
    remoteImagesAllowed: !imagesBlocked,
    degraded: false,
  };
}

function inScope(m: MessageSummary, scope: Parameters<Api['listMessages']>[0]) {
  return scope.kind === 'unified_inbox' ? m.folderId % 100 === 0 : m.folderId === scope.folderId;
}

export const mockApi: Api = {
  detectProvider: async (email) => {
    const d = email.split('@').pop()?.toLowerCase() ?? '';
    if (d === 'gmail.com') return delay({ provider: 'gmail', imap: { host: 'imap.gmail.com', port: 993, tls: 'implicit' }, smtp: { host: 'smtp.gmail.com', port: 465, tls: 'implicit' } });
    if (d.startsWith('yahoo.')) return delay({ provider: 'yahoo', imap: { host: 'imap.mail.yahoo.com', port: 993, tls: 'implicit' }, smtp: { host: 'smtp.mail.yahoo.com', port: 465, tls: 'implicit' } });
    return delay({ provider: null, imap: null, smtp: null });
  },
  addAccount: async (input) => delay({ ...input, id: 99, color: '#F59E0B', sortOrder: 3, status: 'ok', lastError: null }, 600),
  updatePassword: async () => delay(undefined, 400),
  listAccounts: async () => delay(accounts),
  listFolders: async (accountId) => delay(folders.filter((f) => f.accountId === accountId)),
  listMessages: async (scope, offset, limit) => delay(messages.filter((m) => inScope(m, scope)).slice(offset, offset + limit)),
  countMessages: async (scope) => delay(messages.filter((m) => inScope(m, scope)).length),
  selectFolder: async () => delay(undefined),
  openMessage: async (id, showImages) => delay(detail(id, showImages), 120),
  allowImagesFrom: async () => delay(undefined),
  openAttachment: async () => delay(undefined),
  saveAttachment: async () => delay(undefined),
  openUrl: async (url) => {
    window.open(url, '_blank', 'noopener');
  },
  windowAction: async () => undefined,
  onEvent: () => () => undefined,
};
