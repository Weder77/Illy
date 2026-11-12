import Image from '@tiptap/extension-image';
import { Placeholder } from '@tiptap/extensions';
import { EditorContent, useEditor, type Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { api } from '../../api';
import { errorMessage, type Address, type Draft, type DraftAttachment, type DraftInput } from '../../api/types';
import { useAppStore } from '../../state/store';
import { IconChevronDown, IconClose, IconSend, IconTrash } from '../../ui/icons';
import { AttachmentList } from './AttachmentList';
import { ForwardBlock, QuoteBlock, replaceSignature, SendShortcut, SignatureBlock } from './blocks';
import styles from './Composer.module.css';
import { EditorToolbar } from './EditorToolbar';
import { RecipientField } from './RecipientField';
import { registerComposer } from './registry';
import { useAutosave } from './useAutosave';

const MB = 1024 * 1024;
export const MAX_IMAGE_BYTES = 5 * MB;
export const LARGE_ATTACHMENTS_BYTES = 25 * MB;

export function largeAttachmentsQuestion(total: number): string {
  return `Les pièces jointes font ${Math.round(total / MB)} Mo. Gmail et la plupart des serveurs refusent au-delà de 25 Mo. Envoyer quand même ?`;
}

interface Props {
  draftId: number;
  variant: 'inline' | 'window';
  /** Appelé quand l'éditeur doit disparaître (envoyé, supprimé ou fermé). */
  onClosed(): void;
}

export function Composer({ draftId, variant, onClosed }: Props) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setDraft(null);
    api
      .getDraft(draftId)
      .then((d) => !cancelled && setDraft(d))
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [draftId]);

  if (error) return <div className={styles.loadError}>Impossible d'ouvrir le brouillon : {error}</div>;
  if (!draft) return <div className={`${styles.composer} ${styles[variant]}`} aria-busy="true" />;
  return <ComposerForm key={draft.id} initial={draft} variant={variant} onClosed={onClosed} />;
}

type Values = Omit<DraftInput, 'id'>;

function ComposerForm({ initial, variant, onClosed }: { initial: Draft } & Omit<Props, 'draftId'>) {
  const id = initial.id;
  const accounts = useAppStore((s) => s.accounts);
  const [values, setValues] = useState<Values>({
    accountId: initial.accountId,
    to: initial.to,
    cc: initial.cc,
    bcc: initial.bcc,
    subject: initial.subject,
    bodyHtml: initial.bodyHtml,
  });
  const valuesRef = useRef(values);
  const [attachments, setAttachments] = useState<DraftAttachment[]>(initial.attachments);
  const [showCc, setShowCc] = useState(initial.cc.length + initial.bcc.length > 0);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const finished = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const save = useCallback((input: DraftInput) => api.updateDraft(input), []);
  const autosave = useAutosave(save);

  const update = (patch: Partial<Values>) => {
    if (finished.current) return;
    valuesRef.current = { ...valuesRef.current, ...patch };
    setValues(valuesRef.current);
    autosave.schedule({ id, ...valuesRef.current });
  };

  const insertImages = (editor: Editor, files: File[]) => {
    for (const f of files) {
      if (!f.type.startsWith('image/')) continue;
      if (f.size > MAX_IMAGE_BYTES) {
        setNotice('Image trop lourde (5 Mo au plus)');
        continue;
      }
      const reader = new FileReader();
      reader.onload = () => editor.chain().focus().setImage({ src: String(reader.result) }).run();
      reader.readAsDataURL(f);
    }
  };

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: false,
        code: false,
        codeBlock: false,
        horizontalRule: false,
        // Pas de paragraphe ajouté d'office après la citation : ouvrir un brouillon ne le modifie pas.
        trailingNode: false,
        link: { openOnClick: false, autolink: true, protocols: ['http', 'https', 'mailto'] },
      }),
      Image.configure({ allowBase64: true, inline: true }),
      Placeholder.configure({ placeholder: 'Écris ton message…' }),
      SignatureBlock,
      QuoteBlock,
      ForwardBlock,
      SendShortcut,
    ],
    content: initial.bodyHtml,
    autofocus: initial.kind === 'new' ? false : 'start',
    onUpdate: ({ editor: e }) => {
      const bodyHtml = e.getHTML();
      if (bodyHtml !== valuesRef.current.bodyHtml) update({ bodyHtml });
    },
    editorProps: {
      attributes: { class: styles.editorContent, 'aria-label': 'Message' },
      handlePaste: (_view, event) => {
        const files = [...(event.clipboardData?.files ?? [])].filter((f) => f.type.startsWith('image/'));
        if (files.length === 0 || !editorRef.current) return false;
        insertImages(editorRef.current, files);
        return true;
      },
      handleDrop: (_view, event) => {
        const files = [...((event as DragEvent).dataTransfer?.files ?? [])].filter((f) => f.type.startsWith('image/'));
        if (files.length === 0 || !editorRef.current) return false;
        insertImages(editorRef.current, files);
        return true;
      },
    },
  });
  const editorRef = useRef<Editor | null>(editor);
  editorRef.current = editor;

  // Enregistrer et fermer : jamais deux fois, jamais après un envoi ou une suppression.
  const saveAndClose = useCallback(async () => {
    if (finished.current) return;
    finished.current = true;
    await autosave.flush().catch(() => undefined);
    await api.closeDraft(id).catch(() => undefined);
  }, [autosave, id]);
  const saveAndCloseRef = useRef(saveAndClose);
  saveAndCloseRef.current = saveAndClose;

  // Sous le mail : se démonter enregistre le brouillon. Le délai absorbe le double montage du mode strict.
  const mounted = useRef(false);
  useEffect(() => {
    if (variant !== 'inline') return;
    mounted.current = true;
    const unregister = registerComposer(() => saveAndCloseRef.current());
    return () => {
      unregister();
      mounted.current = false;
      setTimeout(() => {
        if (!mounted.current) void saveAndCloseRef.current();
      }, 0);
    };
  }, [variant]);

  useEffect(() => {
    if (variant !== 'window') return;
    return api.onWindowCloseRequested(() => saveAndCloseRef.current());
  }, [variant]);

  const attach = (fn: () => Promise<Draft>) => {
    setError(null);
    fn()
      .then((d) => setAttachments(d.attachments))
      .catch((e) => setError(errorMessage(e)));
  };

  useEffect(
    () =>
      api.onFileDrop((paths, at) => {
        if (variant === 'inline') {
          const r = rootRef.current?.getBoundingClientRect();
          if (!r || at.x < r.left || at.x > r.right || at.y < r.top || at.y > r.bottom) return;
        }
        attach(() => api.addAttachments(id, paths));
      }),
    [id, variant],
  );

  const changeAccount = (accountId: number) => {
    update({ accountId });
    const signature = accounts.find((a) => a.id === accountId)?.signatureHtml ?? '';
    if (editor) replaceSignature(editor, signature);
  };

  const send = async () => {
    if (sending || finished.current) return;
    setError(null);
    const total = attachments.reduce((n, a) => n + a.size, 0);
    if (total > LARGE_ATTACHMENTS_BYTES && !window.confirm(largeAttachmentsQuestion(total))) return;
    setSending(true);
    try {
      autosave.schedule({ id, ...valuesRef.current });
      await autosave.flush();
      await api.sendDraft(id);
      finished.current = true;
      autosave.cancel();
      onClosed();
    } catch (e) {
      setError(errorMessage(e));
      setSending(false);
    }
  };

  const discard = async () => {
    if (finished.current) return;
    finished.current = true;
    autosave.cancel();
    await api.discardDraft(id).catch(() => undefined);
    onClosed();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      void send();
    }
  };

  const account = accounts.find((a) => a.id === values.accountId);
  const status =
    autosave.status === 'saving' ? 'Enregistrement…' : autosave.status === 'saved' ? 'Brouillon enregistré' : autosave.status === 'error' ? 'Enregistrement impossible' : '';

  return (
    <div ref={rootRef} className={`${styles.composer} ${styles[variant]}`} onKeyDown={onKeyDown}>
      <div className={styles.fields}>
        <div className={styles.field}>
          <label className={styles.fieldLabel} htmlFor={`from-${id}`}>
            De
          </label>
          <span className={styles.accountDot} style={{ background: account?.color ?? 'var(--text-2)' }} aria-hidden="true" />
          <div className={styles.fromWrap}>
            <select
              id={`from-${id}`}
              className={styles.fromSelect}
              value={values.accountId}
              onChange={(e) => changeAccount(Number(e.target.value))}
            >
              {accounts.length === 0 && <option value={values.accountId}>…</option>}
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.displayName ? `${a.displayName} <${a.email}>` : a.email}
                </option>
              ))}
            </select>
            <IconChevronDown size={14} className={styles.fromChevron} />
          </div>
          {variant === 'inline' && (
            <button type="button" className={styles.iconBtn} aria-label="Fermer la réponse" title="Fermer (le brouillon est gardé)" onClick={onClosed}>
              <IconClose size={16} />
            </button>
          )}
        </div>
        <RecipientField
          label="À"
          value={values.to}
          onChange={(to: Address[]) => update({ to })}
          autoFocus={initial.kind === 'new' || initial.kind === 'forward'}
          trailing={
            !showCc && (
              <button type="button" className={styles.linkBtn} onClick={() => setShowCc(true)}>
                Cc / Cci
              </button>
            )
          }
        />
        {showCc && (
          <>
            <RecipientField label="Cc" value={values.cc} onChange={(cc) => update({ cc })} />
            <RecipientField label="Cci" value={values.bcc} onChange={(bcc) => update({ bcc })} />
          </>
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        <div className={styles.field}>
          <label className={styles.fieldLabel} htmlFor={`subject-${id}`}>
            Objet
          </label>
          <input
            id={`subject-${id}`}
            className={styles.subjectInput}
            value={values.subject}
            onChange={(e) => update({ subject: e.target.value })}
          />
        </div>
      </div>
      {editor && (
        <EditorToolbar
          editor={editor}
          onImages={(files) => insertImages(editor, files)}
          onAttach={() => attach(() => api.pickAttachments(id))}
        />
      )}
      <div
        className={styles.editor}
        data-testid="composer-editor"
        // Pour les tests seulement : recopier le corps (images comprises) à chaque frappe coûte cher.
        data-html={import.meta.env.MODE === 'test' ? values.bodyHtml : undefined}
      >
        <EditorContent editor={editor} />
      </div>
      {notice && (
        <p className={styles.notice} role="status">
          {notice}
          <button type="button" className={styles.linkBtn} onClick={() => setNotice(null)}>
            OK
          </button>
        </p>
      )}
      <AttachmentList items={attachments} onRemove={(i) => attach(() => api.removeAttachment(id, i))} />
      <div className={styles.footer}>
        <button type="button" className={styles.sendBtn} onClick={() => void send()} disabled={sending} title="Envoyer (Ctrl+Entrée)">
          <IconSend size={16} />
          Envoyer
        </button>
        <span className={styles.status} aria-live="polite">
          {status}
        </span>
        <button type="button" className={styles.iconBtn} aria-label="Supprimer le brouillon" title="Supprimer le brouillon" onClick={() => void discard()}>
          <IconTrash size={16} />
        </button>
      </div>
    </div>
  );
}
