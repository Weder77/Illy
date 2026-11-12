import { useEffect, useState } from 'react';

/** Heure courante en secondes Unix, relue toutes les `everyMs` tant que `active` est vrai. */
export function useNow(everyMs: number, active = true): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    if (!active) return;
    setNow(Math.floor(Date.now() / 1000));
    const t = setInterval(() => setNow(Math.floor(Date.now() / 1000)), everyMs);
    return () => clearInterval(t);
  }, [everyMs, active]);
  return now;
}
