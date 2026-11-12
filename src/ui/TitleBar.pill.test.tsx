import { render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import type { Draft } from '../api/types';

vi.mock('../api', () => ({ api: { windowAction: vi.fn() } }));

import { TitleBar } from './TitleBar';

const queued = (id: number, sendAt: number): Draft => ({
  id,
  accountId: 1,
  kind: 'new',
  sourceMessageId: null,
  to: [{ name: null, email: 'b@ex.fr' }],
  cc: [],
  bcc: [],
  subject: '',
  bodyHtml: '',
  attachments: [],
  state: 'queued',
  lastError: null,
  sendAt,
  remoteUid: null,
  updatedAt: 0,
});

test('pending sends pill', () => {
  const now = Math.floor(Date.now() / 1000);
  const { rerender } = render(<TitleBar accounts={[]} outbox={[queued(1, now - 60), queued(2, now + 8)]} />);
  // Le mail encore dans son délai d'annulation ne compte pas.
  expect(screen.getByText('1 envoi en attente')).toBeInTheDocument();
  rerender(<TitleBar accounts={[]} outbox={[queued(1, now - 60), queued(3, now - 5)]} />);
  expect(screen.getByText('2 envois en attente')).toBeInTheDocument();
  rerender(<TitleBar accounts={[]} outbox={[queued(2, now + 8)]} />);
  expect(screen.queryByText(/en attente/)).not.toBeInTheDocument();
});
