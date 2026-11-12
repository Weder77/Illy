import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { ContextMenu } from './ContextMenu';

const setup = () => {
  const a = vi.fn();
  const b = vi.fn();
  const c = vi.fn();
  const onClose = vi.fn();
  render(
    <>
      <button>dehors</button>
      <ContextMenu
        position={{ x: 10, y: 10 }}
        onClose={onClose}
        items={[
          { label: 'Répondre', onSelect: a },
          { label: 'Bientôt', onSelect: c, disabled: true },
          { label: 'Transférer', onSelect: b },
        ]}
      />
    </>,
  );
  return { a, b, c, onClose };
};

test('arrow keys and Enter select', () => {
  const { a, b, c, onClose } = setup();
  const menu = screen.getByRole('menu');
  expect(menu).toHaveFocus();
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  // L'entrée désactivée est sautée.
  expect(screen.getByRole('menuitem', { name: 'Transférer' }).className).toMatch(/active/);
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  expect(screen.getByRole('menuitem', { name: 'Répondre' }).className).toMatch(/active/);
  fireEvent.keyDown(menu, { key: 'ArrowUp' });
  fireEvent.keyDown(menu, { key: 'Enter' });
  expect(b).toHaveBeenCalledTimes(1);
  expect(a).not.toHaveBeenCalled();
  expect(c).not.toHaveBeenCalled();
  expect(onClose).toHaveBeenCalled();
});

test('Escape and outside click close', () => {
  const { a, onClose } = setup();
  fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
  expect(onClose).toHaveBeenCalledTimes(1);
  fireEvent.mouseDown(screen.getByRole('button', { name: 'dehors' }));
  expect(onClose).toHaveBeenCalledTimes(2);
  fireEvent.mouseDown(screen.getByRole('menuitem', { name: 'Répondre' }));
  expect(onClose).toHaveBeenCalledTimes(2);
  expect(a).not.toHaveBeenCalled();
});

test('click selects an item', () => {
  const { a } = setup();
  fireEvent.click(screen.getByRole('menuitem', { name: 'Répondre' }));
  expect(a).toHaveBeenCalledTimes(1);
});
