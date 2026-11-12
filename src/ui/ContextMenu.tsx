import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import styles from './ContextMenu.module.css';

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  onSelect(): void;
  disabled?: boolean;
}

interface Props {
  items: MenuItem[];
  position: { x: number; y: number };
  onClose(): void;
}

const MARGIN = 8;

/** Menu contextuel : ↑ ↓ Entrée Échap, se ferme au clic extérieur. Extensible au lot C (actions). */
export function ContextMenu({ items, position, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const enabled = items.map((it, i) => (it.disabled ? -1 : i)).filter((i) => i >= 0);
  const [active, setActive] = useState(enabled[0] ?? -1);
  const [pos, setPos] = useState(position);

  // Reste dans la fenêtre.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      x: Math.max(MARGIN, Math.min(position.x, window.innerWidth - width - MARGIN)),
      y: Math.max(MARGIN, Math.min(position.y, window.innerHeight - height - MARGIN)),
    });
  }, [position.x, position.y]);

  useEffect(() => {
    ref.current?.focus();
    const outside = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const close = () => onClose();
    document.addEventListener('mousedown', outside, true);
    window.addEventListener('blur', close);
    window.addEventListener('resize', close);
    document.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('mousedown', outside, true);
      window.removeEventListener('blur', close);
      window.removeEventListener('resize', close);
      document.removeEventListener('scroll', close, true);
    };
  }, [onClose]);

  const choose = (i: number) => {
    const it = items[i];
    if (!it || it.disabled) return;
    onClose();
    it.onSelect();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (enabled.length === 0 && e.key !== 'Escape') return;
    const at = enabled.indexOf(active);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive(enabled[(at + 1) % enabled.length]);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive(enabled[(at - 1 + enabled.length) % enabled.length]);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      choose(active);
    } else if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      onClose();
    }
  };

  return createPortal(
    <div
      ref={ref}
      role="menu"
      tabIndex={-1}
      className={styles.menu}
      style={{ left: pos.x, top: pos.y }}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
      aria-activedescendant={active >= 0 ? `ctx-item-${active}` : undefined}
    >
      {items.map((it, i) => (
        <div
          key={it.label}
          id={`ctx-item-${i}`}
          role="menuitem"
          aria-disabled={it.disabled || undefined}
          className={[styles.item, i === active ? styles.active : '', it.disabled ? styles.disabled : ''].join(' ')}
          onMouseEnter={() => !it.disabled && setActive(i)}
          onClick={() => choose(i)}
        >
          <span className={styles.icon}>{it.icon}</span>
          {it.label}
        </div>
      ))}
    </div>,
    document.body,
  );
}
