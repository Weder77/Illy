import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import type { MessageDetail } from '../../api/types';
import { message } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  openMessage: vi.fn(),
  allowImagesFrom: vi.fn().mockResolvedValue(undefined),
  openAttachment: vi.fn().mockResolvedValue(undefined),
  saveAttachment: vi.fn().mockResolvedValue(undefined),
  openUrl: vi.fn(),
  selectFolder: vi.fn(),
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
  degraded: false,
  ...over,
});

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  Object.values(fake).forEach((f) => f.mockClear());
  fake.openMessage.mockReset().mockResolvedValue(detail());
});

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

test('action buttons are visible but disabled', async () => {
  select();
  render(<Reader />);
  await screen.findByRole('heading', { name: 'Devis cuisine' });
  for (const name of ['Répondre', 'Répondre à tous', 'Transférer', 'Marquer non lu', 'Déplacer', 'Spam', 'Supprimer']) {
    expect(screen.getByRole('button', { name })).toBeDisabled();
  }
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
