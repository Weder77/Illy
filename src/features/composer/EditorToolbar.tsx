import { useEditorState, type Editor } from '@tiptap/react';
import { useRef } from 'react';
import {
  IconBold,
  IconClip,
  IconImage,
  IconItalic,
  IconLink,
  IconList,
  IconListOrdered,
  IconQuote,
  IconUnderline,
} from '../../ui/icons';
import styles from './Composer.module.css';

interface Props {
  editor: Editor;
  onImages(files: File[]): void;
  onAttach(): void;
}

function safeLink(url: string): string | null {
  const t = url.trim();
  if (!t) return null;
  if (/^(https?:|mailto:)/i.test(t)) return t;
  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(t)) return `mailto:${t}`;
  if (/^[\w-]+(\.[\w-]+)+(\/\S*)?$/.test(t)) return `https://${t}`;
  return null;
}

export function EditorToolbar({ editor, onImages, onAttach }: Props) {
  const fileRef = useRef<HTMLInputElement>(null);
  const active = useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      bold: e.isActive('bold'),
      italic: e.isActive('italic'),
      underline: e.isActive('underline'),
      bulletList: e.isActive('bulletList'),
      orderedList: e.isActive('orderedList'),
      link: e.isActive('link'),
      blockquote: e.isActive('blockquote'),
    }),
  });

  const editLink = () => {
    const prev = (editor.getAttributes('link').href as string | undefined) ?? '';
    const answer = window.prompt('Adresse du lien', prev);
    if (answer === null) return;
    if (answer.trim() === '') {
      editor.chain().focus().extendMarkRange('link').unsetLink().run();
      return;
    }
    const href = safeLink(answer);
    if (!href) return;
    editor.chain().focus().extendMarkRange('link').setLink({ href }).run();
  };

  const tools = [
    { label: 'Gras', Icon: IconBold, on: active.bold, run: () => editor.chain().focus().toggleBold().run() },
    { label: 'Italique', Icon: IconItalic, on: active.italic, run: () => editor.chain().focus().toggleItalic().run() },
    { label: 'Souligné', Icon: IconUnderline, on: active.underline, run: () => editor.chain().focus().toggleUnderline().run() },
    { label: 'Liste à puces', Icon: IconList, on: active.bulletList, run: () => editor.chain().focus().toggleBulletList().run() },
    { label: 'Liste numérotée', Icon: IconListOrdered, on: active.orderedList, run: () => editor.chain().focus().toggleOrderedList().run() },
    { label: 'Lien', Icon: IconLink, on: active.link, run: editLink },
    { label: 'Citation', Icon: IconQuote, on: active.blockquote, run: () => editor.chain().focus().toggleBlockquote().run() },
  ];

  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Mise en forme">
      {tools.map(({ label, Icon, on, run }) => (
        <button
          key={label}
          type="button"
          className={on ? `${styles.toolBtn} ${styles.toolBtnOn}` : styles.toolBtn}
          aria-label={label}
          aria-pressed={on}
          title={label}
          onMouseDown={(e) => e.preventDefault()}
          onClick={run}
        >
          <Icon size={16} />
        </button>
      ))}
      <span className={styles.toolSep} />
      <button type="button" className={styles.toolBtn} aria-label="Insérer une image" title="Insérer une image" onClick={() => fileRef.current?.click()}>
        <IconImage size={16} />
      </button>
      <button type="button" className={styles.toolBtn} aria-label="Joindre des fichiers" title="Joindre des fichiers" onClick={onAttach}>
        <IconClip size={16} />
      </button>
      <input
        ref={fileRef}
        type="file"
        accept="image/*"
        multiple
        hidden
        onChange={(e) => {
          onImages([...(e.target.files ?? [])]);
          e.target.value = '';
        }}
      />
    </div>
  );
}
