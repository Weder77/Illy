import { render, screen } from '@testing-library/react';
import { TitleBar } from './TitleBar';
import type { Account } from '../api/types';

const acc = (status: Account['status']): Account => ({
  id: 1,
  email: 'a@b.fr',
  displayName: 'A',
  provider: 'custom',
  imap: { host: 'h', port: 993, tls: 'implicit' },
  smtp: { host: 'h', port: 465, tls: 'implicit' },
  color: '#8B5CF6',
  sortOrder: 0,
  status,
  lastError: null,
});

test('offline pill when all accounts offline', () => {
  const { rerender } = render(<TitleBar accounts={[acc('offline'), { ...acc('offline'), id: 2 }]} />);
  expect(screen.getByText('Hors ligne')).toBeInTheDocument();
  rerender(<TitleBar accounts={[acc('offline'), { ...acc('ok'), id: 2 }]} />);
  expect(screen.queryByText('Hors ligne')).not.toBeInTheDocument();
  rerender(<TitleBar accounts={[]} />);
  expect(screen.queryByText('Hors ligne')).not.toBeInTheDocument();
});
