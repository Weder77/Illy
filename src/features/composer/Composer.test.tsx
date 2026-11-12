import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type { Account, Draft } from '../../api/types';

const fake = vi.hoisted(() => ({
  getDraft: vi.fn(),
  updateDraft: vi.fn(),
  sendDraft: vi.fn(),
  discardDraft: vi.fn(),
  closeDraft: vi.fn(),
  pickAttachments: vi.fn(),
  addAttachments: vi.fn(),
  removeAttachment: vi.fn(),
  searchContacts: vi.fn(),
  onWindowCloseRequested: vi.fn(),
  onFileDrop: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { Composer, largeAttachmentsQuestion } from './Composer';

const account = (id: number, signatureHtml?: string): Account => ({
  id,
  email: `moi${id}@ex.fr`,
  displayName: 'Moi',
  provider: 'custom',
  imap: { host: 'h', port: 993, tls: 'implicit' },
  smtp: { host: 'h', port: 465, tls: 'implicit' },
  color: '#8B5CF6',
  sortOrder: id,
  status: 'ok',
  lastError: null,
  signatureHtml,
});

const draft = (over: Partial<Draft> = {}): Draft => ({
  id: 7,
  accountId: 1,
  kind: 'reply',
  sourceMessageId: 5,
  to: [{ name: 'Bob', email: 'bob@ex.fr' }],
  cc: [],
  bcc: [],
  subject: 'Re: Salut',
  bodyHtml:
    '<p></p><div data-illy-signature><p>Signé Moi</p></div><div class="illy-quote"><p>Le jeudi, Bob a écrit :</p><blockquote><table><tr><td>Bonjour</td></tr></table></blockquote></div>',
  attachments: [],
  state: 'editing',
  lastError: null,
  sendAt: null,
  remoteUid: null,
  updatedAt: 0,
  ...over,
});

beforeEach(() => {
  for (const f of Object.values(fake)) f.mockReset();
  fake.getDraft.mockResolvedValue(draft());
  fake.updateDraft.mockImplementation(async (input) => ({ ...draft(), ...input }));
  fake.sendDraft.mockResolvedValue({ sendAt: 100 });
  fake.discardDraft.mockResolvedValue(undefined);
  fake.closeDraft.mockResolvedValue(undefined);
  fake.searchContacts.mockResolvedValue([]);
  fake.onWindowCloseRequested.mockReturnValue(() => undefined);
  fake.onFileDrop.mockReturnValue(() => undefined);
  useAppStore.setState({ accounts: [account(1, '<p>Signé Moi</p>'), account(2)] });
});

afterEach(async () => {
  vi.useRealTimers();
  // Le démontage enregistre le brouillon au tick suivant : on le laisse finir avant le test suivant.
  cleanup();
  await new Promise((r) => setTimeout(r, 5));
  vi.restoreAllMocks();
});

const html = () => screen.getByTestId('composer-editor').getAttribute('data-html') ?? '';

test('prefilled draft renders subject and recipients', async () => {
  render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  expect(await screen.findByDisplayValue('Re: Salut')).toBeInTheDocument();
  expect(screen.getByText('Bob')).toBeInTheDocument();
  expect(html()).toContain('class="illy-quote"');
  expect(fake.getDraft).toHaveBeenCalledWith(7);
});

test('the quoted original keeps its HTML (tables are not flattened)', async () => {
  render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  await screen.findByDisplayValue('Re: Salut');
  // L'éditeur restitue la citation intacte quand il réécrit le corps.
  fireEvent.change(screen.getByLabelText('Objet'), { target: { value: 'Re: Salut !' } });
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Gras' }));
  await waitFor(() => expect(html()).toContain('<table>'));
});

test('Ctrl+Enter sends and closes', async () => {
  const onClosed = vi.fn();
  render(<Composer draftId={7} variant="inline" onClosed={onClosed} />);
  const subject = await screen.findByDisplayValue('Re: Salut');
  fireEvent.keyDown(subject, { key: 'Enter', ctrlKey: true });
  await waitFor(() => expect(onClosed).toHaveBeenCalled());
  expect(fake.updateDraft).toHaveBeenCalledWith(expect.objectContaining({ id: 7, subject: 'Re: Salut' }));
  expect(fake.sendDraft).toHaveBeenCalledWith(7);
  expect(fake.updateDraft.mock.invocationCallOrder[0]).toBeLessThan(fake.sendDraft.mock.invocationCallOrder[0]);
});

test('text typed in the To field is kept when sending with Ctrl+Enter', async () => {
  fake.getDraft.mockResolvedValue(draft({ to: [], kind: 'new' }));
  render(<Composer draftId={7} variant="window" onClosed={() => undefined} />);
  const to = await screen.findByLabelText('À');
  fireEvent.change(to, { target: { value: 'alice@ex.fr' } });
  fireEvent.keyDown(to, { key: 'Enter', ctrlKey: true });
  await waitFor(() => expect(fake.sendDraft).toHaveBeenCalled());
  expect(fake.updateDraft).toHaveBeenCalledWith(expect.objectContaining({ to: [{ name: null, email: 'alice@ex.fr' }] }));
});

test('send error shows message under recipients', async () => {
  fake.sendDraft.mockRejectedValue({ kind: 'invalid_input', message: 'Adresse invalide : pas une adresse' });
  const onClosed = vi.fn();
  const user = userEvent.setup();
  render(<Composer draftId={7} variant="inline" onClosed={onClosed} />);
  await screen.findByDisplayValue('Re: Salut');
  await user.click(screen.getByRole('button', { name: /Envoyer/ }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Adresse invalide : pas une adresse');
  expect(onClosed).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: /Envoyer/ })).toBeEnabled();
});

test('over 25 MB asks confirmation and does not send when refused', async () => {
  const big = 31 * 1024 * 1024;
  fake.getDraft.mockResolvedValue(
    draft({ attachments: [{ filename: 'video.mp4', mimeType: 'video/mp4', size: big, source: { kind: 'file', path: 'C:/v.mp4' } }] }),
  );
  const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
  const user = userEvent.setup();
  render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  await screen.findByText('video.mp4');
  await user.click(screen.getByRole('button', { name: /Envoyer/ }));
  expect(confirm).toHaveBeenCalledWith(
    'Les pièces jointes font 31 Mo. Gmail et la plupart des serveurs refusent au-delà de 25 Mo. Envoyer quand même ?',
  );
  expect(largeAttachmentsQuestion(big)).toBe(confirm.mock.calls[0][0]);
  expect(fake.sendDraft).not.toHaveBeenCalled();
});

test('autosave calls updateDraft after 2 s', async () => {
  render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  const subject = await screen.findByDisplayValue('Re: Salut');
  vi.useFakeTimers();
  fireEvent.change(subject, { target: { value: 'Re: Salut, ça va ?' } });
  act(() => vi.advanceTimersByTime(1900));
  expect(fake.updateDraft).not.toHaveBeenCalled();
  await act(async () => {
    vi.advanceTimersByTime(200);
  });
  expect(fake.updateDraft).toHaveBeenCalledTimes(1);
  expect(fake.updateDraft).toHaveBeenCalledWith(expect.objectContaining({ subject: 'Re: Salut, ça va ?' }));
  vi.useRealTimers();
  expect(await screen.findByText('Brouillon enregistré')).toBeInTheDocument();
});

test('discard calls discardDraft', async () => {
  const onClosed = vi.fn();
  const user = userEvent.setup();
  render(<Composer draftId={7} variant="inline" onClosed={onClosed} />);
  await screen.findByDisplayValue('Re: Salut');
  await user.click(screen.getByRole('button', { name: 'Supprimer le brouillon' }));
  await waitFor(() => expect(onClosed).toHaveBeenCalled());
  expect(fake.discardDraft).toHaveBeenCalledWith(7);
  expect(fake.closeDraft).not.toHaveBeenCalled();
});

test('unmounting the inline reply saves and closes the draft', async () => {
  const { unmount } = render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  const subject = await screen.findByDisplayValue('Re: Salut');
  fireEvent.change(subject, { target: { value: 'Re: modifié' } });
  unmount();
  await waitFor(() => expect(fake.closeDraft).toHaveBeenCalledWith(7));
  expect(fake.updateDraft).toHaveBeenCalledWith(expect.objectContaining({ subject: 'Re: modifié' }));
  expect(fake.updateDraft.mock.invocationCallOrder[0]).toBeLessThan(fake.closeDraft.mock.invocationCallOrder[0]);
});

test('changing the From account replaces the signature', async () => {
  render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  await screen.findByDisplayValue('Re: Salut');
  fireEvent.change(screen.getByLabelText('De'), { target: { value: '2' } });
  await waitFor(() => expect(html()).not.toContain('Signé Moi'));
  expect(html()).toContain('illy-quote');
  fireEvent.change(screen.getByLabelText('De'), { target: { value: '1' } });
  await waitFor(() => expect(html()).toContain('Signé Moi'));
  expect(html().indexOf('Signé Moi')).toBeLessThan(html().indexOf('illy-quote'));
});

test('an image over 5 MB is refused', async () => {
  const user = userEvent.setup();
  const { container } = render(<Composer draftId={7} variant="inline" onClosed={() => undefined} />);
  await screen.findByDisplayValue('Re: Salut');
  const input = container.querySelector('input[type="file"]') as HTMLInputElement;
  const big = new File([new Uint8Array(5 * 1024 * 1024 + 1)], 'photo.png', { type: 'image/png' });
  await user.upload(input, big);
  expect(await screen.findByText('Image trop lourde (5 Mo au plus)')).toBeInTheDocument();
});
