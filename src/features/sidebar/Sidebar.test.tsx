import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { account, folder } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  selectFolder: vi.fn().mockResolvedValue(undefined),
  createDraft: vi.fn(),
  openComposeWindow: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { Sidebar } from './Sidebar';

beforeEach(() => {
  localStorage.clear();
  useAppStore.setState(useAppStore.getInitialState(), true);
  useAppStore.setState({
    accounts: [account(1), account(2, { status: 'auth_error', lastError: 'NO [AUTHENTICATIONFAILED]' })],
    folders: {
      1: [folder(10, 1, 'inbox', 'Boîte de réception', 3), folder(11, 1, 'sent', 'Envoyés'), folder(12, 1, 'custom', 'Factures', 1)],
      2: [folder(20, 2, 'inbox', 'Boîte de réception', 5)],
    },
    selection: { scope: { kind: 'folder', folderId: 10 }, messageId: null },
  });
});

test('shows accounts folders and unread pills', () => {
  render(<Sidebar />);
  expect(screen.getByText('moi@gmail.com')).toBeInTheDocument();
  expect(screen.getByText('moi@yahoo.fr')).toBeInTheDocument();
  expect(screen.getAllByText('Boîte de réception')).toHaveLength(2);
  const factures = screen.getByRole('button', { name: /Factures/ });
  expect(within(factures).getByText('1')).toBeInTheDocument();
});

test('unified inbox sums unread', () => {
  render(<Sidebar />);
  const unified = screen.getByRole('button', { name: /Toutes les boîtes de réception/ });
  expect(within(unified).getByText('8')).toBeInTheDocument();
});

test('click folder selects scope', () => {
  render(<Sidebar />);
  fireEvent.click(screen.getByRole('button', { name: /Factures/ }));
  expect(useAppStore.getState().selection.scope).toEqual({ kind: 'folder', folderId: 12 });
  expect(fake.selectFolder).toHaveBeenCalledWith(12);
});

test('active folder is marked current', () => {
  render(<Sidebar />);
  const inboxes = screen.getAllByRole('button', { name: /Boîte de réception/ });
  expect(inboxes[0]).toHaveAttribute('aria-current', 'true');
});

test('auth_error shows warning card and opens password dialog', () => {
  render(<Sidebar />);
  fireEvent.click(screen.getByRole('button', { name: 'Problème de connexion : moi@yahoo.fr' }));
  expect(screen.getByText('Mot de passe refusé')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Mettre à jour' }));
  expect(useAppStore.getState().passwordDialogFor).toBe(2);
});

test('gear opens the add-account wizard', () => {
  render(<Sidebar />);
  fireEvent.click(screen.getByRole('button', { name: 'Ajouter un compte' }));
  expect(useAppStore.getState().showWizard).toBe(true);
});

test('collapsing an account hides its folders', () => {
  render(<Sidebar />);
  fireEvent.click(screen.getByRole('button', { name: /Replier moi@gmail.com/ }));
  expect(screen.queryByRole('button', { name: /Factures/ })).not.toBeInTheDocument();
  expect(JSON.parse(localStorage.getItem('illy.collapsed')!)).toEqual([1]);
});

test('Nouveau mail opens a compose window for the displayed account', async () => {
  fake.createDraft.mockReset().mockResolvedValue({ id: 42 });
  fake.openComposeWindow.mockReset().mockResolvedValue(undefined);
  useAppStore.setState({ selection: { scope: { kind: 'folder', folderId: 20 }, messageId: null } });
  render(<Sidebar />);
  fireEvent.click(screen.getByRole('button', { name: /Nouveau mail/ }));
  await vi.waitFor(() => expect(fake.openComposeWindow).toHaveBeenCalledWith(42));
  expect(fake.createDraft).toHaveBeenCalledWith('new', 2);
});
