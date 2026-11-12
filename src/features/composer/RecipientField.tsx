import { useEffect, useId, useRef, useState, type ClipboardEvent, type KeyboardEvent } from 'react';
import { api } from '../../api';
import type { Address } from '../../api/types';
import { IconClose } from '../../ui/icons';
import { addressLabel, isValidEmail, parseAddress, parseAddressList } from './address';
import styles from './Composer.module.css';

interface Props {
  label: string;
  value: Address[];
  onChange(value: Address[]): void;
  autoFocus?: boolean;
  /** Élément à droite du champ (lien « Cc / Cci »). */
  trailing?: React.ReactNode;
}

const SUGGEST_DELAY_MS = 150;

export function RecipientField({ label, value, onChange, autoFocus, trailing }: Props) {
  const [text, setText] = useState('');
  const [suggestions, setSuggestions] = useState<Address[]>([]);
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const inputId = useId();

  useEffect(() => {
    const q = text.trim();
    if (!q) {
      setSuggestions([]);
      return;
    }
    let cancelled = false;
    const t = setTimeout(() => {
      api
        .searchContacts(q)
        .then((found) => {
          if (cancelled) return;
          const already = new Set(value.map((a) => a.email.toLowerCase()));
          setSuggestions(found.filter((a) => !already.has(a.email.toLowerCase())));
          setActive(0);
        })
        .catch(() => undefined);
    }, SUGGEST_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [text, value]);

  const add = (list: Address[]) => {
    if (list.length === 0) return;
    const seen = new Set(value.map((a) => a.email.toLowerCase()));
    const fresh = list.filter((a) => !seen.has(a.email.toLowerCase()) && (seen.add(a.email.toLowerCase()), true));
    onChange([...value, ...fresh]);
    setText('');
    setSuggestions([]);
  };

  const commitText = () => {
    const a = parseAddress(text);
    if (a) add([a]);
    else setText('');
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    // Ctrl+Entrée envoie : le texte en cours devient une adresse avant que l'événement remonte.
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      if (text.trim()) commitText();
      return;
    }
    if (suggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActive((i) => (i + 1) % suggestions.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActive((i) => (i - 1 + suggestions.length) % suggestions.length);
        return;
      }
      if (e.key === 'Escape') {
        e.stopPropagation();
        setSuggestions([]);
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        add([suggestions[active]]);
        return;
      }
    }
    if (e.key === 'Enter' || e.key === ',' || e.key === ';') {
      if (text.trim()) {
        e.preventDefault();
        commitText();
      } else if (e.key !== 'Enter') {
        e.preventDefault();
      }
      return;
    }
    if (e.key === 'Backspace' && text === '' && value.length > 0) {
      e.preventDefault();
      onChange(value.slice(0, -1));
    }
  };

  const onPaste = (e: ClipboardEvent<HTMLInputElement>) => {
    const pasted = e.clipboardData.getData('text');
    if (/[,;\n]/.test(pasted)) {
      e.preventDefault();
      add(parseAddressList(text + pasted));
    }
  };

  return (
    <div className={styles.field} onClick={() => inputRef.current?.focus()}>
      <label className={styles.fieldLabel} htmlFor={inputId}>
        {label}
      </label>
      <div className={styles.chips}>
        {value.map((a, i) => {
          const invalid = !isValidEmail(a.email);
          return (
            <span
              key={`${a.email}-${i}`}
              className={invalid ? `${styles.chip} ${styles.chipInvalid}` : styles.chip}
              aria-invalid={invalid}
              title={invalid ? `Adresse invalide : ${a.email}` : a.email}
            >
              {addressLabel(a)}
              <button
                type="button"
                className={styles.chipRemove}
                aria-label={`Retirer ${addressLabel(a)}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onChange(value.filter((_, j) => j !== i));
                }}
              >
                <IconClose size={12} />
              </button>
            </span>
          );
        })}
        <div className={styles.chipInputWrap}>
          <input
            ref={inputRef}
            id={inputId}
            className={styles.chipInput}
            value={text}
            autoFocus={autoFocus}
            autoComplete="off"
            spellCheck={false}
            role="combobox"
            aria-expanded={suggestions.length > 0}
            aria-controls={listId}
            aria-autocomplete="list"
            onChange={(e) => setText(e.target.value)}
            onKeyDown={onKeyDown}
            onPaste={onPaste}
            onBlur={() => {
              // Un clic sur une suggestion ne fait pas perdre le focus (mousedown annulé) :
              // quitter le champ valide donc tout de suite le texte saisi, avant un clic sur « Envoyer ».
              setSuggestions([]);
              if (text.trim()) commitText();
            }}
          />
          {suggestions.length > 0 && (
            <ul id={listId} role="listbox" className={styles.suggestions}>
              {suggestions.map((a, i) => (
                <li
                  key={a.email}
                  role="option"
                  aria-selected={i === active}
                  className={i === active ? `${styles.suggestion} ${styles.suggestionActive}` : styles.suggestion}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    add([a]);
                  }}
                  onMouseEnter={() => setActive(i)}
                >
                  <span className={styles.suggestionName}>{a.name ?? a.email}</span>
                  {a.name && <span className={styles.suggestionEmail}>{a.email}</span>}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
      {trailing}
    </div>
  );
}
