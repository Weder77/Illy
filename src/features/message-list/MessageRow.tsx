import type { CSSProperties } from 'react';
import type { MessageSummary } from '../../api/types';
import { IconClip } from '../../ui/icons';
import { formatListDate, senderName } from './format';
import styles from './MessageList.module.css';

export function MessageRow(props: {
  m: MessageSummary;
  selected: boolean;
  accountColor?: string;
  style: CSSProperties;
  onSelect: (id: number) => void;
  onContextMenu?: (id: number, at: { x: number; y: number }) => void;
}) {
  const { m } = props;
  const cls = [styles.row, props.selected ? styles.selected : '', m.seen ? '' : styles.unread].join(' ');
  return (
    <div
      role="option"
      aria-selected={props.selected}
      className={cls}
      style={props.style}
      onClick={() => props.onSelect(m.id)}
      onContextMenu={(e) => {
        if (!props.onContextMenu) return;
        e.preventDefault();
        props.onContextMenu(m.id, { x: e.clientX, y: e.clientY });
      }}
    >
      <div className={styles.line}>
        {props.accountColor && (
          <span data-testid="account-dot" className={styles.accountDot} style={{ background: props.accountColor }} />
        )}
        <span className={styles.sender}>{senderName(m.from)}</span>
        <span className={styles.date}>{formatListDate(m.date)}</span>
      </div>
      <div className={styles.line}>
        <span className={styles.subject}>{m.subject.trim() || '(sans objet)'}</span>
        {m.hasAttachments && (
          <span className={styles.clip} role="img" aria-label="Pièce jointe">
            <IconClip size={14} />
          </span>
        )}
        {!m.seen && <span data-testid="unread-dot" className={styles.unreadDot} />}
      </div>
      {m.snippet && <div className={styles.snippet}>{m.snippet}</div>}
    </div>
  );
}
