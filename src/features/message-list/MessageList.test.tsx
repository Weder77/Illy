import { fireEvent, render, screen } from '@testing-library/react';
import { beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';
import { account, fakeLayout, folder, message } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  countMessages: vi.fn(),
  listMessages: vi.fn(),
  selectFolder: vi.fn().mockResolvedValue(undefined),
  createDraft: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { MessageList } from './MessageList';
import { formatListDate } from './format';

beforeAll(fakeLayout);

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  useAppStore.setState({
    accounts: [account(1), account(2)],
    folders: { 1: [folder(10, 1, 'inbox', 'Boîte de réception', 1)], 2: [] },
    selection: { scope: { kind: 'folder', folderId: 10 }, messageId: null },
  });
  const rows = [
    message(1, { seen: false, hasAttachments: true }),
    message(2, { from: { name: null, email: 'banque@horizon.example' }, subject: '' }),
    message(3, { accountId: 2 }),
  ];
  fake.countMessages.mockReset().mockResolvedValue(rows.length);
  fake.listMessages.mockReset().mockImplementation(async (_s, offset: number, limit: number) => rows.slice(offset, offset + limit));
});

describe('formatListDate', () => {
  const now = new Date(2026, 10, 12, 15, 0);
  const ts = (d: Date) => Math.floor(d.getTime() / 1000);
  test('cases', () => {
    expect(formatListDate(ts(new Date(2026, 10, 12, 14, 32)), now)).toBe('14:32');
    expect(formatListDate(ts(new Date(2026, 10, 11, 9, 0)), now)).toBe('hier');
    expect(formatListDate(ts(new Date(2026, 10, 9, 9, 0)), now)).toBe('lundi');
    expect(formatListDate(ts(new Date(2026, 2, 4, 9, 0)), now)).toBe('04/03');
    expect(formatListDate(ts(new Date(2023, 4, 4, 9, 0)), now)).toBe('04/05/2023');
  });
});

test('renders rows from api', async () => {
  render(<MessageList />);
  expect(await screen.findByText('Sujet 1')).toBeInTheDocument();
  expect(screen.getAllByText('Marie Dupont').length).toBeGreaterThan(0);
  expect(screen.getByText('banque@horizon.example')).toBeInTheDocument();
  expect(screen.getByText('(sans objet)')).toBeInTheDocument();
});

test('unread row has dot and attachment icon', async () => {
  render(<MessageList />);
  await screen.findByText('Sujet 1');
  expect(screen.getAllByTestId('unread-dot')).toHaveLength(1);
  expect(screen.getAllByLabelText('Pièce jointe')).toHaveLength(1);
});

test('click selects message', async () => {
  render(<MessageList />);
  fireEvent.click(await screen.findByText('Sujet 1'));
  expect(useAppStore.getState().selection.messageId).toBe(1);
});

test('arrow keys move the selection', async () => {
  render(<MessageList />);
  fireEvent.click(await screen.findByText('Sujet 1'));
  fireEvent.keyDown(screen.getByRole('listbox'), { key: 'ArrowDown' });
  expect(useAppStore.getState().selection.messageId).toBe(2);
  fireEvent.keyDown(screen.getByRole('listbox'), { key: 'ArrowUp' });
  expect(useAppStore.getState().selection.messageId).toBe(1);
});

test('empty state', async () => {
  fake.countMessages.mockResolvedValue(0);
  render(<MessageList />);
  expect(await screen.findByText('Aucun mail dans cette boîte')).toBeInTheDocument();
});

test('unified view shows account dots', async () => {
  useAppStore.setState({ selection: { scope: { kind: 'unified_inbox' }, messageId: null } });
  render(<MessageList />);
  await screen.findByText('Sujet 1');
  expect(screen.getAllByTestId('account-dot')).toHaveLength(3);
});

test('right click opens menu with reply actions', async () => {
  fake.createDraft.mockReset().mockResolvedValue({ id: 55 });
  render(<MessageList />);
  fireEvent.contextMenu(await screen.findByText('Sujet 3'), { clientX: 40, clientY: 50 });
  expect(useAppStore.getState().selection.messageId).toBe(3);
  const menu = screen.getByRole('menu');
  expect(screen.getAllByRole('menuitem').map((i) => i.textContent)).toEqual(['Répondre', 'Répondre à tous', 'Transférer']);
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  fireEvent.keyDown(menu, { key: 'Enter' });
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  await vi.waitFor(() => expect(useAppStore.getState().replyTarget).toEqual({ messageId: 3, draftId: 55 }));
  expect(fake.createDraft).toHaveBeenCalledWith('forward', undefined, 3);
});

test('snippet hidden when showSnippet is false', async () => {
  useAppStore.setState((st) => ({ settings: { ...st.settings, showSnippet: false } }));
  render(<MessageList />);
  expect(await screen.findByText('Sujet 1')).toBeInTheDocument();
  expect(screen.queryByText('Extrait 1')).not.toBeInTheDocument();
});
