import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { defaultSettings } from '../../api/types';
import { account } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  setSetting: vi.fn(),
  reorderAccounts: vi.fn(),
  updateAccount: vi.fn(),
  updateServers: vi.fn(),
  removeAccount: vi.fn(),
  listImageAllowlist: vi.fn(),
  removeFromImageAllowlist: vi.fn(),
  storageUsage: vi.fn(),
  clearCache: vi.fn(),
  openLogsFolder: vi.fn(),
  appInfo: vi.fn(),
  checkUpdate: vi.fn(),
  downloadUpdate: vi.fn(),
  applyUpdate: vi.fn(),
  closeComposeWindows: vi.fn(),
  listAccounts: vi.fn(),
  listFolders: vi.fn(),
  getSettings: vi.fn(),
  listOutbox: vi.fn(),
  selectFolder: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { SettingsPanel } from './SettingsPanel';

const gmail = account(1, { provider: 'gmail', email: 'moi@gmail.com' });
const ovh = account(2, { provider: 'ovh', email: 'contact@mondomaine.fr', imap: { host: 'ssl0.ovh.net', port: 993, tls: 'implicit' }, smtp: { host: 'ssl0.ovh.net', port: 465, tls: 'implicit' } });

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  useAppStore.setState({ accounts: [gmail, ovh], showSettings: true, settingsSection: 'accounts' });
  for (const f of Object.values(fake)) f.mockReset();
  fake.setSetting.mockImplementation(async (key: string, value: unknown) => ({ ...defaultSettings, [key === 'theme' ? 'theme' : key]: value }));
  fake.reorderAccounts.mockImplementation(async (ids: number[]) => ids.map((id) => (id === 1 ? gmail : ovh)));
  fake.updateAccount.mockImplementation(async (id: number, patch: object) => ({ ...(id === 1 ? gmail : ovh), ...patch }));
  fake.listImageAllowlist.mockResolvedValue(['anna@ex.fr', 'news@ex.fr']);
  fake.removeFromImageAllowlist.mockResolvedValue(undefined);
  fake.storageUsage.mockResolvedValue({ database: 48_200_000, drafts: 1_843_200, temporary: 0 });
  fake.appInfo.mockResolvedValue({ version: '0.3.0', logsDir: 'C:\\logs' });
  fake.listAccounts.mockResolvedValue([gmail, ovh]);
  fake.listFolders.mockResolvedValue([]);
  fake.getSettings.mockResolvedValue(defaultSettings);
  fake.listOutbox.mockResolvedValue([]);
});

afterEach(() => vi.restoreAllMocks());

test('Escape closes the panel', () => {
  render(<SettingsPanel />);
  expect(screen.getByRole('dialog', { name: 'Paramètres' })).toBeInTheDocument();
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(useAppStore.getState().showSettings).toBe(false);
});

test('sections navigate', async () => {
  render(<SettingsPanel />);
  const nav = screen.getByRole('navigation');
  for (const name of ['Comptes', 'Apparence', 'Lecture et rédaction', 'Mises à jour', 'À propos']) {
    expect(within(nav).getByRole('button', { name })).toBeInTheDocument();
  }
  fireEvent.click(within(nav).getByRole('button', { name: 'Apparence' }));
  expect(await screen.findByRole('radiogroup', { name: 'Thème' })).toBeInTheDocument();
  expect(within(nav).getByRole('button', { name: 'Apparence' })).toHaveAttribute('aria-current', 'page');
});

test('theme choice calls setSetting', async () => {
  useAppStore.setState({ settingsSection: 'appearance' });
  render(<SettingsPanel />);
  fireEvent.click(screen.getByRole('radio', { name: 'Clair' }));
  await waitFor(() => expect(fake.setSetting).toHaveBeenCalledWith('theme', 'light'));
  expect(useAppStore.getState().settings.theme).toBe('light');
  fireEvent.click(screen.getByRole('radio', { name: 'Rose' }));
  await waitFor(() => expect(fake.setSetting).toHaveBeenCalledWith('accent', 'pink'));
  fireEvent.click(screen.getByRole('checkbox', { name: "Afficher l'extrait dans la liste" }));
  await waitFor(() => expect(fake.setSetting).toHaveBeenCalledWith('show_snippet', false));
});

test('dragging with the pointer reorders accounts (no HTML5 drag: Tauri keeps file drops on Windows)', async () => {
  render(<SettingsPanel />);
  const list = screen.getByRole('list', { name: 'Comptes' });
  const items = within(list).getAllByRole('listitem');
  items.forEach((li, i) => {
    li.getBoundingClientRect = () => ({ top: i * 40, bottom: i * 40 + 40, height: 40, left: 0, right: 200, width: 200, x: 0, y: i * 40, toJSON: () => ({}) }) as DOMRect;
  });
  expect(items[0]).not.toHaveAttribute('draggable', 'true');
  fireEvent.pointerDown(items[1], { clientY: 60, pointerId: 1, button: 0 });
  fireEvent.pointerMove(items[1], { clientY: 30, pointerId: 1 });
  fireEvent.pointerMove(items[1], { clientY: 8, pointerId: 1 });
  fireEvent.pointerUp(items[1], { clientY: 8, pointerId: 1 });
  await waitFor(() => expect(fake.reorderAccounts).toHaveBeenCalledWith([2, 1]));
  expect(useAppStore.getState().accounts.map((a) => a.id)).toEqual([2, 1]);
});

test('a simple click selects without reordering', async () => {
  render(<SettingsPanel />);
  const list = screen.getByRole('list', { name: 'Comptes' });
  const second = within(list).getAllByRole('listitem')[1];
  fireEvent.pointerDown(second, { clientY: 60, pointerId: 1, button: 0 });
  fireEvent.pointerUp(second, { clientY: 61, pointerId: 1 });
  fireEvent.click(within(second).getByRole('button'));
  expect(fake.reorderAccounts).not.toHaveBeenCalled();
  expect(screen.getByText('contact@mondomaine.fr', { selector: 'div' })).toBeInTheDocument();
});

test('Alt+arrow reorders accounts from the keyboard', async () => {
  render(<SettingsPanel />);
  const list = screen.getByRole('list', { name: 'Comptes' });
  fireEvent.keyDown(within(list).getByRole('button', { name: /moi@gmail.com/ }), { key: 'ArrowDown', altKey: true });
  await waitFor(() => expect(fake.reorderAccounts).toHaveBeenCalledWith([2, 1]));
});

test('delete asks the exact confirmation', async () => {
  const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
  render(<SettingsPanel />);
  fireEvent.click(screen.getByRole('button', { name: 'Supprimer ce compte' }));
  expect(confirm).toHaveBeenCalledWith("Supprimer moi@gmail.com d'Illy ? Tes mails restent sur le serveur ; seules les données de ce PC sont effacées.");
  expect(fake.removeAccount).not.toHaveBeenCalled();
  confirm.mockReturnValue(true);
  fake.removeAccount.mockResolvedValue(undefined);
  fake.listAccounts.mockResolvedValue([ovh]);
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Supprimer ce compte' }));
  });
  expect(fake.removeAccount).toHaveBeenCalledWith(1);
  await waitFor(() => expect(useAppStore.getState().accounts.map((a) => a.id)).toEqual([2]));
});

test('servers read-only for gmail, editable for ovh', async () => {
  const user = userEvent.setup();
  render(<SettingsPanel />);
  expect(screen.queryByRole('button', { name: 'Tester et enregistrer' })).not.toBeInTheDocument();
  expect(screen.getByText('IMAP h:993 · SMTP h:465')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: /contact@mondomaine.fr/ }));
  const host = screen.getByLabelText('Serveur IMAP');
  await user.clear(host);
  await user.type(host, 'imap.mondomaine.fr');
  fake.updateServers.mockResolvedValue({ ...ovh, imap: { ...ovh.imap, host: 'imap.mondomaine.fr' } });
  await user.click(screen.getByRole('button', { name: 'Tester et enregistrer' }));
  expect(fake.updateServers).toHaveBeenCalledWith(2, { host: 'imap.mondomaine.fr', port: 993, tls: 'implicit' }, ovh.smtp);
  expect(await screen.findByText('Serveurs enregistrés')).toBeInTheDocument();
});

test('display name and color are saved', async () => {
  const user = userEvent.setup();
  render(<SettingsPanel />);
  const name = screen.getByLabelText('Nom affiché');
  await user.clear(name);
  await user.type(name, 'Élodie');
  await user.tab();
  await waitFor(() => expect(fake.updateAccount).toHaveBeenCalledWith(1, { displayName: 'Élodie' }));
  await user.click(screen.getByRole('radio', { name: 'Couleur #EC4899' }));
  await waitFor(() => expect(fake.updateAccount).toHaveBeenCalledWith(1, { color: '#EC4899' }));
});

test('the display name is saved when the panel closes with Escape', async () => {
  const user = userEvent.setup();
  const { unmount } = render(<SettingsPanel />);
  const name = screen.getByLabelText('Nom affiché');
  await user.clear(name);
  await user.type(name, 'Élodie');
  unmount();
  await waitFor(() => expect(fake.updateAccount).toHaveBeenCalledWith(1, { displayName: 'Élodie' }));
});

test('a signature with a remote image keeps it', async () => {
  useAppStore.setState({ accounts: [{ ...gmail, signatureHtml: '<p>Moi</p><p><img data-blocked-src="https://ex.fr/logo.png" alt="logo"></p>' }, ovh] });
  render(<SettingsPanel />);
  await waitFor(() => expect(screen.getByTestId('signature-editor').querySelector('img[alt="logo"]')?.getAttribute('src')).toBe('https://ex.fr/logo.png'));
});

test('remove sender from allowlist', async () => {
  useAppStore.setState({ settingsSection: 'reading' });
  render(<SettingsPanel />);
  expect(await screen.findByText('anna@ex.fr')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Retirer anna@ex.fr' }));
  await waitFor(() => expect(fake.removeFromImageAllowlist).toHaveBeenCalledWith('anna@ex.fr'));
  await waitFor(() => expect(screen.queryByText('anna@ex.fr')).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole('radio', { name: '30 s' }));
  await waitFor(() => expect(fake.setSetting).toHaveBeenCalledWith('undo_send_seconds', 30));
});

test('clear cache refusal is shown', async () => {
  useAppStore.setState({ settingsSection: 'about' });
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  fake.clearCache.mockRejectedValue({ kind: 'invalid_input', message: 'Attends la fin des envois en cours' });
  render(<SettingsPanel />);
  expect(await screen.findByText('0.3.0')).toBeInTheDocument();
  expect(screen.getByText(/46 Mo|45,9 Mo|46,0 Mo/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Vider le cache local' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Attends la fin des envois en cours');
});

test('updates section checks and offers to install', async () => {
  useAppStore.setState({ settingsSection: 'updates' });
  fake.checkUpdate.mockResolvedValue({ version: '0.3.1', notes: null });
  fake.downloadUpdate.mockResolvedValue(undefined);
  fake.closeComposeWindows.mockResolvedValue(undefined);
  fake.applyUpdate.mockResolvedValue(undefined);
  render(<SettingsPanel />);
  fireEvent.click(screen.getByRole('button', { name: 'Rechercher des mises à jour' }));
  expect(await screen.findByText('Illy 0.3.1 est disponible.')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Installer' }));
  await waitFor(() => expect(fake.applyUpdate).toHaveBeenCalled());
});
