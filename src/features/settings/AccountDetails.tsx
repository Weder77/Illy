import { useEffect, useRef, useState } from 'react';
import { api } from '../../api';
import { ACCOUNT_COLORS, errorMessage, type Account, type AccountPatch, type ServerConfig, type TlsMode } from '../../api/types';
import { useAppStore } from '../../state/store';
import { Row, Swatches } from './controls';
import { SignatureEditor } from './SignatureEditor';
import styles from './Settings.module.css';

const TLS_LABEL: Record<TlsMode, string> = { implicit: 'SSL/TLS', start_tls: 'STARTTLS', plain: 'Aucun' };

function ServerFields({ label, value, onChange }: { label: string; value: ServerConfig; onChange(v: ServerConfig): void }) {
  return (
    <div className={styles.serverRow}>
      <label className={styles.field}>
        <span>Serveur {label}</span>
        <input className={styles.input} value={value.host} onChange={(e) => onChange({ ...value, host: e.target.value })} />
      </label>
      <label className={styles.fieldSmall}>
        <span>Port {label}</span>
        <input className={styles.input} inputMode="numeric" value={value.port} onChange={(e) => onChange({ ...value, port: Number(e.target.value) || 0 })} />
      </label>
      <label className={styles.fieldSmall}>
        <span>Sécurité {label}</span>
        <select className={styles.input} value={value.tls} onChange={(e) => onChange({ ...value, tls: e.target.value as TlsMode })}>
          <option value="implicit">{TLS_LABEL.implicit}</option>
          <option value="start_tls">{TLS_LABEL.start_tls}</option>
        </select>
      </label>
    </div>
  );
}

export function AccountDetails({ account, onError }: { account: Account; onError(message: string): void }) {
  const setPasswordDialogFor = useAppStore((s) => s.setPasswordDialogFor);
  const [name, setName] = useState(account.displayName);
  const [imap, setImap] = useState(account.imap);
  const [smtp, setSmtp] = useState(account.smtp);
  const [serverState, setServerState] = useState<{ kind: 'idle' | 'testing' | 'ok' } | { kind: 'error'; message: string }>({ kind: 'idle' });
  const fixedServers = account.provider === 'gmail' || account.provider === 'yahoo';
  // Le nom est enregistré en quittant le champ… ou en fermant le panneau (Échap, clic à côté).
  const nameRef = useRef(name);
  nameRef.current = name;
  useEffect(
    () => () => {
      const n = nameRef.current.trim();
      if (n && n !== account.displayName) {
        void api
          .updateAccount(account.id, { displayName: n })
          .then((a) => useAppStore.setState((s) => ({ accounts: s.accounts.map((x) => (x.id === a.id ? a : x)) })))
          .catch(() => undefined);
      }
    },
    [account.id, account.displayName],
  );

  const patch = (p: AccountPatch) => {
    api
      .updateAccount(account.id, p)
      .then((a) => useAppStore.setState((s) => ({ accounts: s.accounts.map((x) => (x.id === a.id ? a : x)) })))
      .catch((e) => onError(errorMessage(e)));
  };

  const saveServers = () => {
    setServerState({ kind: 'testing' });
    api
      .updateServers(account.id, imap, smtp)
      .then((a) => {
        useAppStore.setState((s) => ({ accounts: s.accounts.map((x) => (x.id === a.id ? a : x)) }));
        setServerState({ kind: 'ok' });
      })
      .catch((e) => setServerState({ kind: 'error', message: errorMessage(e) }));
  };

  const remove = () => {
    const question = `Supprimer ${account.email} d'Illy ? Tes mails restent sur le serveur ; seules les données de ce PC sont effacées.`;
    if (!window.confirm(question)) return;
    api
      .removeAccount(account.id)
      .then(() => useAppStore.getState().init())
      .catch((e) => onError(errorMessage(e)));
  };

  return (
    <div className={styles.details}>
      <div className={styles.detailsTitle}>{account.email}</div>
      <Row label="Nom affiché" hint="Le nom que voient tes destinataires.">
        <input
          className={styles.input}
          aria-label="Nom affiché"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name.trim() && name.trim() !== account.displayName) patch({ displayName: name.trim() });
            else setName(account.displayName);
          }}
          onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
        />
      </Row>
      <Row label="Couleur">
        <Swatches
          label="Couleur du compte"
          value={account.color}
          options={ACCOUNT_COLORS.map((c) => ({ value: c, color: c, label: `Couleur ${c}` }))}
          onChange={(color) => patch({ color })}
        />
      </Row>
      <div className={styles.block}>
        <div className={styles.rowLabel}>Signature</div>
        <SignatureEditor value={account.signatureHtml ?? ''} onSave={(signatureHtml) => patch({ signatureHtml })} />
      </div>
      <Row label="Mot de passe" hint="Le mot de passe (d'application) est gardé dans le Gestionnaire d'identification Windows.">
        <button type="button" className={styles.secondaryBtn} onClick={() => setPasswordDialogFor(account.id)}>
          Changer le mot de passe
        </button>
      </Row>
      <div className={styles.block}>
        <div className={styles.rowLabel}>Serveurs</div>
        {fixedServers ? (
          <p className={styles.muted}>{`IMAP ${account.imap.host}:${account.imap.port} · SMTP ${account.smtp.host}:${account.smtp.port}`}</p>
        ) : (
          <>
            <ServerFields label="IMAP" value={imap} onChange={setImap} />
            <ServerFields label="SMTP" value={smtp} onChange={setSmtp} />
            <div className={styles.inline}>
              <button type="button" className={styles.primaryBtn} onClick={saveServers} disabled={serverState.kind === 'testing'}>
                {serverState.kind === 'testing' ? 'Test en cours…' : 'Tester et enregistrer'}
              </button>
              {serverState.kind === 'ok' && <span className={styles.ok}>Serveurs enregistrés</span>}
              {serverState.kind === 'error' && (
                <span className={styles.errorText} role="alert">
                  {serverState.message}
                </span>
              )}
            </div>
          </>
        )}
      </div>
      <div className={styles.danger}>
        <button type="button" className={styles.dangerBtn} onClick={remove}>
          Supprimer ce compte
        </button>
      </div>
    </div>
  );
}
