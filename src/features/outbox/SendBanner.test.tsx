import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { Draft } from '../../api/types';

const fake = vi.hoisted(() => ({
  cancelSend: vi.fn(),
  sendDraft: vi.fn(),
  openComposeWindow: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { SendBanner } from './SendBanner';

const draft = (id: number, over: Partial<Draft> = {}): Draft => ({
  id,
  accountId: 1,
  kind: 'new',
  sourceMessageId: null,
  to: [{ name: 'Bob', email: 'bob@ex.fr' }],
  cc: [],
  bcc: [],
  subject: 'Salut',
  bodyHtml: '<p>x</p>',
  attachments: [],
  state: 'queued',
  lastError: null,
  sendAt: null,
  remoteUid: null,
  updatedAt: 0,
  ...over,
});

const NOW = 1_800_000_000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW * 1000);
  useAppStore.setState(useAppStore.getInitialState(), true);
  for (const f of Object.values(fake)) f.mockReset();
  fake.cancelSend.mockResolvedValue(true);
  fake.sendDraft.mockResolvedValue({ sendAt: NOW + 10 });
  fake.openComposeWindow.mockResolvedValue(undefined);
});

afterEach(() => vi.useRealTimers());

test('countdown and cancel', async () => {
  useAppStore.setState({ outbox: [draft(4, { sendAt: NOW + 10 })] });
  render(<SendBanner />);
  expect(screen.getByText('Envoi dans 10 s')).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(3000));
  expect(screen.getByText('Envoi dans 7 s')).toBeInTheDocument();
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
  });
  expect(fake.cancelSend).toHaveBeenCalledWith(4);
  // Brouillon sans mail d'origine affiché : rouvert dans une fenêtre.
  expect(fake.openComposeWindow).toHaveBeenCalledWith(4);
});

test('cancel too late does not reopen the draft', async () => {
  fake.cancelSend.mockResolvedValue(false);
  useAppStore.setState({ outbox: [draft(4, { sendAt: NOW + 2 })] });
  render(<SendBanner />);
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Annuler' }));
  });
  expect(fake.openComposeWindow).not.toHaveBeenCalled();
});

test('queued drafts past their deadline are not counted down', () => {
  useAppStore.setState({ outbox: [draft(4, { sendAt: NOW - 30 })] });
  render(<SendBanner />);
  expect(screen.queryByText(/Envoi dans/)).not.toBeInTheDocument();
});

test('failure shows retry and edit', async () => {
  useAppStore.setState({
    outbox: [draft(5, { state: 'failed', lastError: 'Destinataire refusé', sendAt: NOW - 5, sourceMessageId: 9 })],
    selection: { scope: { kind: 'unified_inbox' }, messageId: 9 },
  });
  render(<SendBanner />);
  expect(screen.getByText("Échec de l'envoi à Bob : Destinataire refusé")).toBeInTheDocument();
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Réessayer' }));
  });
  expect(fake.sendDraft).toHaveBeenCalledWith(5);
  fireEvent.click(screen.getByRole('button', { name: 'Modifier' }));
  // Réponse au mail affiché : rouverte sous le mail.
  expect(useAppStore.getState().replyTarget).toEqual({ messageId: 9, draftId: 5 });
});

test('sent message disappears after 3 s', () => {
  useAppStore.setState({ outbox: [draft(4, { sendAt: NOW })] });
  render(<SendBanner />);
  act(() => {
    useAppStore.getState().applyEvent({ type: 'outbox_changed', draftId: 4, state: 'sent', sendAt: NOW, error: null });
  });
  expect(screen.getByText('Mail envoyé')).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(3100));
  expect(screen.queryByText('Mail envoyé')).not.toBeInTheDocument();
});

test('sending state is shown', () => {
  useAppStore.setState({ outbox: [draft(4, { state: 'sending', sendAt: NOW - 1 })] });
  render(<SendBanner />);
  expect(screen.getByText('Envoi en cours…')).toBeInTheDocument();
});
