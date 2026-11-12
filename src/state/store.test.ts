import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { Account, Folder } from '../api/types';

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

const fake = vi.hoisted(() => ({
  listAccounts: vi.fn(),
  listFolders: vi.fn(),
  selectFolder: vi.fn(),
}));
vi.mock('../api', () => ({ api: fake }));

import { useAppStore } from './store';

beforeEach(() => {
  fake.listAccounts.mockReset().mockResolvedValue([acc(1), acc(2)]);
  fake.listFolders.mockReset().mockImplementation(async (id: number) =>
    id === 1 ? [folder(11, 1, 'custom'), folder(10, 1, 'inbox')] : [folder(20, 2, 'inbox')],
  );
  fake.selectFolder.mockReset().mockResolvedValue(undefined);
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
});
