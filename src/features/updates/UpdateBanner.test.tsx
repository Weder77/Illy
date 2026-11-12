import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const fake = vi.hoisted(() => ({ checkUpdate: vi.fn(), downloadUpdate: vi.fn(), applyUpdate: vi.fn(), closeComposeWindows: vi.fn() }));
vi.mock('../../api', () => ({ api: fake }));
const saved = vi.hoisted(() => ({ order: [] as string[] }));
vi.mock('../composer/registry', () => ({
  saveOpenComposers: vi.fn(async () => {
    saved.order.push('save');
  }),
}));

import { useAppStore } from '../../state/store';
import { UpdateBanner } from './UpdateBanner';

beforeEach(() => {
  vi.useFakeTimers();
  useAppStore.setState(useAppStore.getInitialState(), true);
  fake.checkUpdate.mockReset().mockResolvedValue({ version: '0.3.1', notes: null });
  saved.order = [];
  fake.downloadUpdate.mockReset().mockImplementation(async () => {
    saved.order.push('download');
  });
  fake.closeComposeWindows.mockReset().mockImplementation(async () => {
    saved.order.push('close');
  });
  fake.applyUpdate.mockReset().mockImplementation(async () => {
    saved.order.push('apply');
  });
});

afterEach(() => vi.useRealTimers());

const startup = async () => {
  render(<UpdateBanner />);
  await act(async () => {
    vi.advanceTimersByTime(10_000);
  });
};

test('checks 10 s after start, never before', async () => {
  render(<UpdateBanner />);
  act(() => vi.advanceTimersByTime(9_900));
  expect(fake.checkUpdate).not.toHaveBeenCalled();
  await act(async () => {
    vi.advanceTimersByTime(200);
  });
  expect(fake.checkUpdate).toHaveBeenCalledTimes(1);
  expect(screen.getByText('Illy 0.3.1 est disponible')).toBeInTheDocument();
  expect(fake.downloadUpdate).not.toHaveBeenCalled();
  expect(fake.applyUpdate).not.toHaveBeenCalled();
});

test('later hides until next start', async () => {
  await startup();
  fireEvent.click(screen.getByRole('button', { name: 'Plus tard' }));
  expect(screen.queryByText(/est disponible/)).not.toBeInTheDocument();
  expect(fake.downloadUpdate).not.toHaveBeenCalled();
});

test('install shows progress, saves drafts, closes compose windows, then installs', async () => {
  let report: (p: number | null) => void = () => undefined;
  let finish: () => void = () => undefined;
  fake.downloadUpdate.mockImplementation((onProgress: (p: number | null) => void) => {
    report = onProgress;
    saved.order.push('download');
    return new Promise<void>((r) => (finish = r));
  });
  await startup();
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Installer' }));
  });
  expect(fake.downloadUpdate).toHaveBeenCalledTimes(1);
  act(() => report(42));
  expect(screen.getByText('Téléchargement de la mise à jour… 42 %')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Installer' })).not.toBeInTheDocument();
  expect(fake.applyUpdate).not.toHaveBeenCalled();
  await act(async () => finish());
  expect(saved.order).toEqual(['download', 'save', 'close', 'apply']);
});

test('install is postponed while a mail is being sent', async () => {
  await startup();
  useAppStore.setState({ outbox: [{ id: 1, state: 'sending' } as never] });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Installer' }));
  });
  expect(fake.downloadUpdate).not.toHaveBeenCalled();
  expect(screen.getByRole('alert')).toHaveTextContent("un mail est en cours d'envoi");
});

test('failure message', async () => {
  fake.downloadUpdate.mockRejectedValue(new Error('signature invalide'));
  await startup();
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Installer' }));
  });
  expect(screen.getByRole('alert')).toHaveTextContent("La mise à jour n'a pas pu être installée : signature invalide");
  expect(screen.getByRole('button', { name: 'Réessayer' })).toBeInTheDocument();
});

test('nothing shown when up to date or when the check fails', async () => {
  fake.checkUpdate.mockResolvedValueOnce(null);
  const { unmount } = render(<UpdateBanner />);
  await act(async () => {
    vi.advanceTimersByTime(10_000);
  });
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
  unmount();
  useAppStore.setState(useAppStore.getInitialState(), true);
  fake.checkUpdate.mockRejectedValueOnce(new Error('hors ligne'));
  render(<UpdateBanner />);
  await act(async () => {
    vi.advanceTimersByTime(10_000);
  });
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.queryByText(/disponible/)).not.toBeInTheDocument();
});
