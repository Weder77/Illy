import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { Account, Draft, Folder } from '../api/types';
import { defaultSettings } from '../api/types';

const acc = (id: number, status: Account['status'] = 'ok'): Account => ({
  id,
  email: `moi${id}@exemple.fr`,
  displayName: 'Moi',
  provider: 'custom',
  imap: { host: 'h', port: 993, tls: 'implicit' },
  smtp: { host: 'h', port: 465, tls: 'implicit' },
  color: '#8B5CF6',
  sortOrder: id,
  status,
  lastError: null,
});
const folder = (id: number, accountId: number, role: Folder['role']): Folder => ({
  id,
  accountId,
  imapPath: role === 'inbox' ? 'INBOX' : `F${id}`,
  displayName: role === 'inbox' ? 'Boîte de réception' : `F${id}`,
  role,
  unreadCount: 0,
  totalCount: 0,
});

const draft = (id: number, over: Partial<Draft> = {}): Draft => ({
  id,
  accountId: 1,
  kind: 'reply',
  sourceMessageId: 5,
  to: [{ name: 'Bob', email: 'bob@exemple.fr' }],
  cc: [],
  bcc: [],
  subject: 'Re: Salut',
  bodyHtml: '<p></p>',
  attachments: [],
  state: 'editing',
  lastError: null,
  sendAt: null,
  remoteUid: null,
  updatedAt: 0,
  ...over,
});

const fake = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  listFolders: vi.fn(),
  selectFolder: vi.fn(),
  getSettings: vi.fn(),
  listOutbox: vi.fn(),
  getDraft: vi.fn(),
  createDraft: vi.fn(),
  openComposeWindow: vi.fn(),
}));
vi.mock('../api', () => ({ api: fake }));

import { useAppStore } from './store';

beforeEach(() => {
  fake.listAccounts.mockReset().mockResolvedValue([acc(1), acc(2)]);
  fake.listFolders.mockReset().mockImplementation(async (id: number) =>
    id === 1 ? [folder(11, 1, 'custom'), folder(10, 1, 'inbox')] : [folder(20, 2, 'inbox')],
  );
  fake.selectFolder.mockReset().mockResolvedValue(undefined);
  fake.getSettings.mockReset().mockResolvedValue(defaultSettings);
  fake.listOutbox.mockReset().mockResolvedValue([]);
  fake.getDraft.mockReset().mockImplementation(async (id: number) => draft(id, { state: 'queued', sendAt: 100 }));
  fake.createDraft.mockReset().mockImplementation(async () => draft(7));
  fake.openComposeWindow.mockReset().mockResolvedValue(undefined);
  useAppStore.setState(useAppStore.getInitialState(), true);
});

describe('app store', () => {
  test('startup selects first inbox', async () => {
    await useAppStore.getState().init();
    const s = useAppStore.getState();
    expect(s.selection.scope).toEqual({ kind: 'folder', folderId: 10 });
    expect(s.showWizard).toBe(false);
    expect(s.folders[2]).toHaveLength(1);
  });

  test('startup without accounts opens the wizard on the unified view', async () => {
    fake.listAccounts.mockResolvedValue([]);
    await useAppStore.getState().init();
    expect(useAppStore.getState().showWizard).toBe(true);
    expect(useAppStore.getState().selection.scope).toEqual({ kind: 'unified_inbox' });
  });

  test('applyEvent account_status patches account', async () => {
    await useAppStore.getState().init();
    useAppStore.getState().applyEvent({ type: 'account_status', accountId: 2, status: 'auth_error', error: 'NO' });
    const a = useAppStore.getState().accounts.find((x) => x.id === 2)!;
    expect(a.status).toBe('auth_error');
    expect(a.lastError).toBe('NO');
  });

  test('selectScope folder calls api.selectFolder and clears message', async () => {
    await useAppStore.getState().init();
    useAppStore.getState().selectMessage(5);
    useAppStore.getState().selectScope({ kind: 'folder', folderId: 20 });
    expect(fake.selectFolder).toHaveBeenCalledWith(20);
    expect(useAppStore.getState().selection).toEqual({ scope: { kind: 'folder', folderId: 20 }, messageId: null });
  });

  test('folder_changed bumps list version and reloads folders', async () => {
    await useAppStore.getState().init();
    const v = useAppStore.getState().listVersion;
    fake.listFolders.mockClear();
    useAppStore.getState().applyEvent({ type: 'folder_changed', accountId: 1, folderId: 10 });
    await vi.waitFor(() => expect(useAppStore.getState().listVersion).toBe(v + 1));
    expect(fake.listFolders).toHaveBeenCalledWith(1);
  });

  test('focusAccount selects the new inbox once its folders arrive', async () => {
    await useAppStore.getState().init();
    useAppStore.setState({ focusAccount: 2 });
    useAppStore.getState().applyEvent({ type: 'folders_changed', accountId: 2 });
    await vi.waitFor(() => expect(useAppStore.getState().selection.scope).toEqual({ kind: 'folder', folderId: 20 }));
    expect(useAppStore.getState().focusAccount).toBeNull();
  });

  test('bursts of sync events refresh the list at most once per window', async () => {
    await useAppStore.getState().init();
    vi.useFakeTimers();
    try {
      const v = useAppStore.getState().listVersion;
      fake.listFolders.mockClear();
      for (let i = 0; i < 50; i++) {
        useAppStore.getState().applyEvent({ type: 'message_updated', messageId: i });
        useAppStore.getState().applyEvent({ type: 'folder_changed', accountId: 1, folderId: 10 });
      }
      expect(useAppStore.getState().listVersion).toBe(v);
      vi.advanceTimersByTime(400);
      expect(useAppStore.getState().listVersion).toBe(v + 1);
      expect(fake.listFolders).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  test('init loads settings and outbox', async () => {
    fake.getSettings.mockResolvedValue({ ...defaultSettings, undoSendSeconds: 30 });
    fake.listOutbox.mockResolvedValue([draft(3, { state: 'failed', lastError: 'refusé' })]);
    await useAppStore.getState().init();
    expect(useAppStore.getState().settings.undoSendSeconds).toBe(30);
    expect(useAppStore.getState().outbox.map((d) => d.id)).toEqual([3]);
  });

  test('outbox_changed updates queue', async () => {
    await useAppStore.getState().init();
    const { applyEvent } = useAppStore.getState();
    applyEvent({ type: 'outbox_changed', draftId: 4, state: 'queued', sendAt: 100, error: null });
    await vi.waitFor(() => expect(useAppStore.getState().outbox.map((d) => d.id)).toEqual([4]));
    applyEvent({ type: 'outbox_changed', draftId: 4, state: 'failed', sendAt: 100, error: 'Destinataire refusé' });
    expect(useAppStore.getState().outbox[0]).toMatchObject({ state: 'failed', lastError: 'Destinataire refusé' });
    // Annulé : le brouillon redevient modifiable et quitte la file.
    applyEvent({ type: 'outbox_changed', draftId: 4, state: 'editing', sendAt: null, error: null });
    expect(useAppStore.getState().outbox).toEqual([]);
    expect(fake.getDraft).toHaveBeenCalledTimes(1);
  });

  test('sent drafts leave the outbox after 3 s', async () => {
    useAppStore.setState({ outbox: [draft(4, { state: 'queued', sendAt: 100 })] });
    vi.useFakeTimers();
    try {
      useAppStore.getState().applyEvent({ type: 'outbox_changed', draftId: 4, state: 'sent', sendAt: 100, error: null });
      expect(useAppStore.getState().outbox).toEqual([]);
      expect(useAppStore.getState().recentlySent).toEqual([4]);
      vi.advanceTimersByTime(3100);
      expect(useAppStore.getState().recentlySent).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  test('openReply creates a draft for the selected message', async () => {
    await useAppStore.getState().init();
    useAppStore.getState().selectMessage(5);
    await useAppStore.getState().openReply('reply_all');
    expect(fake.createDraft).toHaveBeenCalledWith('reply_all', undefined, 5);
    expect(useAppStore.getState().replyTarget).toEqual({ messageId: 5, draftId: 7 });
    useAppStore.getState().closeReply();
    expect(useAppStore.getState().replyTarget).toBeNull();
  });

  test('changing message closes the inline reply', async () => {
    await useAppStore.getState().init();
    useAppStore.getState().selectMessage(5);
    await useAppStore.getState().openReply('reply');
    useAppStore.getState().selectMessage(6);
    expect(useAppStore.getState().replyTarget).toBeNull();
  });

  test('reopenDraft goes inline for the selected message, else to a window', async () => {
    await useAppStore.getState().init();
    useAppStore.getState().selectMessage(5);
    useAppStore.getState().reopenDraft(draft(8, { sourceMessageId: 5 }));
    expect(useAppStore.getState().replyTarget).toEqual({ messageId: 5, draftId: 8 });
    useAppStore.getState().reopenDraft(draft(9, { sourceMessageId: 77 }));
    expect(fake.openComposeWindow).toHaveBeenCalledWith(9);
  });

  test('settings_changed reloads settings', async () => {
    await useAppStore.getState().init();
    fake.getSettings.mockResolvedValue({ ...defaultSettings, theme: 'light' });
    useAppStore.getState().applyEvent({ type: 'settings_changed', key: 'theme' });
    await vi.waitFor(() => expect(useAppStore.getState().settings.theme).toBe('light'));
  });
});
