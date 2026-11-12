import { useVirtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useMemo, useReducer, useRef, useState, type KeyboardEvent } from 'react';
import { api } from '../../api';
import type { MessageSummary } from '../../api/types';
import { useAppStore } from '../../state/store';
import { ContextMenu } from '../../ui/ContextMenu';
import { IconForward, IconReply, IconReplyAll } from '../../ui/icons';
import { MessageRow } from './MessageRow';
import { SearchBar } from './SearchBar';
import styles from './MessageList.module.css';

const PAGE = 100;
const ROW = 76;

export function MessageList() {
  const scope = useAppStore((s) => s.selection.scope);
  const selectedId = useAppStore((s) => s.selection.messageId);
  const listVersion = useAppStore((s) => s.listVersion);
  const accounts = useAppStore((s) => s.accounts);
  const folders = useAppStore((s) => s.folders);
  const selectMessage = useAppStore((s) => s.selectMessage);
  const openReply = useAppStore((s) => s.openReply);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);

  const scopeKey = scope.kind === 'folder' ? `f${scope.folderId}` : 'unified';
  const [count, setCount] = useState<number | null>(null);
  const pages = useRef(new Map<number, MessageSummary[]>());
  const pending = useRef(new Set<number>());
  const generation = useRef(0);
  const [, rerender] = useReducer((x: number) => x + 1, 0);
  const parentRef = useRef<HTMLDivElement>(null);
  const lastScope = useRef(scopeKey);

  const loadPage = useCallback(
    (page: number) => {
      if (pending.current.has(page)) return;
      pending.current.add(page);
      const gen = generation.current;
      api
        .listMessages(scope, page * PAGE, PAGE)
        .then((rows) => {
          if (gen !== generation.current) return;
          pages.current.set(page, rows);
          rerender();
        })
        .catch((e) => console.error('liste des mails', e))
        .finally(() => {
          if (gen === generation.current) pending.current.delete(page);
        });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scopeKey],
  );

  // Changement de boîte : on repart de zéro. Mise à jour de la même boîte : on relit sans vider l'écran.
  useEffect(() => {
    generation.current += 1;
    pending.current = new Set();
    const sameScope = lastScope.current === scopeKey;
    lastScope.current = scopeKey;
    const stale = sameScope ? [...pages.current.keys()] : [];
    if (!sameScope) {
      pages.current = new Map();
      setCount(null);
      parentRef.current?.scrollTo?.({ top: 0 });
    }
    let cancelled = false;
    api
      .countMessages(scope)
      .then((n) => {
        if (cancelled) return;
        setCount(n);
        stale.forEach(loadPage);
        rerender();
      })
      .catch(() => !cancelled && setCount(0));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, listVersion]);

  const virtualizer = useVirtualizer({
    count: count ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW,
    overscan: 10,
    initialRect: { width: 380, height: 800 },
  });
  const items = virtualizer.getVirtualItems();

  useEffect(() => {
    const needed = new Set(items.map((i) => Math.floor(i.index / PAGE)));
    needed.forEach((p) => {
      if (!pages.current.has(p)) loadPage(p);
    });
  });

  const rowAt = (index: number) => pages.current.get(Math.floor(index / PAGE))?.[index % PAGE];

  const selectedIndex = useMemo(() => {
    if (selectedId == null) return -1;
    for (const [p, rows] of pages.current) {
      const i = rows.findIndex((r) => r.id === selectedId);
      if (i >= 0) return p * PAGE + i;
    }
    return -1;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, count, items.length, listVersion]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const next = selectedIndex < 0 ? 0 : selectedIndex + (e.key === 'ArrowDown' ? 1 : -1);
    const row = rowAt(next);
    if (row) {
      selectMessage(row.id);
      virtualizer.scrollToIndex(next, { align: 'auto' });
    }
  };

  const colors = useMemo(() => new Map(accounts.map((a) => [a.id, a.color])), [accounts]);
  const folder = scope.kind === 'folder' ? Object.values(folders).flat().find((f) => f.id === scope.folderId) : undefined;
  const account = folder ? accounts.find((a) => a.id === folder.accountId) : undefined;
  const title = scope.kind === 'unified_inbox' ? 'Toutes les boîtes' : (folder?.displayName ?? 'Boîte');
  const subtitle = scope.kind === 'unified_inbox' ? `${accounts.length} compte${accounts.length > 1 ? 's' : ''}` : account?.email;

  return (
    <div className={styles.wrap}>
      <div className={styles.header}>
        <div className={styles.title}>
          <span>{title}</span>
        </div>
        {subtitle && <div className={styles.subtitle}>{subtitle}</div>}
        <SearchBar />
      </div>
      <div ref={parentRef} className={styles.list} role="listbox" aria-label="Mails" tabIndex={0} onKeyDown={onKeyDown}>
        {count === 0 && <div className={styles.empty}>Aucun mail dans cette boîte</div>}
        {count === null && (
          <div className={styles.inner} style={{ height: 8 * ROW }}>
            {Array.from({ length: 8 }, (_, i) => (
              <div key={i} className={styles.skeleton} style={{ top: i * ROW + 8 }} />
            ))}
          </div>
        )}
        {count !== null && count > 0 && (
          <div className={styles.inner} style={{ height: virtualizer.getTotalSize() }}>
            {items.map((item) => {
              const m = rowAt(item.index);
              const style = { top: item.start, height: ROW - 4 };
              if (!m) return <div key={item.key} className={styles.skeleton} style={{ top: item.start + 8 }} />;
              return (
                <MessageRow
                  key={m.id}
                  m={m}
                  style={style}
                  selected={m.id === selectedId}
                  accountColor={scope.kind === 'unified_inbox' ? colors.get(m.accountId) : undefined}
                  onSelect={selectMessage}
                  onContextMenu={(id, at) => {
                    selectMessage(id);
                    setMenu(at);
                  }}
                />
              );
            })}
          </div>
        )}
      </div>
      {menu && (
        <ContextMenu
          position={menu}
          onClose={closeMenu}
          items={[
            { label: 'Répondre', icon: <IconReply size={16} />, onSelect: () => void openReply('reply') },
            { label: 'Répondre à tous', icon: <IconReplyAll size={16} />, onSelect: () => void openReply('reply_all') },
            { label: 'Transférer', icon: <IconForward size={16} />, onSelect: () => void openReply('forward') },
          ]}
        />
      )}
    </div>
  );
}
