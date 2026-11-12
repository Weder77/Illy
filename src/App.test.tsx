import { render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

const calls = vi.hoisted(() => [] as string[]);
vi.mock('./api', async () => {
  const { mockApi } = await vi.importActual<typeof import('./api/mock')>('./api/mock');
  return {
    api: {
      ...mockApi,
      onEvent: (cb: Parameters<typeof mockApi.onEvent>[0]) => {
        calls.push('onEvent');
        return mockApi.onEvent(cb);
      },
      listAccounts: () => {
        calls.push('listAccounts');
        return mockApi.listAccounts();
      },
    },
  };
});

import App from './App';

test('renders three panels and title', async () => {
  render(<App />);
  expect(screen.getByText('Illy')).toBeInTheDocument();
  expect(screen.getByTestId('panel-sidebar')).toBeInTheDocument();
  expect(screen.getByTestId('panel-list')).toBeInTheDocument();
  expect(screen.getByTestId('panel-reader')).toBeInTheDocument();
});

test('listens to core events before loading accounts (no lost status at startup)', async () => {
  calls.length = 0;
  render(<App />);
  await waitFor(() => expect(calls).toContain('listAccounts'));
  expect(calls.indexOf('onEvent')).toBeLessThan(calls.indexOf('listAccounts'));
});
