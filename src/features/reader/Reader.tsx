import { useEffect, useState } from 'react';
import { api } from '../../api';
import { errorMessage, type Attachment, type MessageDetail } from '../../api/types';
import { useAppStore } from '../../state/store';
import {
  IconForward,
  IconMailUnread,
  IconMove,
  IconReply,
  IconReplyAll,
  IconSpam,
  IconTrash,
} from '../../ui/icons';
import { senderName } from '../message-list/format';
import { Attachments } from './Attachments';
import { ImagesBanner } from './ImagesBanner';
import { MailFrame } from './MailFrame';
import styles from './Reader.module.css';

export { formatSize } from './Attachments';

const fullDate = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });

const ACTIONS = [
  { name: 'Répondre', Icon: IconReply },
  { name: 'Répondre à tous', Icon: IconReplyAll },
  { name: 'Transférer', Icon: IconForward },
  { name: 'Marquer non lu', Icon: IconMailUnread },
  { name: 'Déplacer', Icon: IconMove },
  { name: 'Spam', Icon: IconSpam },
  { name: 'Supprimer', Icon: IconTrash },
];

type Load = { state: 'idle' } | { state: 'loading' } | { state: 'error'; message: string } | { state: 'ready'; detail: MessageDetail };

export function Reader() {
  const messageId = useAppStore((s) => s.selection.messageId);
  const [load, setLoad] = useState<Load>({ state: 'idle' });
  const [showImages, setShowImages] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    setShowImages(false);
    setNotice(null);
  }, [messageId]);

  useEffect(() => {
    if (messageId == null) {
      setLoad({ state: 'idle' });
      return;
    }
    let cancelled = false;
    setLoad((prev) => (prev.state === 'ready' && prev.detail.summary.id === messageId ? prev : { state: 'loading' }));
    api
      .openMessage(messageId, showImages)
      .then((detail) => !cancelled && setLoad({ state: 'ready', detail }))
      .catch((e) => !cancelled && setLoad({ state: 'error', message: errorMessage(e) }));
    return () => {
      cancelled = true;
    };
  }, [messageId, showImages, reloadKey]);

  if (messageId == null) return <div className={styles.empty}>Sélectionne un mail pour l'afficher</div>;
  if (load.state === 'error') return <div className={styles.error}>Impossible d'ouvrir ce mail : {load.message}</div>;
  if (load.state !== 'ready') {
    return (
      <div className={styles.reader} aria-busy="true">
        <div className={styles.skeletonTitle} />
        <div className={styles.skeletonLine} />
        <div className={styles.skeletonCard} />
      </div>
    );
  }

  const d = load.detail;
  const from = d.summary.from;
  const recipients = [...d.to, ...d.cc].map((a) => a.name?.trim() || a.email).join(', ');
  const attachmentAction = (fn: (id: number, part: string) => Promise<void>, verb: string) => (a: Attachment) => {
    setNotice(null);
    fn(d.summary.id, a.partPath).catch((e) => setNotice(`Impossible ${verb} « ${a.filename} » : ${errorMessage(e)}`));
  };

  return (
    <article className={styles.reader}>
      <div className={styles.toolbar} role="toolbar" aria-label="Actions">
        {ACTIONS.map(({ name, Icon }) => (
          <button key={name} className={styles.tool} aria-label={name} title={`${name} — disponible bientôt`} disabled>
            <Icon size={17} />
          </button>
        ))}
      </div>
      <div className={styles.scroll}>
        <h1 className={styles.subject}>{d.summary.subject.trim() || '(sans objet)'}</h1>
        <div className={styles.meta}>
          <div className={styles.avatar} aria-hidden="true">
            {senderName(from).charAt(0).toUpperCase()}
          </div>
          <div className={styles.metaText}>
            <div>
              <span className={styles.fromName}>{senderName(from)}</span>{' '}
              {from.name && <span className={styles.fromEmail}>{`<${from.email}>`}</span>}
            </div>
            <div className={styles.recipients}>{recipients ? `à ${recipients}` : ''}</div>
          </div>
          <div className={styles.date}>{fullDate.format(new Date(d.summary.date * 1000))}</div>
        </div>
        {d.imagesBlocked && (
          <ImagesBanner
            onShow={() => setShowImages(true)}
            onAlways={() => {
              api
                .allowImagesFrom(from.email)
                .then(() => setReloadKey((k) => k + 1))
                .catch((e) => setNotice(errorMessage(e)));
            }}
          />
        )}
        {d.degraded && <div className={styles.degraded}>Ce mail n'a pas pu être affiché correctement</div>}
        <MailFrame body={d.body} allowRemoteImages={d.remoteImagesAllowed} />
        {d.attachments.length > 0 && (
          <Attachments
            items={d.attachments}
            onOpen={attachmentAction(api.openAttachment, "d'ouvrir")}
            onSave={attachmentAction(api.saveAttachment, "d'enregistrer")}
          />
        )}
        {notice && (
          <div className={styles.notice} role="alert">
            {notice}
          </div>
        )}
      </div>
    </article>
  );
}
