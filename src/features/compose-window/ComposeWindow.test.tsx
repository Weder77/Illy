import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

const fake = vi.hoisted(() => ({
  listAccounts: vi.fn().mockResolvedValue([]),
  getDraft: vi.fn(),
  windowAction: vi.fn().mockResolvedValue(undefined),
  onWindowCloseRequested: vi.fn(),
  onFileDrop: vi.fn().mockReturnValue(() => undefined),
  searchContacts: vi.fn().mockResolvedValue([]),
  closeDraft: vi.fn().mockResolvedValue(undefined),
  updateDraft: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { ComposeWindow, composeRoute } from './ComposeWindow';

test('composeRoute', () => {
  expect(composeRoute('#compose/12')).toBe(12);
  expect(composeRoute('')).toBeNull();
  expect(composeRoute('#compose/x')).toBeNull();
});

test('closing the window saves the draft before it goes away', async () => {
  let onClose: (() => Promise<void>) | null = null;
  fake.onWindowCloseRequested.mockImplementation((cb) => {
    onClose = cb;
    return () => undefined;
  });
  fake.getDraft.mockResolvedValue({
    id: 12, accountId: 1, kind: 'new', sourceMessageId: null, to: [], cc: [], bcc: [], subject: 'Coucou',
    bodyHtml: '<p>x</p>', attachments: [], state: 'editing', lastError: null, sendAt: null, remoteUid: null, updatedAt: 0,
  });
  render(<ComposeWindow draftId={12} />);
  await screen.findByDisplayValue('Coucou');
  fireEvent.click(screen.getByRole('button', { name: 'Fermer (le brouillon est gardé)' }));
  expect(fake.windowAction).toHaveBeenCalledWith('close');
  // Tauri appelle le gestionnaire de fermeture : le brouillon est enregistré puis fermé côté cœur.
  await onClose!();
  await waitFor(() => expect(fake.closeDraft).toHaveBeenCalledWith(12));
});
