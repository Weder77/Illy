import type { DraftAttachment } from '../../api/types';
import { IconClip, IconClose } from '../../ui/icons';
import { formatSize } from '../reader/Attachments';
import styles from './Composer.module.css';

export function AttachmentList({ items, onRemove }: { items: DraftAttachment[]; onRemove(index: number): void }) {
  if (items.length === 0) return null;
  return (
    <ul className={styles.attachments} aria-label="Pièces jointes">
      {items.map((a, i) => (
        <li key={`${a.filename}-${i}`} className={styles.attachment}>
          <IconClip size={14} />
          <span className={styles.attachmentName} title={a.filename}>
            {a.filename}
          </span>
          <span className={styles.attachmentSize}>{formatSize(a.size)}</span>
          <button type="button" className={styles.chipRemove} aria-label={`Retirer ${a.filename}`} onClick={() => onRemove(i)}>
            <IconClose size={12} />
          </button>
        </li>
      ))}
    </ul>
  );
}
