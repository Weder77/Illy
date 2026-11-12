import type { Attachment } from '../../api/types';
import { IconClip, IconLock } from '../../ui/icons';
import styles from './Reader.module.css';

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} Ko`;
  return `${(bytes / 1024 / 1024).toFixed(1).replace('.', ',')} Mo`;
}

export function Attachments(props: {
  items: Attachment[];
  onOpen: (a: Attachment) => void;
  onSave: (a: Attachment) => void;
}) {
  return (
    <div className={styles.attachments}>
      {props.items.map((a) => (
        <div key={a.partPath} className={styles.attachment}>
          <button
            className={styles.attachmentMain}
            title={a.canOpen ? 'Ouvrir' : 'Ce type de fichier ne s’ouvre pas directement : enregistre-le'}
            onClick={() => (a.canOpen ? props.onOpen(a) : props.onSave(a))}
          >
            {a.canOpen ? <IconClip size={14} /> : <IconLock size={14} />}
            <span className={styles.attachmentName}>{a.filename}</span>
            <span className={styles.attachmentSize}>{formatSize(a.size)}</span>
          </button>
          <button className={styles.attachmentSave} aria-label={`Enregistrer sous… ${a.filename}`} title="Enregistrer sous…" onClick={() => props.onSave(a)}>
            ⤓
          </button>
        </div>
      ))}
    </div>
  );
}
