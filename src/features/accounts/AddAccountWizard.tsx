import { useState, type FormEvent } from 'react';
import { api } from '../../api';
import type { NewAccount, Provider, ServerConfig } from '../../api/types';
import { useAppStore } from '../../state/store';
import { accountErrorText } from './errors';
import styles from './Accounts.module.css';

const OVH_IMAP: ServerConfig = { host: 'ssl0.ovh.net', port: 993, tls: 'implicit' };
const OVH_SMTP: ServerConfig = { host: 'ssl0.ovh.net', port: 465, tls: 'implicit' };

const HELP: Partial<Record<Provider, { name: string; steps: string[]; links: [string, string][] }>> = {
  gmail: {
    name: 'Gmail',
    steps: [
      'Active la validation en deux étapes sur ton compte Google.',
      "Vérifie qu'IMAP est activé dans Gmail (Paramètres › Transfert et POP/IMAP).",
      "Crée un mot de passe d'application et colle-le ci-dessous.",
    ],
    links: [
      ['Ouvrir la page des mots de passe d’application', 'https://myaccount.google.com/apppasswords'],
      ['Ouvrir les réglages IMAP de Gmail', 'https://mail.google.com/mail/u/0/#settings/fwdandpop'],
    ],
  },
  yahoo: {
    name: 'Yahoo',
    steps: [
      'Active la validation en deux étapes sur ton compte Yahoo.',
      "Crée un mot de passe d'application et colle-le ci-dessous.",
    ],
    links: [['Ouvrir la sécurité du compte Yahoo', 'https://login.yahoo.com/account/security']],
  },
};

type Step = 1 | 2 | 3;
type Hosting = 'ovh' | 'manual' | null;

const isEmail = (v: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim());

export function AddAccountWizard({ onClose }: { onClose: () => void }) {
  const firstRun = useAppStore((s) => s.accounts.length === 0);
  const loadAccounts = useAppStore((s) => s.loadAccounts);
  const setFocusAccount = useAppStore((s) => s.setFocusAccount);

  const [step, setStep] = useState<Step>(1);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [provider, setProvider] = useState<Provider | null>(null);
  const [detected, setDetected] = useState<{ imap: ServerConfig; smtp: ServerConfig } | null>(null);
  const [unknownDomain, setUnknownDomain] = useState(false);
  const [hosting, setHosting] = useState<Hosting>(null);
  const [imap, setImap] = useState<ServerConfig>({ host: '', port: 993, tls: 'implicit' });
  const [smtp, setSmtp] = useState<ServerConfig>({ host: '', port: 465, tls: 'implicit' });
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ text: string; detail: string | null } | null>(null);

  const help = provider ? HELP[provider] : undefined;

  const continueAddress = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!isEmail(email)) {
      setError({ text: 'Saisis une adresse mail valide.', detail: null });
      return;
    }
    if (unknownDomain) {
      if (hosting === 'ovh') {
        setProvider('ovh');
        setDetected({ imap: OVH_IMAP, smtp: OVH_SMTP });
        setStep(2);
      } else if (hosting === 'manual') {
        if (!imap.host.trim() || !smtp.host.trim()) {
          setError({ text: 'Indique les serveurs IMAP et SMTP de ton hébergeur.', detail: null });
          return;
        }
        setProvider('custom');
        setDetected({ imap: { ...imap, host: imap.host.trim() }, smtp: { ...smtp, host: smtp.host.trim() } });
        setStep(2);
      } else {
        setError({ text: 'Choisis où est hébergée cette adresse.', detail: null });
      }
      return;
    }
    setBusy(true);
    try {
      const guess = await api.detectProvider(email.trim());
      if (guess.provider && guess.imap && guess.smtp) {
        setProvider(guess.provider);
        setDetected({ imap: guess.imap, smtp: guess.smtp });
        setStep(2);
      } else {
        setUnknownDomain(true);
      }
    } catch (err) {
      setError(accountErrorText(err));
    } finally {
      setBusy(false);
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!detected || !provider || !password) return;
    setError(null);
    setStep(3);
    setBusy(true);
    const input: NewAccount = { email: email.trim(), displayName: name.trim(), provider, imap: detected.imap, smtp: detected.smtp };
    try {
      const account = await api.addAccount(input, password);
      setFocusAccount(account.id);
      await loadAccounts();
      onClose();
    } catch (err) {
      setError(accountErrorText(err));
    } finally {
      setBusy(false);
    }
  };

  const stepLabel = (n: Step, label: string) => (
    <span className={`${styles.step} ${step === n ? styles.stepActive : ''}`}>
      <span className={styles.stepNum}>{n}</span>
      {label}
    </span>
  );

  return (
    <div className={styles.overlay} role="dialog" aria-modal="true" aria-labelledby="wizard-title">
      <div className={styles.card}>
        <h2 id="wizard-title" className={styles.title}>
          {firstRun ? 'Bienvenue dans Illy' : 'Ajouter un compte'}
        </h2>
        <p className={styles.lead}>
          {firstRun ? 'Commence par ajouter une adresse mail.' : 'Ajoute une autre adresse mail.'}
        </p>
        <div className={styles.steps}>
          {stepLabel(1, 'Adresse')}
          <span className={styles.stepSep}>—</span>
          {stepLabel(2, 'Mot de passe')}
          <span className={styles.stepSep}>—</span>
          {stepLabel(3, 'Vérification')}
        </div>

        {step === 1 && (
          <form onSubmit={continueAddress} noValidate>
            <label className={styles.field}>
              Ton nom
              <input className={styles.input} value={name} onChange={(e) => setName(e.target.value)} placeholder="Prénom Nom" autoFocus />
            </label>
            <label className={styles.field}>
              Adresse mail
              <input
                className={styles.input}
                type="email"
                value={email}
                onChange={(e) => {
                  setEmail(e.target.value);
                  setUnknownDomain(false);
                  setHosting(null);
                }}
                placeholder="toi@exemple.fr"
              />
            </label>
            {unknownDomain && (
              <div>
                <label className={styles.choice}>
                  <input type="radio" name="hosting" checked={hosting === 'ovh'} onChange={() => setHosting('ovh')} />
                  Hébergé chez OVH
                </label>
                <label className={styles.choice}>
                  <input type="radio" name="hosting" checked={hosting === 'manual'} onChange={() => setHosting('manual')} />
                  Autre hébergeur (réglages manuels)
                </label>
                {hosting === 'manual' && (
                  <>
                    <div className={styles.row}>
                      <label className={styles.field}>
                        Serveur IMAP
                        <input className={styles.input} value={imap.host} onChange={(e) => setImap({ ...imap, host: e.target.value })} placeholder="imap.exemple.fr" />
                      </label>
                      <label className={styles.field}>
                        Port IMAP
                        <input className={styles.input} inputMode="numeric" value={imap.port} onChange={(e) => setImap({ ...imap, port: Number(e.target.value) || 0 })} />
                      </label>
                    </div>
                    <div className={styles.row}>
                      <label className={styles.field}>
                        Serveur SMTP
                        <input className={styles.input} value={smtp.host} onChange={(e) => setSmtp({ ...smtp, host: e.target.value })} placeholder="smtp.exemple.fr" />
                      </label>
                      <label className={styles.field}>
                        Port SMTP
                        <input className={styles.input} inputMode="numeric" value={smtp.port} onChange={(e) => setSmtp({ ...smtp, port: Number(e.target.value) || 0 })} />
                      </label>
                    </div>
                    <label className={styles.field}>
                      Sécurité SMTP
                      <select
                        className={styles.select}
                        value={smtp.tls}
                        onChange={(e) => {
                          const tls = e.target.value as ServerConfig['tls'];
                          setSmtp({ ...smtp, tls, port: tls === 'start_tls' ? 587 : 465 });
                        }}
                      >
                        <option value="implicit">SSL/TLS (port 465)</option>
                        <option value="start_tls">STARTTLS (port 587)</option>
                      </select>
                    </label>
                  </>
                )}
              </div>
            )}
            {error && <div className={styles.error}>{error.text}</div>}
            <div className={styles.actions}>
              {!firstRun && (
                <button type="button" className={styles.secondary} onClick={onClose}>
                  Annuler
                </button>
              )}
              <button type="submit" className={styles.primary} disabled={busy}>
                Continuer
              </button>
            </div>
          </form>
        )}

        {step === 2 && (
          <form onSubmit={submit}>
            {help && (
              <div className={styles.help}>
                <p>{help.name} demande un mot de passe d'application.</p>
                <ol>
                  {help.steps.map((s) => (
                    <li key={s}>{s}</li>
                  ))}
                </ol>
                <div className={styles.links}>
                  {help.links.map(([label, url]) => (
                    <button key={url} type="button" className={styles.linkBtn} onClick={() => void api.openUrl(url)}>
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            )}
            <label className={styles.field}>
              {help ? "Mot de passe d'application" : 'Mot de passe'}
              <input className={styles.input} type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoFocus />
            </label>
            <div className={styles.actions}>
              <button type="button" className={styles.secondary} onClick={() => setStep(1)}>
                Retour
              </button>
              <button type="submit" className={styles.primary} disabled={!password}>
                Tester et ajouter
              </button>
            </div>
          </form>
        )}

        {step === 3 && (
          <div>
            {busy && (
              <div className={styles.progress}>
                <span className={styles.spinner} />
                Connexion à {detected?.imap.host}…
              </div>
            )}
            {error && (
              <div className={styles.error} role="alert">
                {error.text}
                {error.detail && <span className={styles.errorDetail}>{error.detail}</span>}
              </div>
            )}
            {!busy && (
              <div className={styles.actions}>
                <button type="button" className={styles.secondary} onClick={() => setStep(2)}>
                  Retour
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
