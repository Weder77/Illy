import { render } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

vi.mock('../../api', () => ({ api: { openUrl: vi.fn() } }));

import { buildSrcDoc, MailFrame } from './MailFrame';

test('csp blocks remote images by default', () => {
  const doc = buildSrcDoc({ kind: 'html', content: '<p>x</p>' }, false);
  expect(doc).toContain("img-src data:;");
  expect(doc).not.toContain('https:');
  expect(doc).toContain("default-src 'none'");
  expect(doc).toContain('<p>x</p>');
});

test('csp allows remote when asked', () => {
  expect(buildSrcDoc({ kind: 'html', content: '' }, true)).toContain('img-src data: https: http:;');
});

test('iframe is sandboxed without scripts', () => {
  const { container } = render(<MailFrame body={{ kind: 'html', content: '<p>x</p>' }} allowRemoteImages={false} />);
  const iframe = container.querySelector('iframe')!;
  expect(iframe.getAttribute('sandbox')).toBe('allow-same-origin');
  expect(iframe.getAttribute('srcdoc')).toContain('<p>x</p>');
});
