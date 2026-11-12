import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import type { Address } from '../../api/types';

const fake = vi.hoisted(() => ({ searchContacts: vi.fn() }));
vi.mock('../../api', () => ({ api: fake }));

import { parseAddress } from './address';
import { RecipientField } from './RecipientField';

let last: Address[] = [];
function Harness({ initial = [] as Address[] }) {
  const [v, setV] = useState<Address[]>(initial);
  return (
    <RecipientField
      label="À"
      value={v}
      onChange={(x) => {
        last = x;
        setV(x);
      }}
    />
  );
}

beforeEach(() => {
  last = [];
  fake.searchContacts.mockReset().mockResolvedValue([]);
});

test('parseAddress', () => {
  expect(parseAddress('  ')).toBeNull();
  expect(parseAddress('Bob <bob@ex.fr>')).toEqual({ name: 'Bob', email: 'bob@ex.fr' });
  expect(parseAddress('"Dupont, Marie" <m@ex.fr>')).toEqual({ name: 'Dupont, Marie', email: 'm@ex.fr' });
  expect(parseAddress('a@ex.fr,')).toEqual({ name: null, email: 'a@ex.fr' });
});

test('comma and Enter create chips', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  const input = screen.getByRole('combobox');
  await user.type(input, 'a@ex.fr,');
  await user.type(input, 'Bob <bob@ex.fr>{Enter}');
  await user.type(input, 'c@ex.fr;');
  expect(last.map((a) => a.email)).toEqual(['a@ex.fr', 'bob@ex.fr', 'c@ex.fr']);
  expect(screen.getByText('Bob')).toBeInTheDocument();
  expect(input).toHaveValue('');
});

test('invalid address chip is marked', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.type(screen.getByRole('combobox'), 'pas une adresse{Enter}');
  const chip = screen.getByText('pas une adresse');
  expect(chip).toHaveAttribute('aria-invalid', 'true');
});

test('Backspace on empty input removes last chip', async () => {
  const user = userEvent.setup();
  render(<Harness initial={[{ name: null, email: 'a@ex.fr' }, { name: null, email: 'b@ex.fr' }]} />);
  await user.click(screen.getByRole('combobox'));
  await user.keyboard('{Backspace}');
  expect(last.map((a) => a.email)).toEqual(['a@ex.fr']);
});

test('pasting a list creates several chips', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('combobox'));
  await user.paste('a@ex.fr; b@ex.fr, a@ex.fr');
  expect(last.map((a) => a.email)).toEqual(['a@ex.fr', 'b@ex.fr']);
});

test('suggestions from searchContacts are selectable with arrows and Enter', async () => {
  fake.searchContacts.mockResolvedValue([
    { name: 'Marie Dupont', email: 'marie@ex.fr' },
    { name: 'Marc Lenoir', email: 'marc@ex.fr' },
  ]);
  const user = userEvent.setup();
  render(<Harness />);
  await user.type(screen.getByRole('combobox'), 'mar');
  const options = await screen.findAllByRole('option');
  expect(options).toHaveLength(2);
  expect(fake.searchContacts).toHaveBeenLastCalledWith('mar');
  await user.keyboard('{ArrowDown}');
  expect(screen.getAllByRole('option')[1]).toHaveAttribute('aria-selected', 'true');
  await act(async () => {
    await user.keyboard('{Enter}');
  });
  expect(last).toEqual([{ name: 'Marc Lenoir', email: 'marc@ex.fr' }]);
  expect(screen.queryByRole('option')).not.toBeInTheDocument();
});

test('leaving the field keeps the typed address', async () => {
  const user = userEvent.setup();
  render(
    <>
      <Harness />
      <button>ailleurs</button>
    </>,
  );
  await user.type(screen.getByRole('combobox'), 'zoe@ex.fr');
  await user.click(screen.getByRole('button', { name: 'ailleurs' }));
  expect(last).toEqual([{ name: null, email: 'zoe@ex.fr' }]);
});
