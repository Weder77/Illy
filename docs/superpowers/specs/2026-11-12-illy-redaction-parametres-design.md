# Illy — Lots A (rédaction) et B (paramètres, mises à jour) — Design

*Date : 2026-11-12 · Statut : en relecture · Complète la spec principale `2026-11-11-illy-client-mail-design.md`, qui reste la référence pour tout ce qui n'est pas redéfini ici.*

## 1. Contexte

Le jalon 1 (comptes, synchro, lecture) est installé et fonctionne sur le PC du propriétaire, avec Gmail. Après installation, il demande :

- de **pouvoir écrire** des mails : nouveau mail, répondre, transférer ;
- un **vrai écran Paramètres** : comptes, apparence, réglages divers ;
- des **mises à jour récupérées depuis les releases GitHub**, sans retélécharger le `.msi` à chaque fois ;
- un **clic droit** sur un mail pour accéder aux actions, comme dans Spark.

Ordre des lots décidé avec lui : **A (rédaction) → B (paramètres) → C (actions) → D (recherche)**. Le tableau des jalons de la spec principale (§ 12) est remplacé par le § 6 ci-dessous.

## 2. Décisions

| Sujet | Décision |
|---|---|
| Annulation d'envoi | Oui : bandeau « Envoi dans N s · Annuler ». Délai réglable 5 / 10 / 30 s, **10 s par défaut**. |
| Signature | Insérée automatiquement dès le lot A ; elle se règle dans Paramètres › Comptes (lot B). Vide par défaut. |
| Éditeur | **TipTap** (ProseMirror), commun à la réponse sous le mail et à la fenêtre « Nouveau mail ». |
| Envoi | Géré par le **cœur Rust** (file `ops`, avec échéance = maintenant + délai d'annulation), pas par l'interface. |
| Réglages | Stockés dans la **base locale** (table `settings`), partagés par toutes les fenêtres. |
| Paramètres | Grand panneau par-dessus la fenêtre principale, menu à gauche : Comptes · Apparence · Lecture et rédaction · Mises à jour · À propos. |
| Mises à jour | Module `updater` de Tauri + releases GitHub du dépôt public `Weder77/Illy`, signées. **Jamais automatiques** : un bandeau propose « Installer ». |
| Clic droit | Lot A : Répondre, Répondre à tous, Transférer. Lot C : toutes les actions (non lu, déplacer, spam, supprimer). |
| Préfixes | « Re: » et « Fwd: », non ajoutés si l'objet commence déjà par re:, fwd:, fw:, tr: (sans tenir compte de la casse). |

## 3. Lot A — Rédaction

### 3.1 Données (schéma v2)

- **Nouvelle table `settings`** : `key TEXT PRIMARY KEY`, `value_json TEXT NOT NULL`. Clés et valeurs par défaut :
  - `undo_send_seconds` : 10 (valeurs permises 5, 10, 30) ;
  - `remote_images` : `"block"` (ou `"always"`) ;
  - `theme` : `"dark"` (ou `"light"`, `"system"`) ;
  - `accent` : `"blue"` ;
  - `text_size` : `"normal"` (ou `"small"`, `"large"`) ;
  - `density` : `"comfortable"` (ou `"compact"`) ;
  - `show_snippet` : true.

  Une clé absente vaut sa valeur par défaut.
- **`messages.reply_to_json`** (nouvelle colonne, `'[]'` par défaut) : l'en-tête `Reply-To` est désormais récupéré (ajouté à `HEADER.FIELDS`). Les mails déjà synchronisés n'en ont pas ; la réponse se fait alors à l'expéditeur.
- **`contacts`** (table existante, enfin alimentée) :
  - chaque nouvel en-tête stocké met à jour ses adresses From, To, Cc et Reply-To : `count + 1`, `last_seen_at` = max des dates, `name` remplacé s'il n'était pas vide ;
  - les adresses de l'utilisateur lui-même sont exclues ;
  - recherche : préfixe sur l'adresse ou sur un mot du nom, sans tenir compte des accents ni de la casse ; 8 résultats au plus, triés par `count` décroissant puis `last_seen_at` décroissant.
- **`drafts`** (table existante) : on ajoute `state` (`editing` | `queued` | `sending` | `failed`) et `last_error`.
  - Les pièces jointes ajoutées depuis le disque sont **copiées** dans `%LOCALAPPDATA%\Illy\drafts\<draft_id>\`, pour ne pas dépendre du fichier d'origine.
  - Les pièces jointes reprises d'un mail transféré sont décrites par `{message_id, part_path}` et téléchargées au moment de l'envoi.
- **`ops`** : nouveaux types `send`, `append_draft`, `set_answered`, et une colonne `started_at`. Une action déjà commencée ne peut plus être annulée.

### 3.2 API du cœur (nouvelles commandes)

| Méthode | Rôle |
|---|---|
| `search_contacts(query) -> Vec<Address>` | Autocomplétion des destinataires |
| `create_draft(kind, account_id?, source_message_id?) -> Draft` | `kind` vaut `new`, `reply`, `reply_all` ou `forward` ; brouillon prérempli (voir 3.3) |
| `draft_for_message(message_id) -> Option<Draft>` | Réponse en cours sur ce mail, pour proposer « Reprendre la réponse » |
| `draft_from_message(message_id) -> Draft` | Ouvre un mail du dossier Brouillons pour le modifier |
| `get_draft(id)` / `update_draft(input)` | Lecture ; sauvegarde locale (l'interface appelle `update_draft` 2 s après la dernière frappe) |
| `add_attachments(draft_id, paths)` / `remove_attachment(draft_id, index)` | Pièces jointes |
| `close_draft(id)` | Fermeture de l'éditeur : copie du brouillon sur le serveur, sauf s'il est vide |
| `discard_draft(id)` | Suppression locale et serveur |
| `send_draft(id) -> SendTicket { send_at }` | Valide, puis place l'envoi en file avec l'échéance `maintenant + undo_send_seconds` |
| `cancel_send(id) -> bool` | `true` si l'envoi a été annulé à temps (le brouillon redevient `editing`) |
| `list_outbox() -> Vec<OutboxItem>` | Envois en attente, en cours ou en échec |
| `get_settings()` / `set_setting(key, value)` | Réglages (utilisés dès le lot A pour le délai d'annulation) |

Validation de `send_draft` : au moins un destinataire et toutes les adresses valides. Sinon `InvalidInput`, avec un message en français qui nomme l'adresse fautive.

Nouveaux événements :
- `outbox_changed { draftId, state, sendAt?, error? }` ;
- `settings_changed { key }`, émis vers **toutes** les fenêtres.

### 3.3 Préremplissage

- **Répondre** :
  - À : `Reply-To`, sinon l'expéditeur ;
  - Objet : « Re: » + l'objet ;
  - `In-Reply-To` = le `Message-ID` d'origine ; `References` = les références d'origine suivies de ce `Message-ID` ;
  - De : le compte qui a reçu le mail.
- **Répondre à tous** : comme Répondre, et Cc = (À + Cc d'origine), sans les adresses de l'utilisateur ni celles déjà dans À.
- **Transférer** :
  - aucun destinataire ; Objet : « Fwd: » + l'objet ;
  - les pièces jointes d'origine sont reprises ;
  - en tête de la citation : « ---------- Message transféré ---------- » suivi de De, Date, Objet et À.
- **Corps** : une ligne vide, puis la signature du compte « De » si elle n'est pas vide, puis la citation.
  - La signature est entourée de `<div data-illy-signature>`. Changer de compte « De » remplace ce bloc.
  - Citation d'une réponse : « Le {jeudi 12 novembre 2026 à 14:32}, {Nom} a écrit : », suivie d'un `<blockquote>` contenant le HTML d'origine **nettoyé** (`sanitize_html`, images distantes toujours bloquées) ou le texte converti par `text_to_html`.
- **Nouveau mail** : De = le compte du dossier affiché, sinon le premier compte ; corps = une ligne vide puis la signature.

### 3.4 Envoi et brouillons (cœur)

Le **message MIME** est construit avec le builder de `lettre` :
- en-têtes `From` (nom affiché + adresse), `To`, `Cc`, `Bcc` (pour SMTP seulement, jamais écrit dans les copies), `Subject`, `Date`, `Message-ID` `<uuid@domaine-de-l-expéditeur>`, `In-Reply-To` et `References` ;
- `multipart/alternative` : un texte (`html_to_text` du HTML) et le HTML ;
- `multipart/related` pour les images collées : les URL `data:` de l'éditeur deviennent des parties `cid:` ;
- `multipart/mixed` pour les pièces jointes ;
- le HTML envoyé est celui de l'éditeur, passé par une liste blanche TipTap (pas de script possible).

**Op `send`** :
1. Le worker marque `started_at` ; si l'op a été annulée entre-temps, il ne fait rien.
2. Envoi SMTP (`lettre`, avec le mot de passe du trousseau).
3. En cas de succès :
   - copie dans Envoyés : après 15 s, `UID SEARCH HEADER Message-ID` dans le dossier `sent` ; si le mail est absent, `APPEND` avec `\Seen`. Gmail l'y place déjà lui-même, Illy ne le double pas ;
   - `\Answered` sur le mail d'origine (op `set_answered`) ;
   - suppression de la copie serveur du brouillon (`\Deleted` + `UID EXPUNGE`) ;
   - suppression du brouillon local et de ses fichiers ;
   - événement `outbox_changed` avec l'état `sent`.
4. En cas d'échec :
   - réseau : nouvelle tentative avec backoff, état `queued` ;
   - mot de passe refusé : état `failed`, sans nouvelle tentative, et le compte passe en `auth_error` ;
   - refus définitif du serveur (5xx, destinataire refusé…) : état `failed` avec le message du serveur.

**Op `append_draft`** : copie du brouillon dans le dossier `drafts`, avec les drapeaux `\Draft \Seen` et l'en-tête `X-Illy-Draft: <id>`.
- L'ancienne copie (`remote_uid`) est supprimée.
- Le nouvel UID est retrouvé par `UID SEARCH HEADER X-Illy-Draft <id>`.
- L'op est déclenchée au plus une fois par minute pendant la rédaction, et à la fermeture de l'éditeur.

**Sans réseau** : l'envoi attend dans la file et part dès le retour de la connexion.

### 3.5 Interface

- **Composant `Composer`** (`src/features/composer/`), commun à la réponse sous le mail et à la fenêtre « Nouveau mail ». Il contient :
  - De : liste des comptes, avec la pastille de couleur ;
  - À : pastilles d'adresses avec autocomplétion. Entrée, virgule ou point-virgule valident une adresse ; une adresse invalide s'affiche en rouge. « Cc / Cci » est un lien qui révèle les deux champs ;
  - Objet ;
  - barre d'outils TipTap : gras, italique, souligné, liste à puces, liste numérotée, lien, citation, image, pièce jointe ;
  - zone de texte ;
  - pièces jointes en pastilles (nom, taille, ✕) ;
  - pied : **Envoyer** (bouton principal, Ctrl+Entrée), corbeille « Supprimer le brouillon », état « Brouillon enregistré » ou « Enregistrement… ».
  - Images collées ou déposées dans le texte : 5 Mo au plus par image.
  - Pièces jointes : par le bouton (dialogue de fichiers Windows) ou par glisser-déposer sur l'éditeur.
  - Au-delà de 25 Mo de pièces jointes, une confirmation : « Les pièces jointes font 31 Mo. Gmail et la plupart des serveurs refusent au-delà de 25 Mo. Envoyer quand même ? ».
- **Répondre sous le mail** : l'éditeur s'ouvre sous la carte du mail et défile jusqu'à elle.
  - Changer de mail enregistre le brouillon et ferme l'éditeur.
  - Revenir sur ce mail affiche « Reprendre la réponse » (`draft_for_message`).
- **Fenêtre « Nouveau mail »** : une fenêtre Tauri `compose-<draftId>` de 760×640, sans cadre système, avec la barre de titre d'Illy.
  - Elle charge `index.html#compose/<draftId>` ; `main.tsx` choisit la vue d'après l'adresse.
  - Fermer la fenêtre enregistre le brouillon (`close_draft`).
- **Envoyer** :
  - la fenêtre « Nouveau mail » se ferme aussitôt, et l'éditeur sous le mail se ferme ;
  - la **fenêtre principale** affiche en bas un bandeau « Envoi dans 10 s · Annuler », avec un compte à rebours ;
  - « Annuler » appelle `cancel_send` et rouvre l'éditeur avec le brouillon ;
  - après l'envoi : « Mail envoyé » pendant 3 s.
- **Échec d'envoi** : un bandeau persistant « Échec de l'envoi à {destinataire} : {raison} » avec « Réessayer » et « Modifier ».
- **Barre de titre** : une pastille « N envoi(s) en attente » tant que la file contient des envois non partis (hors délai d'annulation).
- **Barre d'actions du lecteur** : Répondre, Répondre à tous et Transférer deviennent actifs. « Nouveau mail » (panneau de gauche) aussi.
- **Mail du dossier Brouillons** : le lecteur affiche « Modifier ce brouillon » (`draft_from_message`).
- **Clic droit sur une ligne de la liste** :
  - un menu contextuel (`src/ui/ContextMenu.tsx`) avec Répondre, Répondre à tous et Transférer ;
  - navigable au clavier (↑ ↓ Entrée Échap) ;
  - il se ferme au clic extérieur ;
  - il est extensible au lot C.

### 3.6 Tests

- **Rust, tests unitaires** :
  - préremplissage (préfixes, Reply-To, exclusion des adresses de l'utilisateur, citation, signature) ;
  - construction MIME (alternative / related / mixed, `cid:`, `Bcc` absent des copies, en-têtes de réponse) ;
  - alimentation et recherche des contacts (accents, casse, tri) ;
  - réglages (valeurs par défaut, valeurs refusées) ;
  - annulation (`started_at`).
- **Rust, tests d'intégration GreenMail**, avec un second utilisateur `bob@localhost` :
  - l'envoi arrive dans la boîte de bob ;
  - la copie dans Envoyés est ajoutée une seule fois ;
  - `\Answered` est posé sur le mail d'origine ;
  - le brouillon est copié, remplacé puis supprimé sur le serveur ;
  - une annulation dans le délai n'envoie rien ;
  - une coupure (worker arrêté) laisse l'envoi en file, puis il part au redémarrage ;
  - un mot de passe refusé donne l'état `failed`.
- **Interface (Vitest)** :
  - pastilles et autocomplétion ;
  - Ctrl+Entrée ;
  - confirmation au-delà de 25 Mo ;
  - bandeau d'annulation ;
  - menu contextuel au clavier ;
  - « Reprendre la réponse ».
- **Liste de vérification manuelle** `docs/checklists/lot-a.md`, sur Gmail, Yahoo et OVH : envoi, réponse, transfert avec pièce jointe, annulation, brouillon visible dans le webmail, envoi hors ligne.

## 4. Lot B — Paramètres et mises à jour

### 4.1 Panneau Paramètres

La roue dentée ouvre un panneau de 960×680 au plus, centré sur la fenêtre assombrie. Échap le ferme. Le menu à gauche contient : **Comptes · Apparence · Lecture et rédaction · Mises à jour · À propos**. L'ajout de compte (assistant existant) devient le bouton « Ajouter un compte » de la section Comptes.

### 4.2 Comptes

- **Liste des comptes**, réordonnable par glisser-déposer (`reorder_accounts(ids)`, qui met à jour `sort_order`). L'ordre s'applique au panneau de gauche.
- **Pour le compte sélectionné** :
  - nom affiché (utilisé dans `From`) ;
  - couleur : 6 pastilles, la palette des comptes ;
  - **signature** : éditeur TipTap, enregistrée 1 s après la dernière frappe (`update_account(id, {display_name?, color?, signature_html?})`) ;
  - « Changer le mot de passe » : la boîte de dialogue existante ;
  - serveurs : modifiables pour OVH et les comptes manuels, en lecture seule pour Gmail et Yahoo. « Tester et enregistrer » teste IMAP et SMTP avec le mot de passe enregistré (`update_servers`), puis redémarre le worker ;
  - « Supprimer ce compte » demande confirmation : « Supprimer moi@… d'Illy ? Tes mails restent sur le serveur ; seules les données de ce PC sont effacées. » Puis `remove_account`.

### 4.3 Apparence

Les réglages s'appliquent **en direct**, dans toutes les fenêtres, par des attributs sur `<html>` : `data-theme`, `data-accent`, `data-text-size`, `data-density`, `data-snippet`.

- **Thème** : sombre (jetons actuels), clair ou automatique. L'automatique suit `prefers-color-scheme`, donc le thème de Windows, en direct. Jetons du thème clair :

| Jeton | Clair |
|---|---|
| Fond | `#F4F6FA` |
| Panneau | `#FFFFFF` |
| Surélevé / ligne sélectionnée | `#EEF2F8` |
| Bordure | `rgba(16,24,40,0.08)` |
| Texte | `#101828` |
| Texte doux | `#344054` |
| Texte secondaire | `#667085` |
| Halo | `0 0 14px rgba(47,123,255,0.25)` |

- **Couleur d'accent** (valeur d'accent / fond des boutons à texte blanc) :
  - Bleu `#2F7BFF` / `#2A6FF0` ;
  - Violet `#8B5CF6` / `#7C4DEB` ;
  - Vert `#10B981` / `#0E8F64` ;
  - Orange `#F59E0B` / `#B45309` ;
  - Rose `#EC4899` / `#DB2777` ;
  - Turquoise `#14B8A6` / `#0F8A7D`.

  La pastille d'un compte garde sa propre couleur, même si elle est identique à l'accent.
- **Taille du texte** : taille de base 13 px, 14 px ou 15,5 px.
- **Densité et extrait**, hauteur d'une ligne de la liste :

| | Avec extrait | Sans extrait |
|---|---|---|
| Confortable | 76 px | 58 px |
| Compacte | 62 px | 48 px |

### 4.4 Lecture et rédaction

- **Images distantes** : « Bloquées (recommandé) » ou « Toujours afficher ». Avec `always`, `message_detail` renvoie `remote_images_allowed = true`.
- **Expéditeurs autorisés** pour les images : la liste, avec un bouton « Retirer » (`list_image_allowlist`, `remove_from_image_allowlist`).
- **Délai d'annulation d'envoi** : 5, 10 ou 30 s.

### 4.5 Mises à jour

- `tauri-plugin-updater`, avec l'endpoint `https://github.com/Weder77/Illy/releases/latest/download/latest.json` et la clé publique de signature dans `tauri.conf.json`.
- Vérification 10 s après le démarrage, puis par le bouton « Rechercher des mises à jour ».
- Si une version plus récente existe, la fenêtre principale affiche le bandeau « Illy {version} est disponible · Installer · Plus tard ».
  - « Installer » télécharge (avec la progression), lance l'installateur NSIS en mode passif, puis relance Illy.
  - « Plus tard » masque le bandeau jusqu'au prochain démarrage.
  - **Aucune installation sans clic.**
- **Publication** : `.github/workflows/release.yml`, déclenché par un tag `v*`, utilise `tauri-apps/tauri-action`. Il compile, signe (secrets `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) et crée la release GitHub avec les installateurs et `latest.json` (`bundle.createUpdaterArtifacts: true`).
- `scripts/bump-version.mjs <x.y.z>` met à jour la version dans `package.json`, `Cargo.toml` (workspace) et `tauri.conf.json`.
- `docs/release.md` décrit :
  - la configuration unique du propriétaire : générer la clé sur son PC (`npx @tauri-apps/cli signer generate -w %USERPROFILE%\.tauri\illy.key`), ajouter les deux secrets dans GitHub, transmettre la **clé publique** pour qu'elle soit inscrite dans `tauri.conf.json` ;
  - la publication : version, commit, tag, push.
- La première version qui contient le module de mise à jour s'installe une dernière fois à la main.

### 4.6 À propos

- Version d'Illy.
- « Ouvrir le dossier des journaux » : ouvre `%LOCALAPPDATA%\Illy\logs` dans l'Explorateur.
- Espace utilisé : base, brouillons et dossier temporaire.
- **« Vider le cache local »** :
  - demande confirmation ;
  - est refusé tant que des envois sont en file (« Attends la fin des envois en cours ») ;
  - arrête les workers, supprime dossiers, mails, parties et actions, puis `VACUUM` ;
  - garde les comptes, mots de passe, réglages, expéditeurs autorisés et brouillons locaux ;
  - redémarre la synchro.

### 4.7 Tests

- **Rust** :
  - `reorder_accounts`, `update_account`, `update_servers` (refus avec un mauvais serveur, rien d'enregistré), réglages `remote_images: always` ;
  - vider le cache (comptes et réglages gardés, mails resynchronisés, refus si la file d'envoi n'est pas vide) ;
  - `bump-version` (les 3 fichiers modifiés).
- **Interface** : navigation du panneau, application en direct des attributs `data-*`, glisser-déposer de l'ordre, confirmation de suppression, bandeau de mise à jour (« Plus tard » masque, « Installer » appelle le module).
- **Liste de vérification manuelle** `docs/checklists/lot-b.md`, dont une vraie mise à jour : publier v0.3.1 et vérifier que la v0.3.0 installée la propose et l'installe.

## 5. Erreurs

| Situation | Comportement |
|---|---|
| Destinataire invalide | Pastille rouge ; « Envoyer » refusé avec « Adresse invalide : {adresse} » |
| Fermeture de la fenêtre pendant le délai d'annulation | L'envoi part quand même ; l'annulation reste possible depuis le bandeau de la fenêtre principale |
| Fermeture d'Illy pendant le délai d'annulation | L'envoi reste en file et part au prochain démarrage |
| Pièce jointe introuvable au moment de l'envoi | État `failed` : « Pièce jointe introuvable : {nom} » |
| Mise à jour : téléchargement ou signature invalide | Bandeau « La mise à jour n'a pas pu être installée : {raison} » ; la version actuelle continue de fonctionner |

## 6. Jalons (remplace le § 12 de la spec principale)

| # | Lot | Contenu |
|---|---|---|
| 1 | ✅ Fondations, comptes, synchro, lecture | Livré (version 0.1.0) |
| A | Rédaction | § 3 de ce document ; version 0.2.0 |
| B | Paramètres et mises à jour | § 4 ; version 0.3.0 (la première à se mettre à jour toute seule ensuite) |
| C | Actions | Marquer non lu, supprimer, déplacer, spam, dossiers, clic droit complet (spec principale § 5) |
| D | Recherche | Spec principale § 12, ancien jalon 3 |
