import { useEffect, useRef, useState } from 'react';
import { api } from '../../api';
import { errorMessage, type Attachment, type Draft, type DraftKind, type MessageDetail } from '../../api/types';
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
import { Composer } from '../composer/Composer';
import { senderName } from '../message-list/format';
import { Attachments } from './Attachments';
import { ImagesBanner } from './ImagesBanner';
import { MailFrame } from './MailFrame';
import styles from './Reader.module.css';

export { formatSize } from './Attachments';

const fullDate = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });

const REPLIES: { name: string; Icon: typeof IconReply; kind: Exclude<DraftKind, 'new'> }[] = [
  { name: 'Répondre', Icon: IconReply, kind: 'reply' },
  { name: 'Répondre à tous', Icon: IconReplyAll, kind: 'reply_all' },
  { name: 'Transférer', Icon: IconForward, kind: 'forward' },
];

// Lot C : actions sur le mail.
const LATER = [
  { name: 'Marquer non lu', Icon: IconMailUnread },
  { name: 'Déplacer', Icon: IconMove },
  { name: 'Spam', Icon: IconSpam },
  { name: 'Supprimer', Icon: IconTrash },
];

/** Délai avant de chercher un brouillon de réponse : laisse l'éditeur qui se ferme l'enregistrer (ou le jeter). */
const DRAFT_LOOKUP_DELAY_MS = 400;

type Load = { state: 'idle' } | { state: 'loading' } | { state: 'error'; message: string } | { state: 'ready'; detail: MessageDetail };

export function Reader() {
  const messageId = useAppStore((s) => s.selection.messageId);
  const [load, setLoad] = useState<Load>({ state: 'idle' });
  const [showImages, setShowImages] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const replyTarget = useAppStore((s) => s.replyTarget);
  const openReply = useAppStore((s) => s.openReply);
  const closeReply = useAppStore((s) => s.closeReply);
  const reopenDraft = useAppStore((s) => s.reopenDraft);
  const folders = useAppStore((s) => s.folders);
  const [pendingDraft, setPendingDraft] = useState<Draft | null>(null);
  const composerRef = useRef<HTMLDivElement>(null);
  const replying = replyTarget != null && replyTarget.messageId === messageId;

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

  // « Reprendre la réponse » : un brouillon répond déjà à ce mail.
  useEffect(() => {
    setPendingDraft(null);
    if (messageId == null || replying) return;
    let cancelled = false;
    const t = setTimeout(() => {
      api
        .draftForMessage(messageId)
        .then((d) => !cancelled && setPendingDraft(d))
        .catch(() => undefined);
    }, DRAFT_LOOKUP_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [messageId, replying]);

  useEffect(() => {
    if (replying) composerRef.current?.scrollIntoView?.({ block: 'start', behavior: 'smooth' });
  }, [replying, replyTarget?.draftId]);

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

  const inDrafts = Object.values(folders)
    .flat()
    .some((f) => f.id === d.summary.folderId && f.role === 'drafts');

  const reply = (kind: Exclude<DraftKind, 'new'>) => {
    setNotice(null);
    openReply(kind).catch((e) => setNotice(`Impossible de préparer la réponse : ${errorMessage(e)}`));
  };

  const editDraft = () => {
    setNotice(null);
    api
      .draftFromMessage(d.summary.id)
      .then((draft) => api.openComposeWindow(draft.id))
      .catch((e) => setNotice(`Impossible d'ouvrir le brouillon : ${errorMessage(e)}`));
  };

  return (
    <article className={styles.reader}>
      <div className={styles.toolbar} role="toolbar" aria-label="Actions">
        {REPLIES.map(({ name, Icon, kind }) => (
          <button key={name} className={styles.tool} aria-label={name} title={name} onClick={() => reply(kind)}>
            <Icon size={17} />
          </button>
        ))}
        {LATER.map(({ name, Icon }) => (
          <button key={name} className={styles.tool} aria-label={name} title={`${name} — disponible bientôt`} disabled>
            <Icon size={17} />
          </button>
        ))}
        {inDrafts && (
          <button className={styles.toolText} onClick={editDraft}>
            Modifier ce brouillon
          </button>
        )}
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
        {!replying && pendingDraft && (
          <button className={styles.resume} onClick={() => reopenDraft(pendingDraft)}>
            Reprendre la réponse
          </button>
        )}
        {replying && (
          <div ref={composerRef}>
            <Composer key={replyTarget.draftId} draftId={replyTarget.draftId} variant="inline" onClosed={closeReply} />
          </div>
        )}
      </div>
    </article>
  );
}
