// Données fictives pour développer l'interface dans un navigateur (npm run dev).
import type { Api } from './index';
import type { Account, Address, CoreEvent, Draft, DraftKind, Folder, MessageDetail, MessageSummary, Settings } from './types';
import { defaultSettings } from './types';

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

// --- Rédaction en mémoire ---
const listeners = new Set<(e: CoreEvent) => void>();
const emit = (e: CoreEvent) => listeners.forEach((cb) => cb(e));
const drafts = new Map<number, Draft>();
const sendTimers = new Map<number, ReturnType<typeof setTimeout>>();
let nextDraftId = 1;
let settings: Settings = { ...defaultSettings };
const allowlist = new Set(['newsletter@exemple.fr', 'info@banque-horizon.example']);
const contacts: Address[] = senders.map((s) => ({ name: s[0], email: s[1] }));
const fold = (t: string) => t.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();

function getDraft(id: number): Draft {
  const d = drafts.get(id);
  if (!d) throw { kind: 'not_found', message: 'brouillon introuvable' };
  return d;
}

function newDraft(kind: DraftKind, accountId?: number, source?: number): Draft {
  const src = source ? messages.find((m) => m.id === source) : undefined;
  const prefix = kind === 'forward' ? 'Fwd: ' : kind === 'new' ? '' : 'Re: ';
  const d: Draft = {
    id: nextDraftId++,
    accountId: src?.accountId ?? accountId ?? accounts[0].id,
    kind,
    sourceMessageId: src?.id ?? null,
    to: src && kind !== 'forward' ? [src.from] : [],
    cc: [],
    bcc: [],
    subject: src ? prefix + src.subject : '',
    bodyHtml: src
      ? kind === 'forward'
        ? `<p></p><div class="illy-forward"><p>---------- Message transféré ----------<br>De : ${src.from.name} &lt;${src.from.email}&gt;<br>Objet : ${src.subject}</p><p>${src.snippet}</p></div>`
        : `<p></p><div class="illy-quote"><p>Le ${new Date(src.date * 1000).toLocaleString('fr-FR', { dateStyle: 'full', timeStyle: 'short' })}, ${src.from.name} a écrit :</p><blockquote><p>${src.snippet}</p></blockquote></div>`
      : '<p></p>',
    attachments: kind === 'forward' && src?.hasAttachments ? [{ filename: 'devis-cuisine.pdf', mimeType: 'application/pdf', size: 253_952, source: { kind: 'part', messageId: src.id, partPath: '2' } }] : [],
    state: 'editing',
    lastError: null,
    sendAt: null,
    remoteUid: null,
    updatedAt: now,
  };
  drafts.set(d.id, d);
  return d;
}

function setDraft(d: Draft): Draft {
  drafts.set(d.id, d);
  return d;
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
  onEvent: (cb) => {
    listeners.add(cb);
    return () => listeners.delete(cb);
  },
  searchContacts: async (query) => {
    const q = fold(query.trim());
    return delay(q ? contacts.filter((c) => ` ${fold(c.email)} ${fold(c.name ?? '')}`.includes(` ${q}`)).slice(0, 8) : []);
  },
  createDraft: async (kind, accountId, sourceMessageId) => delay(newDraft(kind, accountId, sourceMessageId)),
  draftForMessage: async (messageId) =>
    delay([...drafts.values()].find((d) => d.sourceMessageId === messageId && d.state === 'editing') ?? null),
  draftFromMessage: async (messageId) => delay(newDraft('new', undefined, undefined), 80).then((d) => {
    const src = messages.find((m) => m.id === messageId);
    return setDraft({ ...d, subject: src?.subject ?? '', bodyHtml: `<p>${src?.snippet ?? ''}</p>`, remoteUid: messageId });
  }),
  // Une fenêtre « Nouveau mail » du navigateur a son propre mock : le brouillon y est recréé.
  getDraft: async (draftId) => delay(drafts.get(draftId) ?? setDraft({ ...newDraft('new'), id: draftId })),
  updateDraft: async (input) => {
    const d = getDraft(input.id);
    if (d.state !== 'editing' && d.state !== 'failed') throw { kind: 'invalid_input', message: "brouillon en cours d'envoi" };
    return delay(setDraft({ ...d, ...input, updatedAt: Math.floor(Date.now() / 1000) }));
  },
  pickAttachments: async (draftId) => {
    const d = getDraft(draftId);
    const file = { filename: 'photo-chantier.jpg', mimeType: 'image/jpeg', size: 1_843_200, source: { kind: 'file' as const, path: 'C:/photo-chantier.jpg' } };
    return delay(setDraft({ ...d, attachments: [...d.attachments, file] }), 200);
  },
  addAttachments: async (draftId, paths) => {
    const d = getDraft(draftId);
    const files = paths.map((path) => ({ filename: path.split(/[\\/]/).pop() ?? path, mimeType: 'application/octet-stream', size: 120_000, source: { kind: 'file' as const, path } }));
    return delay(setDraft({ ...d, attachments: [...d.attachments, ...files] }));
  },
  removeAttachment: async (draftId, index) => {
    const d = getDraft(draftId);
    return delay(setDraft({ ...d, attachments: d.attachments.filter((_, i) => i !== index) }));
  },
  closeDraft: async () => delay(undefined),
  discardDraft: async (draftId) => {
    drafts.delete(draftId);
    return delay(undefined);
  },
  sendDraft: async (draftId) => {
    const d = getDraft(draftId);
    if (d.to.length + d.cc.length + d.bcc.length === 0) throw { kind: 'invalid_input', message: 'Ajoute au moins un destinataire.' };
    const sendAt = Math.floor(Date.now() / 1000) + settings.undoSendSeconds;
    setDraft({ ...d, state: 'queued', sendAt, lastError: null });
    emit({ type: 'outbox_changed', draftId, state: 'queued', sendAt, error: null });
    sendTimers.set(
      draftId,
      setTimeout(() => {
        sendTimers.delete(draftId);
        drafts.delete(draftId);
        emit({ type: 'outbox_changed', draftId, state: 'sent', sendAt, error: null });
      }, settings.undoSendSeconds * 1000),
    );
    return delay({ sendAt });
  },
  cancelSend: async (draftId) => {
    const t = sendTimers.get(draftId);
    if (!t) return delay(false);
    clearTimeout(t);
    sendTimers.delete(draftId);
    const d = setDraft({ ...getDraft(draftId), state: 'editing', sendAt: null });
    emit({ type: 'outbox_changed', draftId, state: 'editing', sendAt: null, error: null });
    return delay(d.state === 'editing');
  },
  listOutbox: async () => delay([...drafts.values()].filter((d) => d.state === 'queued' || d.state === 'sending' || d.state === 'failed')),
  getSettings: async () => delay(settings),
  setSetting: async (key, value) => {
    const camel = key.replace(/_(\w)/g, (_, c: string) => c.toUpperCase()) as keyof Settings;
    settings = { ...settings, [camel]: value };
    emit({ type: 'settings_changed', key });
    return delay(settings);
  },
  openComposeWindow: async (draftId) => {
    window.open(`${location.pathname}#compose/${draftId}`, `compose-${draftId}`, 'width=760,height=640');
  },
  reorderAccounts: async (ids) => {
    const byId = new Map(accounts.map((a) => [a.id, a]));
    const next = ids.map((id, i) => ({ ...byId.get(id)!, sortOrder: i }));
    accounts.splice(0, accounts.length, ...next);
    return delay([...accounts]);
  },
  updateAccount: async (accountId, patch) => {
    const i = accounts.findIndex((a) => a.id === accountId);
    if (patch.displayName !== undefined && !patch.displayName.trim()) throw { kind: 'invalid_input', message: 'Le nom affiché ne peut pas être vide.' };
    accounts[i] = { ...accounts[i], ...Object.fromEntries(Object.entries(patch).filter(([, v]) => v !== undefined)) };
    return delay(accounts[i]);
  },
  updateServers: async (accountId, imap, smtp) => {
    if (!imap.host.includes('.')) throw { kind: 'network', message: `serveur injoignable : ${imap.host}` };
    const i = accounts.findIndex((a) => a.id === accountId);
    accounts[i] = { ...accounts[i], imap, smtp };
    return delay(accounts[i], 600);
  },
  removeAccount: async (accountId) => {
    const i = accounts.findIndex((a) => a.id === accountId);
    if (i >= 0) accounts.splice(i, 1);
    return delay(undefined);
  },
  listImageAllowlist: async () => delay([...allowlist].sort()),
  removeFromImageAllowlist: async (sender) => {
    allowlist.delete(sender);
    return delay(undefined);
  },
  storageUsage: async () => delay({ database: 48_200_000, drafts: 1_843_200, temporary: 253_952 }),
  clearCache: async () => {
    if ([...drafts.values()].some((d) => d.state === 'queued' || d.state === 'sending' || d.state === 'failed')) {
      throw { kind: 'invalid_input', message: 'Attends la fin des envois en cours' };
    }
    return delay(undefined, 800);
  },
  openLogsFolder: async () => delay(undefined),
  appInfo: async () => delay({ version: '0.3.0', logsDir: 'C:\\Users\\moi\\AppData\\Local\\Illy\\logs' }),
  checkUpdate: async () => delay(null, 400),
  downloadUpdate: async (onProgress) => {
    for (const p of [0, 35, 70, 100]) {
      onProgress(p);
      await delay(undefined, 150);
    }
  },
  applyUpdate: async () => delay(undefined),
  closeComposeWindows: async () => delay(undefined),
  onWindowCloseRequested: () => () => undefined,
  onFileDrop: () => () => undefined,
};
