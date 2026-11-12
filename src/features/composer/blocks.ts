// Blocs HTML bruts de l'éditeur : signature, citation d'une réponse, message transféré.
// Le HTML du mail d'origine (déjà nettoyé par le cœur) n'entre pas dans le schéma TipTap,
// qui l'aplatirait : il est gardé tel quel et affiché dans un shadow DOM, pour que ses styles
// (<style>, position: fixed…) ne débordent pas sur Illy.
import { Extension, Node, type Editor } from '@tiptap/react';

function rawBlock(name: string, selector: string, attrs: Record<string, string>) {
  return Node.create({
    name,
    group: 'block',
    atom: true,
    selectable: true,
    draggable: false,
    addAttributes() {
      return {
        html: {
          default: '',
          parseHTML: (el: HTMLElement) => el.innerHTML,
          renderHTML: () => ({}),
        },
      };
    },
    parseHTML() {
      return [{ tag: selector, priority: 1000 }];
    },
    renderHTML({ node }) {
      const el = document.createElement('div');
      for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
      el.innerHTML = node.attrs.html as string;
      return el;
    },
    addNodeView() {
      return ({ node }) => {
        const dom = document.createElement('div');
        dom.className = `illy-raw illy-raw-${name}`;
        dom.contentEditable = 'false';
        const root = dom.attachShadow({ mode: 'open' });
        root.innerHTML =
          '<style>:host{display:block;contain:content;overflow:hidden;max-width:100%}' +
          'blockquote{margin:6px 0 0;padding-left:12px;border-left:3px solid #c9ced6}' +
          'img{max-width:100%;height:auto}</style>' +
          (node.attrs.html as string);
        return { dom, ignoreMutation: () => true };
      };
    },
  });
}

export const SignatureBlock = rawBlock('illySignature', 'div[data-illy-signature]', { 'data-illy-signature': '' });
export const QuoteBlock = rawBlock('illyQuote', 'div.illy-quote', { class: 'illy-quote' });
export const ForwardBlock = rawBlock('illyForward', 'div.illy-forward', { class: 'illy-forward' });

/** Ctrl+Entrée envoie (géré par le Composer) : on empêche le saut de ligne de TipTap. */
export const SendShortcut = Extension.create({
  name: 'illySendShortcut',
  priority: 1000,
  addKeyboardShortcuts() {
    return { 'Mod-Enter': () => true };
  },
});

export function hasSignatureContent(html: string): boolean {
  return html.includes('<img') || html.replace(/<[^>]*>/g, '').replace(/&nbsp;/g, ' ').trim() !== '';
}

/** Remplace le bloc de signature (changement de compte « De »), l'ajoute ou le retire. */
export function replaceSignature(editor: Editor, html: string) {
  const { state } = editor;
  let sig: { pos: number; size: number } | null = null;
  let quotePos: number | null = null;
  state.doc.forEach((n, pos) => {
    if (n.type.name === 'illySignature' && !sig) sig = { pos, size: n.nodeSize };
    if ((n.type.name === 'illyQuote' || n.type.name === 'illyForward') && quotePos == null) quotePos = pos;
  });
  const keep = hasSignatureContent(html);
  const tr = state.tr;
  const found = sig as { pos: number; size: number } | null;
  if (found) {
    if (keep) tr.setNodeMarkup(found.pos, undefined, { html });
    else tr.delete(found.pos, found.pos + found.size);
  } else if (keep) {
    tr.insert(quotePos ?? state.doc.content.size, state.schema.nodes.illySignature.create({ html }));
  } else {
    return;
  }
  editor.view.dispatch(tr);
}
