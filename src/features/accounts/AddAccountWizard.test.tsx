import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { account } from '../../test/fixtures';

const fake = vi.hoisted(() => ({
  detectProvider: vi.fn(),
  addAccount: vi.fn(),
  listAccounts: vi.fn(),
  listFolders: vi.fn().mockResolvedValue([]),
  openUrl: vi.fn().mockResolvedValue(undefined),
  selectFolder: vi.fn(),
}));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { AddAccountWizard } from './AddAccountWizard';

const gmail = {
  provider: 'gmail',
  imap: { host: 'imap.gmail.com', port: 993, tls: 'implicit' },
  smtp: { host: 'smtp.gmail.com', port: 465, tls: 'implicit' },
};

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  useAppStore.setState({ accounts: [account(1)] });
  Object.values(fake).forEach((f) => f.mockClear());
  fake.detectProvider.mockReset().mockImplementation(async (email: string) =>
    email.endsWith('@gmail.com') ? gmail : { provider: null, imap: null, smtp: null },
  );
  fake.addAccount.mockReset().mockResolvedValue(account(5, { email: 'nouveau@gmail.com' }));
  fake.listAccounts.mockReset().mockResolvedValue([account(1), account(5)]);
});

function fillAddress(email: string) {
  fireEvent.change(screen.getByLabelText('Ton nom'), { target: { value: 'Moi' } });
  fireEvent.change(screen.getByLabelText('Adresse mail'), { target: { value: email } });
  fireEvent.click(screen.getByRole('button', { name: 'Continuer' }));
}

test('gmail shows app password instructions', async () => {
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('nouveau@gmail.com');
  expect(await screen.findByText("Gmail demande un mot de passe d'application.")).toBeInTheDocument();
  expect(screen.getByLabelText("Mot de passe d'application")).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Ouvrir la page des mots de passe d’application' }));
  expect(fake.openUrl).toHaveBeenCalledWith('https://myaccount.google.com/apppasswords');
});

test('unknown domain offers OVH or manual', async () => {
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('contact@mondomaine.fr');
  expect(await screen.findByLabelText('Hébergé chez OVH')).toBeInTheDocument();
  expect(screen.getByLabelText('Autre hébergeur (réglages manuels)')).toBeInTheDocument();
});

test('manual reveals server fields and sends them', async () => {
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('contact@mondomaine.fr');
  fireEvent.click(await screen.findByLabelText('Autre hébergeur (réglages manuels)'));
  fireEvent.change(screen.getByLabelText('Serveur IMAP'), { target: { value: 'mail.mondomaine.fr' } });
  fireEvent.change(screen.getByLabelText('Serveur SMTP'), { target: { value: 'smtp.mondomaine.fr' } });
  fireEvent.click(screen.getByRole('button', { name: 'Continuer' }));
  fireEvent.change(await screen.findByLabelText('Mot de passe'), { target: { value: 'pw' } });
  fireEvent.click(screen.getByRole('button', { name: 'Tester et ajouter' }));
  await waitFor(() => expect(fake.addAccount).toHaveBeenCalled());
  const [input, pw] = fake.addAccount.mock.calls[0];
  expect(pw).toBe('pw');
  expect(input).toMatchObject({
    email: 'contact@mondomaine.fr',
    displayName: 'Moi',
    provider: 'custom',
    imap: { host: 'mail.mondomaine.fr', port: 993, tls: 'implicit' },
    smtp: { host: 'smtp.mondomaine.fr', port: 465, tls: 'implicit' },
  });
});

test('ovh preset is used', async () => {
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('contact@mondomaine.fr');
  fireEvent.click(await screen.findByLabelText('Hébergé chez OVH'));
  fireEvent.click(screen.getByRole('button', { name: 'Continuer' }));
  fireEvent.change(await screen.findByLabelText('Mot de passe'), { target: { value: 'pw' } });
  fireEvent.click(screen.getByRole('button', { name: 'Tester et ajouter' }));
  await waitFor(() => expect(fake.addAccount).toHaveBeenCalled());
  expect(fake.addAccount.mock.calls[0][0]).toMatchObject({ provider: 'ovh', imap: { host: 'ssl0.ovh.net', port: 993 }, smtp: { host: 'ssl0.ovh.net', port: 465 } });
});

test('auth error message then back keeps password', async () => {
  fake.addAccount.mockRejectedValue({ kind: 'auth', message: 'mot de passe refusé : NO [AUTHENTICATIONFAILED]' });
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('nouveau@gmail.com');
  fireEvent.change(await screen.findByLabelText("Mot de passe d'application"), { target: { value: 'abcd' } });
  fireEvent.click(screen.getByRole('button', { name: 'Tester et ajouter' }));
  expect(await screen.findByText(/Mot de passe refusé\. Vérifie que tu as collé le mot de passe d'application/)).toBeInTheDocument();
  expect(screen.getByText(/NO \[AUTHENTICATIONFAILED\]/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Retour' }));
  expect(screen.getByLabelText("Mot de passe d'application")).toHaveValue('abcd');
});

test('success closes and reloads', async () => {
  const onClose = vi.fn();
  render(<AddAccountWizard onClose={onClose} />);
  fillAddress('nouveau@gmail.com');
  fireEvent.change(await screen.findByLabelText("Mot de passe d'application"), { target: { value: 'abcd' } });
  fireEvent.click(screen.getByRole('button', { name: 'Tester et ajouter' }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(fake.listAccounts).toHaveBeenCalled();
  expect(useAppStore.getState().accounts).toHaveLength(2);
  expect(useAppStore.getState().focusAccount).toBe(5);
});

test('no close button on first run', () => {
  useAppStore.setState({ accounts: [] });
  const { rerender } = render(<AddAccountWizard onClose={() => {}} />);
  expect(screen.queryByRole('button', { name: 'Annuler' })).not.toBeInTheDocument();
  useAppStore.setState({ accounts: [account(1)] });
  rerender(<AddAccountWizard onClose={() => {}} />);
  expect(screen.getByRole('button', { name: 'Annuler' })).toBeInTheDocument();
});

test('invalid email stays on step 1', async () => {
  render(<AddAccountWizard onClose={() => {}} />);
  fillAddress('pasunemail');
  expect(await screen.findByText('Saisis une adresse mail valide.')).toBeInTheDocument();
  expect(fake.detectProvider).not.toHaveBeenCalled();
});
