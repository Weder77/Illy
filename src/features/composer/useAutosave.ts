import { useCallback, useEffect, useRef, useState } from 'react';
import type { DraftInput } from '../../api/types';

export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error';

/** Enregistre le brouillon `delayMs` après la dernière modification ; `flush` enregistre tout de suite. */
export function useAutosave(save: (input: DraftInput) => Promise<unknown>, delayMs = 2000) {
  const [status, setStatus] = useState<SaveStatus>('idle');
  const pending = useRef<DraftInput | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Les enregistrements s'enchaînent : jamais deux à la fois, jamais dans le désordre.
  const chain = useRef<Promise<void>>(Promise.resolve());

  const clearTimer = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
  };

  const flush = useCallback((): Promise<void> => {
    clearTimer();
    const input = pending.current;
    pending.current = null;
    if (!input) return chain.current;
    setStatus('saving');
    const done = chain.current.then(() => save(input)).then(() => undefined);
    chain.current = done.then(
      () => setStatus(pending.current ? 'saving' : 'saved'),
      () => setStatus('error'),
    );
    return done;
  }, [save]);

  const schedule = useCallback(
    (input: DraftInput) => {
      pending.current = input;
      clearTimer();
      timer.current = setTimeout(() => void flush().catch(() => undefined), delayMs);
    },
    [flush, delayMs],
  );

  /** Abandonne ce qui n'est pas encore enregistré (brouillon envoyé ou supprimé). */
  const cancel = useCallback(() => {
    clearTimer();
    pending.current = null;
  }, []);

  useEffect(() => clearTimer, []);

  return { status, schedule, flush, cancel };
}
