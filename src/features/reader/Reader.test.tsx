import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import type { Draft, MessageDetail } from '../../api/types';
import { folder, message } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  openMessage: vi.fn(),
  allowImagesFrom: vi.fn().mockResolvedValue(undefined),
  openAttachment: vi.fn().mockResolvedValue(undefined),
  saveAttachment: vi.fn().mockResolvedValue(undefined),
  openUrl: vi.fn(),
  selectFolder: vi.fn(),
  draftForMessage: vi.fn(),
  draftFromMessage: vi.fn(),
  createDraft: vi.fn(),
  openComposeWindow: vi.fn(),
  getDraft: vi.fn(),
  updateDraft: vi.fn(),
  closeDraft: vi.fn(),
  searchContacts: vi.fn(),
  onFileDrop: vi.fn(),
  onWindowCloseRequested: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { formatSize, Reader } from './Reader';

const detail = (over: Partial<MessageDetail> = {}): MessageDetail => ({
  summary: { ...message(7), subject: 'Devis cuisine', seen: true, date: 1_794_493_920 },
  to: [{ name: 'Moi', email: 'moi@gmail.com' }],
  cc: [],
  body: { kind: 'html', content: '<p>Bonjour</p>' },
  attachments: [{ partPath: '2', filename: 'devis.pdf', mimeType: 'application/pdf', size: 253_952, canOpen: true }],
  imagesBlocked: false,
  remoteImagesAllowed: false,
  degraded: false,
  ...over,
});

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  Object.values(fake).forEach((f) => f.mockClear());
  fake.openMessage.mockReset().mockResolvedValue(detail());
  fake.draftForMessage.mockReset().mockResolvedValue(null);
  fake.createDraft.mockReset().mockResolvedValue(replyDraft);
  fake.getDraft.mockReset().mockResolvedValue(replyDraft);
  fake.updateDraft.mockReset().mockImplementation(async (i) => ({ ...replyDraft, ...i }));
  fake.closeDraft.mockReset().mockResolvedValue(undefined);
  fake.searchContacts.mockReset().mockResolvedValue([]);
  fake.onFileDrop.mockReset().mockReturnValue(() => undefined);
  fake.onWindowCloseRequested.mockReset().mockReturnValue(() => undefined);
  fake.draftFromMessage.mockReset().mockResolvedValue({ ...replyDraft, id: 31, kind: 'new' });
  fake.openComposeWindow.mockReset().mockResolvedValue(undefined);
});

const replyDraft: Draft = {
  id: 30,
  accountId: 1,
  kind: 'reply',
  sourceMessageId: 7,
  to: [{ name: 'Marie Dupont', email: 'marie@exemple.fr' }],
  cc: [],
  bcc: [],
  subject: 'Re: Devis cuisine',
  bodyHtml: '<p></p>',
  attachments: [],
  state: 'editing',
  lastError: null,
  sendAt: null,
  remoteUid: null,
  updatedAt: 0,
};

const select = (id = 7) => useAppStore.setState({ selection: { scope: { kind: 'unified_inbox' }, messageId: id } });

test('empty state when nothing selected', () => {
  render(<Reader />);
  expect(screen.getByText("Sélectionne un mail pour l'afficher")).toBeInTheDocument();
});

test('shows sender subject and date', async () => {
  select();
  render(<Reader />);
  expect(await screen.findByRole('heading', { name: 'Devis cuisine' })).toBeInTheDocument();
  expect(screen.getByText('Marie Dupont')).toBeInTheDocument();
  expect(screen.getByText('<marie@exemple.fr>')).toBeInTheDocument();
  expect(screen.getByText(/à Moi/)).toBeInTheDocument();
  expect(screen.getByText(/novembre 2026/)).toBeInTheDocument();
  expect(fake.openMessage).toHaveBeenCalledWith(7, false);
});

test('lot C actions are visible but disabled', async () => {
  select();
  render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  for (const name of ['Marquer non lu', 'Déplacer', 'Spam', 'Supprimer']) {
    expect(screen.getByRole('button', { name })).toBeDisabled();
  }
});

test('reply buttons enabled and open inline composer', async () => {
  select();
  render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  for (const name of ['Répondre', 'Répondre à tous', 'Transférer']) {
    expect(screen.getByRole('button', { name })).toBeEnabled();
  }
  fireEvent.click(screen.getByRole('button', { name: 'Répondre à tous' }));
  expect(await screen.findByDisplayValue('Re: Devis cuisine')).toBeInTheDocument();
  expect(fake.createDraft).toHaveBeenCalledWith('reply_all', undefined, 7);
  expect(useAppStore.getState().replyTarget).toEqual({ messageId: 7, draftId: 30 });
  // Fermer l'éditeur garde le brouillon.
  fireEvent.click(screen.getByRole('button', { name: 'Fermer la réponse' }));
  await waitFor(() => expect(fake.closeDraft).toHaveBeenCalledWith(30));
});

test('resume reply shown when a draft exists', async () => {
  fake.draftForMessage.mockResolvedValue(replyDraft);
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: 'Reprendre la réponse' }));
  expect(fake.draftForMessage).toHaveBeenCalledWith(7);
  expect(await screen.findByDisplayValue('Re: Devis cuisine')).toBeInTheDocument();
  expect(fake.createDraft).not.toHaveBeenCalled();
  expect(screen.queryByRole('button', { name: 'Reprendre la réponse' })).not.toBeInTheDocument();
});

test('edit draft button in drafts folder', async () => {
  useAppStore.setState({ folders: { 1: [folder(100, 1, 'drafts', 'Brouillons')] } });
  fake.openMessage.mockResolvedValue(detail({ summary: { ...message(7), folderId: 100, subject: 'Brouillon', seen: true } }));
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: 'Modifier ce brouillon' }));
  await waitFor(() => expect(fake.openComposeWindow).toHaveBeenCalledWith(31));
  expect(fake.draftFromMessage).toHaveBeenCalledWith(7);
});

test('no edit draft button outside drafts', async () => {
  useAppStore.setState({ folders: { 1: [folder(100, 1, 'drafts', 'Brouillons')] } });
  select();
  render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  expect(screen.queryByRole('button', { name: 'Modifier ce brouillon' })).not.toBeInTheDocument();
});

test('images banner shows and show-images reopens with true', async () => {
  fake.openMessage.mockResolvedValueOnce(detail({ imagesBlocked: true })).mockResolvedValueOnce(detail());
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: 'Afficher les images' }));
  await waitFor(() => expect(fake.openMessage).toHaveBeenLastCalledWith(7, true));
  await waitFor(() => expect(screen.queryByText(/Images bloquées/)).not.toBeInTheDocument());
});

test('always-for-sender calls allowImagesFrom', async () => {
  fake.openMessage.mockResolvedValueOnce(detail({ imagesBlocked: true })).mockResolvedValueOnce(detail());
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: 'Toujours pour cet expéditeur' }));
  await waitFor(() => expect(fake.allowImagesFrom).toHaveBeenCalledWith('marie@exemple.fr'));
  await waitFor(() => expect(fake.openMessage).toHaveBeenCalledTimes(2));
});

test('attachment opens; save-as is available', async () => {
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: /^devis\.pdf/ }));
  expect(fake.openAttachment).toHaveBeenCalledWith(7, '2');
  fireEvent.click(screen.getByRole('button', { name: 'Enregistrer sous… devis.pdf' }));
  expect(fake.saveAttachment).toHaveBeenCalledWith(7, '2');
  expect(screen.getByText('248 Ko')).toBeInTheDocument();
});

test('dangerous attachment cannot be opened', async () => {
  fake.openMessage.mockResolvedValue(
    detail({ attachments: [{ partPath: '3', filename: 'setup.exe', mimeType: 'application/octet-stream', size: 10, canOpen: false }] }),
  );
  select();
  render(<Reader />);
  fireEvent.click(await screen.findByRole('button', { name: /^setup\.exe/ }));
  expect(fake.saveAttachment).toHaveBeenCalledWith(7, '3');
  expect(fake.openAttachment).not.toHaveBeenCalled();
});

test('degraded note', async () => {
  fake.openMessage.mockResolvedValue(detail({ degraded: true }));
  select();
  render(<Reader />);
  expect(await screen.findByText("Ce mail n'a pas pu être affiché correctement")).toBeInTheDocument();
});

test('error message on failure', async () => {
  fake.openMessage.mockRejectedValue({ kind: 'network', message: 'serveur injoignable' });
  select();
  render(<Reader />);
  expect(await screen.findByText(/Impossible d'ouvrir ce mail : serveur injoignable/)).toBeInTheDocument();
});

test('formatSize', () => {
  expect(formatSize(512)).toBe('512 o');
  expect(formatSize(253_952)).toBe('248 Ko');
  expect(formatSize(3_355_443)).toBe('3,2 Mo');
});

test('iframe csp stays closed unless remote images were explicitly allowed', async () => {
  fake.openMessage.mockResolvedValue(detail({ imagesBlocked: false, remoteImagesAllowed: false }));
  select();
  const { container } = render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  expect(container.querySelector('iframe')!.getAttribute('srcdoc')).toContain('img-src data:;');
});

test('iframe csp opens when remote images are allowed', async () => {
  fake.openMessage.mockResolvedValue(detail({ remoteImagesAllowed: true }));
  select();
  const { container } = render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  expect(container.querySelector('iframe')!.getAttribute('srcdoc')).toContain('img-src data: https: http:;');
});
