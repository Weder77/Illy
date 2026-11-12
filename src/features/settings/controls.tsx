import type { ReactNode } from 'react';
import styles from './Settings.module.css';

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>{title}</h2>
      {children}
    </section>
  );
}

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <div className={styles.rowText}>
        <div className={styles.rowLabel}>{label}</div>
        {hint && <div className={styles.rowHint}>{hint}</div>}
      </div>
      <div className={styles.rowControl}>{children}</div>
    </div>
  );
}

/** Choix exclusif en pastilles (radiogroup accessible). */
export function Choice<T extends string | number>(props: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange(value: T): void;
}) {
  return (
    <div role="radiogroup" aria-label={props.label} className={styles.choice}>
      {props.options.map((o) => (
        <button
          key={String(o.value)}
          type="button"
          role="radio"
          aria-checked={o.value === props.value}
          className={o.value === props.value ? `${styles.choiceBtn} ${styles.choiceOn}` : styles.choiceBtn}
          onClick={() => o.value !== props.value && props.onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Pastilles de couleur (radiogroup). */
export function Swatches(props: {
  label: string;
  value: string;
  options: { value: string; color: string; label: string }[];
  onChange(value: string): void;
}) {
  return (
    <div role="radiogroup" aria-label={props.label} className={styles.swatches}>
      {props.options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value.toLowerCase() === props.value.toLowerCase()}
          aria-label={o.label}
          title={o.label}
          className={styles.swatch}
          style={{ background: o.color }}
          onClick={() => props.onChange(o.value)}
        />
      ))}
    </div>
  );
}
