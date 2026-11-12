import Image from '@tiptap/extension-image';
import { Placeholder } from '@tiptap/extensions';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { useEffect, useRef } from 'react';
import styles from './Settings.module.css';

const SAVE_DELAY_MS = 1000;

/** Éditeur de signature : enregistrée 1 s après la dernière frappe (et à la fermeture). */
export function SignatureEditor({ value, onSave }: { value: string; onSave(html: string): void }) {
  const pending = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saveRef = useRef(onSave);
  saveRef.current = onSave;

  const flush = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    if (pending.current !== null) {
      const html = pending.current;
      pending.current = null;
      saveRef.current(html);
    }
  };

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ heading: false, code: false, codeBlock: false, horizontalRule: false, blockquote: false, trailingNode: false }),
      Image.configure({ allowBase64: true, inline: true }),
      Placeholder.configure({ placeholder: 'Ta signature (nom, téléphone…)' }),
    ],
    // Le cœur range les images distantes en « data-blocked-src » : l'éditeur les montre telles quelles
    // (c'est la signature de l'utilisateur), sinon TipTap les perdrait à la première frappe.
    content: value.replace(/ data-blocked-src="/g, ' src="').replace(/ data-blocked-srcset="/g, ' srcset="'),
    editorProps: { attributes: { class: styles.signatureContent, 'aria-label': 'Signature' } },
    onUpdate: ({ editor: e }) => {
      pending.current = e.isEmpty ? '' : e.getHTML();
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(flush, SAVE_DELAY_MS);
    },
  });

  useEffect(() => flush, []);

  return (
    <div className={styles.signature} data-testid="signature-editor">
      <EditorContent editor={editor} />
    </div>
  );
}
