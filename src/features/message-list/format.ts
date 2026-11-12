const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const pad = (n: number) => String(n).padStart(2, '0');
const weekday = new Intl.DateTimeFormat('fr-FR', { weekday: 'long' });

/** Date courte de la liste : « 14:32 », « hier », « lundi », « 04/03 » ou « 04/05/2023 ». */
export function formatListDate(ts: number, now: Date = new Date()): string {
  const d = new Date(ts * 1000);
  const days = Math.round((startOfDay(now).getTime() - startOfDay(d).getTime()) / 86_400_000);
  if (days === 0) return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (days === 1) return 'hier';
  if (days > 1 && days < 7) return weekday.format(d);
  if (d.getFullYear() === now.getFullYear()) return `${pad(d.getDate())}/${pad(d.getMonth() + 1)}`;
  return `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()}`;
}

export function senderName(from: { name: string | null; email: string }): string {
  return from.name?.trim() || from.email || '(expéditeur inconnu)';
}
