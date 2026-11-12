import { useEffect, useRef, useState } from 'react';
import { api } from '../../api';
import type { MessageBody } from '../../api/types';
import styles from './Reader.module.css';

const BASE_STYLE =
  'html,body{margin:0;padding:0;background:#fff}' +
  'body{font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;color:#111;word-wrap:break-word;overflow-wrap:anywhere}' +
  'img{max-width:100%;height:auto}table{max-width:100%}a{color:#1a56db}' +
  'blockquote{margin:0 0 0 8px;padding-left:12px;border-left:3px solid #d0d5dd;color:#475467}' +
  'pre{white-space:pre-wrap}' +
  'img[data-blocked-src]{display:inline-block;box-sizing:border-box;min-width:24px;min-height:24px;' +
  'background:#f2f4f7;border:1px dashed #d0d5dd;border-radius:6px;color:#98a2b3;font-size:12px}';

/** Document du mail, avec une CSP stricte : aucune ressource externe sauf images autorisées. */
export function buildSrcDoc(body: MessageBody, allowRemoteImages: boolean): string {
  const img = allowRemoteImages ? 'data: https: http:' : 'data:';
  const csp = `default-src 'none'; style-src 'unsafe-inline'; img-src ${img}; font-src data:`;
  return (
    '<!doctype html><html><head><meta charset="utf-8">' +
    `<meta http-equiv="Content-Security-Policy" content="${csp}">` +
    '<meta name="referrer" content="no-referrer">' +
    `<style>${BASE_STYLE}</style></head><body>${body.content}</body></html>`
  );
}

export function MailFrame({ body, allowRemoteImages }: { body: MessageBody; allowRemoteImages: boolean }) {
  const ref = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(200);
  const [hoverUrl, setHoverUrl] = useState<string | null>(null);

  useEffect(() => {
    const iframe = ref.current;
    if (!iframe) return;
    let observer: ResizeObserver | null = null;
    const onLoad = () => {
      const doc = iframe.contentDocument;
      if (!doc) return;
      const measure = () => setHeight(Math.max(120, doc.documentElement.scrollHeight));
      measure();
      doc.querySelectorAll('img').forEach((img) => img.addEventListener('load', measure));
      if (typeof ResizeObserver !== 'undefined') {
        observer = new ResizeObserver(measure);
        observer.observe(doc.body);
      }
      doc.addEventListener('click', (e) => {
        const a = (e.target as Element | null)?.closest?.('a[href]');
        if (!a) return;
        e.preventDefault();
        const href = a.getAttribute('href') ?? '';
        if (/^https?:\/\//i.test(href)) void api.openUrl(href);
      });
      doc.addEventListener('mouseover', (e) => {
        const a = (e.target as Element | null)?.closest?.('a[href]');
        setHoverUrl(a ? a.getAttribute('href') : null);
      });
      doc.addEventListener('mouseleave', () => setHoverUrl(null));
    };
    iframe.addEventListener('load', onLoad);
    return () => {
      iframe.removeEventListener('load', onLoad);
      observer?.disconnect();
    };
  }, [body, allowRemoteImages]);

  return (
    <div className={styles.card}>
      <iframe
        ref={ref}
        title="Contenu du mail"
        className={styles.frame}
        sandbox="allow-same-origin"
        srcDoc={buildSrcDoc(body, allowRemoteImages)}
        style={{ height }}
      />
      <div className={styles.linkStatus} aria-live="polite">
        {hoverUrl}
      </div>
    </div>
  );
}
