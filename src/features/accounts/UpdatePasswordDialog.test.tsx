import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { account } from '../../test/fixtures';

const fake = vi.hoisted(() => ({ updatePassword: vi.fn(), selectFolder: vi.fn() }));
vi.mock('../../api', () => ({ api: fake }));

import { useAppStore } from '../../state/store';
import { UpdatePasswordDialog } from './UpdatePasswordDialog';

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
  useAppStore.setState({ accounts: [account(2, { email: 'moi@yahoo.fr', status: 'auth_error' })] });
  fake.updatePassword.mockReset().mockResolvedValue(undefined);
});

test('saves and closes', async () => {
  const onClose = vi.fn();
  render(<UpdatePasswordDialog accountId={2} onClose={onClose} />);
  expect(screen.getByText('Mettre à jour le mot de passe de moi@yahoo.fr')).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText('Nouveau mot de passe'), { target: { value: 'neuf' } });
  fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(fake.updatePassword).toHaveBeenCalledWith(2, 'neuf');
});

test('shows auth error', async () => {
  fake.updatePassword.mockRejectedValue({ kind: 'auth', message: 'mot de passe refusé : NO' });
  const onClose = vi.fn();
  render(<UpdatePasswordDialog accountId={2} onClose={onClose} />);
  fireEvent.change(screen.getByLabelText('Nouveau mot de passe'), { target: { value: 'faux' } });
  fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
  expect(await screen.findByText(/Mot de passe refusé\./)).toBeInTheDocument();
  expect(onClose).not.toHaveBeenCalled();
});
