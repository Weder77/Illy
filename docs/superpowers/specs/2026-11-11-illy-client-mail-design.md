# Illy — client mail Windows — Design

*Date : 2026-11-11 · Statut : en relecture*

## 1. Contexte et objectif

Le propriétaire du projet utilise Spark au quotidien. C'est le meilleur client qu'il ait trouvé, mais il ne lui convient pas :

- fonctions payantes ;
- trop d'IA, dont il n'a pas besoin ;
- « marquer comme traité » **archive** le mail, alors qu'il veut seulement le passer en **lu** ;
- la boîte de réception unifiée mélange tout ; voir une seule boîte demande trop de clics ;
- mises à jour quasi quotidiennes qui changent l'outil sans qu'il l'ait demandé.

Ce qu'il garde de Spark : une **interface minimaliste en 3 colonnes** (boîtes à gauche, liste des mails au milieu, mail affiché à droite).

**Objectif :** un client mail Windows gratuit, rapide, sans IA, qui ne fait rien dans le dos de l'utilisateur, et qui remplace Spark au quotidien.

**Utilisateurs :** le propriétaire, et quelques proches à qui il donnera l'installateur.

### Critères de réussite

1. Le propriétaire utilise Illy à la place de Spark pour ses comptes Gmail, Yahoo et OVH.
2. Aucun mail n'est jamais archivé, déplacé ou modifié sans une action explicite de l'utilisateur.
3. Chaque boîte de chaque compte est accessible en **un clic** depuis le panneau de gauche.
4. Performances, mesurées sur un compte de 100 000 mails déjà synchronisé :
   - démarrage jusqu'à l'affichage de la boîte de réception : < 2 s ;
   - changement de boîte : < 150 ms ;
   - défilement de la liste fluide (60 images/s) ;
   - recherche locale : < 300 ms ;
   - mémoire au repos : < 200 Mo.
5. Un proche non technicien peut installer Illy et ajouter un compte Gmail grâce aux instructions affichées, sans aide.

## 2. Décisions prises

| Sujet | Décision |
|---|---|
| Plateforme | **Windows uniquement** (10 et 11) |
| Fournisseurs | Gmail, Yahoo, domaines perso hébergés chez **OVH** ; saisie manuelle pour tout autre serveur IMAP/SMTP |
| Connexion | **IMAP + SMTP avec mot de passe** ; pour Gmail et Yahoo, **mot de passe d'application**. Pas d'OAuth. |
| Panneau de gauche | Comptes séparés, chacun avec ses dossiers, plus une vue facultative **« Toutes les boîtes de réception »** en haut |
| Passage en « lu » | **Dès l'ouverture** du mail. Rien d'autre ne se passe : pas d'archivage. |
| Liste des mails | **Mail par mail**, du plus récent au plus ancien, sans regroupement en conversations |
| Historique | **Hybride** : les mails récents d'abord, l'historique complet ensuite en arrière-plan ; pièces jointes téléchargées seulement à l'ouverture |
| Images externes | **Bloquées par défaut** ; « Afficher les images » et « Toujours pour cet expéditeur » |
| Affichage des mails | **Couleurs d'origine**, sur une carte claire aux coins arrondis dans le panneau de droite |
| Éditeur | **Mise en forme de base** : gras, italique, souligné, listes, liens, citation, images collées |
| Signature | Une par compte (même éditeur, images possibles), **vide par défaut**. Une fois remplie, elle est insérée automatiquement. |
| Rédaction | Répondre, Répondre à tous et Transférer : **sous le mail**, dans le panneau de droite. Nouveau mail : **fenêtre séparée**. |
| Recherche | **Dans la boîte affichée**, avec un bouton « Chercher partout » |
| Style | Personnalisé, **sombre uniquement** en v1, inspiré des captures de référence (voir § 6) |
| Mises à jour | Jamais forcées ni automatiques |
| Technologie | **Tauri 2** : cœur en **Rust**, interface en **React + TypeScript** |
| Nom | **Illy** |

### Fonctions de la v1

Lire (HTML + pièces jointes) · lu / non lu · supprimer · déplacer, créer et renommer des dossiers · spam · recherche · écrire · répondre / transférer · choisir l'adresse d'envoi · signature par compte · brouillons automatiques · thème sombre.

S'y ajoute une petite fonction, non demandée explicitement mais nécessaire pour écrire confortablement : l'**autocomplétion des destinataires** à partir des adresses déjà vues dans les mails synchronisés.

### Hors périmètre

- **Plus tard :** étoile ou drapeau, envoi programmé, rappel (snooze), notifications Windows, démarrage avec Windows et icône près de l'horloge, raccourcis clavier, mode hors ligne complet, thème clair, regroupement en conversations, interface en plusieurs langues (la v1 est en français).
- **Jamais :** IA, archivage automatique, abonnement payant, télémétrie.

## 3. Architecture

```
┌──────────── Interface (React + TypeScript, dans WebView2) ────────────┐
│  Panneau boîtes  │  Liste des mails  │  Lecteur (iframe isolée)      │
│  Fenêtre « Nouveau mail »  ·  Assistant d'ajout de compte            │
└───────────── commandes Tauri ↓       ↑ événements Tauri ──────────────┘
┌───────────────────────────── Cœur Rust ───────────────────────────────┐
│  accounts   │  sync (1 worker par compte)  │  send (SMTP)            │
│  store (SQLite + FTS5)  │  mime (analyse, nettoyage HTML)  │  secrets │
│  ops (file d'attente des actions)  │  search                          │
└───────────────────────────────────────────────────────────────────────┘
```

### Principes

1. **Le serveur IMAP est la source de vérité ; la base locale en est le miroir.** On peut supprimer la base : tout se resynchronise.
2. **L'interface ne touche jamais au réseau.** Elle lit la base locale à travers des commandes Tauri. Toute l'interface reste donc instantanée, même avec un serveur lent ou absent.
3. **Les actions sont optimistes.** Elles sont écrites tout de suite en base et à l'écran, puis placées dans la file `ops`, qui les envoie au serveur. Un échec définitif annule le changement local et affiche la raison.
4. **Les comptes sont isolés.** Un worker par compte, chacun dans sa propre tâche `tokio`. La panne d'un compte n'affecte pas les autres.
5. **Le cœur prévient l'interface par des événements** (`folder-changed`, `account-status`, `sync-progress`, `message-updated`). L'interface ne recharge que ce qui a changé.

### Modules Rust

| Module | Rôle | Dépend de |
|---|---|---|
| `accounts` | CRUD des comptes, profils de fournisseurs (Gmail, Yahoo, OVH, manuel), test de connexion | `store`, `secrets` |
| `secrets` | Lecture et écriture des mots de passe dans le **Gestionnaire d'identification Windows** (crate `keyring`). Jamais en base, jamais dans les logs. | — |
| `store` | Accès SQLite (`rusqlite`, SQLite embarqué, mode WAL). Un seul thread écrit, plusieurs lisent. Migrations versionnées. | — |
| `sync` | Worker IMAP par compte : liste des dossiers, synchro initiale et incrémentale, IDLE, téléchargement des corps et des pièces jointes à la demande | `store`, `secrets`, `mime` |
| `ops` | File d'attente persistante des actions vers le serveur, avec reprise et backoff | `store`, `sync` |
| `send` | Construction MIME et envoi SMTP (`lettre`), copie dans « Envoyés » | `store`, `secrets`, `ops` |
| `mime` | Analyse des messages (`mail-parser`), extraction texte et HTML, **nettoyage HTML** (`ammonia`), images `cid:` → `data:` | — |
| `search` | Requêtes FTS5 et filtres | `store` |
| `commands` | Couche fine qui expose les commandes Tauri à l'interface | tous |

Crates principales : `tauri` 2, `tokio`, `async-imap` (IMAP, IDLE), `tokio-rustls` (TLS), `lettre` (SMTP et construction MIME), `mail-parser`, `ammonia`, `rusqlite` (`bundled`, avec FTS5), `keyring`, `tracing`, `serde`.

### Modules de l'interface

| Module | Rôle |
|---|---|
| `api/` | Enveloppes typées des commandes et événements Tauri : le seul endroit qui appelle `invoke` |
| `state/` | État global (Zustand) : compte et dossier sélectionnés, mail ouvert, recherche, statuts des comptes |
| `features/sidebar` | Panneau de gauche |
| `features/message-list` | Liste virtualisée (TanStack Virtual), barre de recherche, filtres |
| `features/reader` | En-tête, barre d'actions, bandeau images, iframe isolée, pièces jointes |
| `features/composer` | Éditeur TipTap, partagé par la réponse sous le mail et la fenêtre « Nouveau mail » |
| `features/accounts` | Assistant d'ajout de compte, réglages du compte (signature, mot de passe) |
| `ui/` | Composants de base et jetons de design (variables CSS) |

Outils : React, TypeScript strict, Vite, CSS Modules avec variables CSS, Vitest et Testing Library.

### Données sur le disque

- Base : `%LOCALAPPDATA%\Illy\illy.db`
- Logs : `%LOCALAPPDATA%\Illy\logs\` (rotation ; aucun contenu de mail ni mot de passe)
- Pièces jointes ouvertes : dossier temporaire `%LOCALAPPDATA%\Illy\tmp\`, vidé au démarrage

## 4. Modèle de données (SQLite)

- **`accounts`** : `id`, `email`, `display_name`, `provider` (`gmail` | `yahoo` | `ovh` | `custom`), `imap_host`, `imap_port`, `smtp_host`, `smtp_port`, `smtp_security` (`tls` | `starttls`), `signature_html` (vide par défaut), `color` (pastille de couleur, utilisée dans la vue unifiée), `sort_order`, `status` (`ok` | `syncing` | `auth_error` | `offline` | `error`), `last_error`.
- **`folders`** : `id`, `account_id`, `imap_path`, `display_name`, `role` (`inbox` | `sent` | `drafts` | `trash` | `spam` | `custom`), `hidden`, `uidvalidity`, `uidnext`, `highest_modseq`, `unread_count`, `total_count`, `last_sync_at`.
- **`messages`** : `id`, `account_id`, `folder_id`, `uid`, `message_id`, `in_reply_to`, `references`, `subject`, `from_name`, `from_addr`, `to_json`, `cc_json`, `date`, `seen`, `answered`, `has_attachments`, `snippet` (200 caractères), `size`, `body_state` (`none` | `ready`), `body_text`, `body_html` (déjà nettoyé, images `cid:` incluses), `has_remote_images`. Index sur `(folder_id, date DESC)` et `(account_id, message_id)`.
- **`parts`** : `message_id`, `part_path` (numéro de partie IMAP, ex. `2.1`), `mime_type`, `filename`, `size`, `content_id`, `is_attachment`, `data` (BLOB, NULL tant que non téléchargé).
- **`messages_fts`** (FTS5, tokenizer `unicode61 remove_diacritics 2`) : `subject`, `from_text`, `to_text`, `body_text`, `attachment_names`.
- **`contacts`** : `email`, `name`, `last_seen_at`, `count`. Alimentée par les en-têtes synchronisés ; sert à l'autocomplétion.
- **`image_allowlist`** : `sender_email`.
- **`drafts`** : `id`, `account_id`, `mode` (`new` | `reply` | `reply_all` | `forward`), `source_message_id`, `to_json`, `cc_json`, `bcc_json`, `subject`, `body_html`, `attachments_json`, `remote_uid`, `updated_at`.
- **`ops`** : `id`, `account_id`, `kind` (`set_seen` | `move` | `delete` | `spam` | `create_folder` | `rename_folder` | `append_draft` | `send`), `payload_json`, `attempts`, `next_attempt_at`, `last_error`, `created_at`.

## 5. Synchronisation IMAP

### Connexions

Chaque worker de compte ouvre **2 connexions IMAP TLS** (port 993) :

- une connexion **IDLE** sur la boîte de réception, renouvelée toutes les 25 minutes ;
- une connexion **de commandes** pour la synchro, les téléchargements et les actions.

### Dossiers et rôles

- Les dossiers sont listés avec `LIST`. Le rôle vient d'abord des attributs SPECIAL-USE (`\Sent`, `\Drafts`, `\Trash`, `\Junk`, `\All`, `\Flagged`, `\Important`). À défaut, il est déduit du nom : « Sent », « Envoyés », « Éléments envoyés », « INBOX.Sent », « Bulk » (spam chez Yahoo), etc.
- **Gmail :** les dossiers `\All` (Tous les messages), `\Important` et `\Flagged` (Suivis) sont marqués `hidden`, ainsi que le conteneur `[Gmail]`. Les dossiers `hidden` ne sont ni affichés ni synchronisés. Les libellés apparaissent comme des dossiers normaux. Un même mail Gmail peut donc exister dans plusieurs dossiers : c'est voulu (une ligne par dossier). Dans les résultats « Chercher partout » et dans la vue unifiée, les doublons d'un même compte sont fusionnés par `Message-ID`.
- Ordre d'affichage : Boîte de réception, Brouillons, Envoyés, Spam, Corbeille, puis les dossiers perso par ordre alphabétique.

### Synchro initiale (hybride)

1. Les en-têtes de la **boîte de réception** sont récupérés en premier, du plus récent au plus ancien, par lots de 500 (`UID FETCH` de `ENVELOPE FLAGS INTERNALDATE RFC822.SIZE BODYSTRUCTURE`). Le premier lot s'affiche dès qu'il arrive.
2. Les **corps** (parties texte et HTML, et images `cid:`, sans les pièces jointes) des mails des **30 derniers jours** sont préchargés.
3. Les autres dossiers suivent de la même façon.
4. **L'historique complet** des en-têtes et des corps se télécharge ensuite en arrière-plan, à basse priorité. Les demandes de l'utilisateur (ouvrir un mail, changer de boîte) passent toujours devant.
5. Les **pièces jointes** ne sont jamais préchargées. Elles sont téléchargées, partie par partie, quand l'utilisateur les ouvre ou les enregistre.

Les corps sont toujours récupérés avec `BODY.PEEK[...]`, pour ne **jamais** marquer un mail comme lu par effet de bord.

### Synchro incrémentale

- **Nouveaux mails** : `UID FETCH (uidnext_connu):*`.
- **Changements de drapeaux** : si le serveur gère CONDSTORE, `FETCH 1:* (FLAGS) (CHANGEDSINCE highest_modseq)`. Sinon, relecture des drapeaux des 2 000 derniers mails, puis du reste toutes les heures.
- **Suppressions** : comparaison de `UID SEARCH ALL` avec les UID connus. On utilise QRESYNC/`VANISHED` quand le serveur le gère.
- **Changement de `UIDVALIDITY`** : le dossier est vidé localement et resynchronisé.
- **Quand** :
  - la boîte de réception, dès qu'IDLE signale un changement ;
  - le dossier affiché, à son ouverture ;
  - tous les autres dossiers, toutes les 5 minutes.

### Actions vers le serveur (file `ops`)

| Action | IMAP | Particularités |
|---|---|---|
| Ouvrir un mail → lu | `UID STORE +FLAGS (\Seen)` | — |
| Marquer non lu | `UID STORE -FLAGS (\Seen)` | — |
| Supprimer | `UID MOVE` vers le dossier `trash`. Dans la Corbeille : `\Deleted` + `UID EXPUNGE`, après confirmation. | Gmail : déplacer vers la Corbeille, pas `\Deleted`, qui ne retire qu'un libellé. |
| Spam | `UID MOVE` vers le dossier `spam` | — |
| Déplacer | `UID MOVE` (repli : `COPY` + `\Deleted` + `UID EXPUNGE`) | — |
| Créer ou renommer un dossier | `CREATE` / `RENAME` | — |

Reprise après échec réseau : backoff exponentiel de 5 s à 5 min, sans limite de tentatives. Un refus du serveur (`NO` ou `BAD`) est définitif : l'action est retirée, le changement local annulé et un message affiché.

## 6. Interface

### Disposition

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ● ● ●  Illy                                                    ─  ▢  ✕  │
│ ┌───────────────┐ ┌──────────────────────┐ ┌──────────────────────────┐ │
│ │ Toutes les    │ │ ( 🔍 Rechercher…   ) │ │ Marie Dupont             │ │
│ │ boîtes   (15) │ │ [Chercher partout]   │ │ à moi · 14:32            │ │
│ │               │ │ Non lus · PJ · Dates │ │ ↩ ↩↩ → ✉ 📁 ⚠ 🗑         │ │
│ │ ▾ perso@gmail │ │──────────────────────│ │ ┌──────────────────────┐ │ │
│ │  Réception (3)│ │▌Marie Dupont   14:32 │ │ │  (carte claire :     │ │ │
│ │  Brouillons   │ │ Devis cuisine        │ │ │   le mail dans ses   │ │ │
│ │  Envoyés      │ │ Bonjour, voici le…   │ │ │   couleurs d'origine)│ │ │
│ │  Spam         │ │──────────────────────│ │ └──────────────────────┘ │ │
│ │  Corbeille    │ │ Banque X       11:05 │ │ 📎 devis.pdf  12 Ko      │ │
│ │ ▸ moi@yahoo   │ │ Votre relevé…        │ │ [ Répondre sous le mail ]│ │
│ │ ▸ contact@ovh │ │                      │ │                          │ │
│ │               │ │                      │ │                          │ │
│ │ (+ Nouveau)  ⚙│ │                      │ │                          │ │
│ └───────────────┘ └──────────────────────┘ └──────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────┘
```

- **Fenêtre sans cadre système**, avec une barre de titre personnalisée (déplacement, réduire, agrandir, fermer).
- **Trois panneaux flottants** aux coins arrondis, séparés de 12 px, sur un fond anthracite. Largeurs : gauche et milieu redimensionnables, valeurs mémorisées.
- **Panneau de gauche :**
  - « Toutes les boîtes de réception » en haut ;
  - puis chaque compte, repliable, avec ses dossiers ;
  - les non-lus dans une pastille bleue ;
  - un compte en erreur affiche un point orange et, au clic, la raison avec un bouton « Mettre à jour le mot de passe » ;
  - en bas : un bouton « Nouveau mail » bleu lumineux, et une roue dentée pour les réglages.
- **Liste du milieu :**
  - barre de recherche en pilule et bouton « Chercher partout » ;
  - filtres : Non lus, Avec pièce jointe, Dates ;
  - chaque ligne montre l'expéditeur, l'objet, un extrait, l'heure ou la date, un point bleu si le mail est non lu et un trombone s'il a une pièce jointe ;
  - ligne sélectionnée : fond légèrement plus clair et barre bleue à gauche ;
  - dans la vue unifiée et dans les résultats « Chercher partout », chaque ligne porte la pastille de couleur de son compte.
- **Panneau de droite :**
  - en-tête (expéditeur, destinataires, date) ;
  - barre d'actions : Répondre, Répondre à tous, Transférer, Marquer non lu, Déplacer, Spam, Supprimer ;
  - bandeau « Images bloquées : Afficher · Toujours pour cet expéditeur » si besoin ;
  - le mail sur une **carte claire** ;
  - les pièces jointes en pastilles (clic : ouvrir ; menu : enregistrer sous) ;
  - la zone de réponse sous le mail.

### Jetons de design (thème sombre)

| Jeton | Valeur |
|---|---|
| Fond de la fenêtre | `#0D0F13` |
| Panneau | `#16191F` |
| Panneau surélevé / ligne sélectionnée | `#1F232B` |
| Bordure | `rgba(255,255,255,0.06)` |
| Texte | `#F2F4F8` |
| Texte secondaire | `#8B93A1` |
| Accent | `#2F7BFF` |
| Halo de l'accent | `0 0 18px rgba(47,123,255,0.55)` |
| Liseré de la fenêtre | `1px solid rgba(47,123,255,0.35)` |
| Avertissement / erreur | `#F5A524` / `#F25F5C` |
| Rayons | panneaux 20 px · lignes et champs 12 px · pastilles et pilules 999 px |
| Police | **Plus Jakarta Sans** (licence OFL), embarquée dans l'application, jamais chargée depuis internet |
| Carte du mail | fond `#FFFFFF`, rayon 16 px, marge intérieure 24 px |

### Fenêtres

- **Principale :** les trois panneaux.
- **Nouveau mail :** une fenêtre Tauri séparée par mail en cours. Plusieurs peuvent être ouvertes en même temps.
- **Assistant d'ajout de compte :** une fenêtre modale au-dessus de la fenêtre principale. Au premier lancement, sans compte, il s'ouvre automatiquement.

## 7. Lecture des mails et sécurité

1. Le HTML est **nettoyé dans le cœur Rust** (`ammonia`), avant tout affichage. Sont retirés : scripts, gestionnaires d'événements (`on*`), `iframe`, `object`, `embed`, `form`, `base`, `meta http-equiv`, `link` vers des feuilles de style externes, URL `javascript:`.
2. Les images `cid:` sont remplacées par des URL `data:` construites à partir des parties téléchargées.
3. Les images distantes sont réécrites (`src` → `data-blocked-src`) **tant que l'expéditeur n'est pas autorisé**. « Afficher les images » restaure les `src` pour ce mail uniquement. « Toujours pour cet expéditeur » ajoute l'adresse à `image_allowlist`.
4. Le mail est affiché dans une **iframe `srcdoc`** avec `sandbox="allow-same-origin"` (sans `allow-scripts`). Elle porte une CSP : `default-src 'none'; style-src 'unsafe-inline'; img-src data:` (plus `https:` quand les images sont autorisées).
5. **Liens :** l'interface intercepte les clics dans l'iframe, ce que `allow-same-origin` permet. Le lien s'ouvre dans le navigateur par défaut, et l'URL réelle s'affiche au survol.
6. Un mail en texte brut est affiché sur la même carte claire, en conservant les retours à la ligne. Les URL y sont rendues cliquables.
7. **Pièces jointes :**
   - « Ouvrir » écrit le fichier dans `%LOCALAPPDATA%\Illy\tmp\` et le confie à l'application Windows associée ;
   - « Enregistrer sous » ouvre le dialogue de Windows ;
   - les extensions exécutables (`.exe`, `.bat`, `.cmd`, `.ps1`, `.vbs`, `.js`, `.msi`, `.scr`) ne s'ouvrent pas directement : seul « Enregistrer sous » est proposé.

## 8. Rédaction et envoi

- **Répondre / Répondre à tous / Transférer** ouvrent l'éditeur **sous le mail**, dans le panneau de droite :
  - les destinataires et l'objet (« Re: », « Fwd: ») sont préremplis ;
  - le message d'origine est cité en dessous ;
  - un transfert reprend les pièces jointes d'origine, téléchargées si besoin.
- **Nouveau mail** ouvre une fenêtre séparée.
- **Éditeur TipTap** : gras, italique, souligné, listes à puces et numérotées, liens, citation, images collées ou glissées (envoyées en images intégrées `cid:`), pièces jointes par bouton ou glisser-déposer.
- **Adresse d'envoi** (sélecteur « De ») :
  - en réponse : le compte qui a reçu le mail ;
  - pour un nouveau mail : le compte du dossier affiché, ou le premier compte depuis la vue unifiée ;
  - toujours modifiable.
- **Destinataires** : autocomplétion depuis la table `contacts`.
- **Signature** : celle du compte « De ». Elle est insérée au-dessus de la citation si elle n'est pas vide, et remplacée si on change de compte « De ».
- **Brouillons** :
  - sauvegarde locale 2 s après la dernière frappe ;
  - copie dans le dossier Brouillons du serveur (`APPEND`, puis suppression de l'ancienne copie) à la fermeture de l'éditeur et toutes les 60 s pendant la rédaction ;
  - un brouillon fermé sans envoi reste dans Brouillons ; un brouillon envoyé est supprimé.
- **Envoi** :
  - construction MIME : `multipart/alternative` (texte + HTML), `multipart/related` pour les images intégrées, `multipart/mixed` pour les pièces jointes ;
  - en-têtes `In-Reply-To` et `References` en réponse ;
  - SMTP (`lettre`) sur le port 465 (TLS) ou 587 (STARTTLS) ;
  - l'envoi passe par la file `ops` : sans réseau, le mail attend dans un état « En attente d'envoi » visible, et part dès le retour du réseau.
- **Copie dans « Envoyés »** : après l'envoi, le worker vérifie au bout de 15 s si le dossier `sent` contient le `Message-ID`, car Gmail l'y place tout seul. Sinon, il fait un `APPEND` avec `\Seen`.
- **Répondu** : après l'envoi d'une réponse, le mail d'origine reçoit `\Answered`.

## 9. Comptes

### Assistant d'ajout

1. L'utilisateur saisit son nom et son adresse mail.
2. Le profil est détecté d'après le domaine :

| Profil | Domaines | IMAP | SMTP |
|---|---|---|---|
| Gmail | `gmail.com`, `googlemail.com` | `imap.gmail.com:993` (TLS) | `smtp.gmail.com:465` (TLS) |
| Yahoo | `yahoo.*`, `ymail.com` | `imap.mail.yahoo.com:993` (TLS) | `smtp.mail.yahoo.com:465` (TLS) |
| OVH | tout autre domaine, si l'utilisateur choisit « Hébergé chez OVH » | `ssl0.ovh.net:993` (TLS) | `ssl0.ovh.net:465` (TLS) |
| Manuel | tout autre domaine | saisi | saisi |

3. **Gmail et Yahoo** : l'assistant affiche trois étapes courtes (activer la validation en deux étapes, vérifier qu'IMAP est activé, créer un mot de passe d'application) avec un bouton qui ouvre directement la bonne page de sécurité du fournisseur.
4. L'utilisateur saisit le mot de passe. L'assistant **teste IMAP et SMTP** avant d'enregistrer, et affiche une erreur claire si besoin : « Mot de passe refusé », « Serveur injoignable », « IMAP désactivé sur ce compte ».
5. Une fois le compte enregistré, la synchro démarre et la boîte de réception s'affiche dès les premiers mails reçus.

### Réglages d'un compte

Nom affiché, signature, mot de passe, couleur de la pastille, serveurs (modifiables pour un compte manuel ou OVH), suppression du compte (efface les données locales et le mot de passe stocké, ne touche à rien sur le serveur).

## 10. Gestion des erreurs

| Situation | Comportement |
|---|---|
| Pas de réseau | Bandeau discret « Hors ligne » ; lecture locale normale ; actions et envois mis en file ; reconnexion avec backoff de 5 s à 5 min |
| Mot de passe refusé | Le compte passe en `auth_error` ; point orange dans le panneau de gauche ; **le worker cesse de réessayer** jusqu'à la mise à jour du mot de passe, pour ne pas bloquer le compte chez le fournisseur |
| Action refusée par le serveur | Annulation locale ; message « Impossible de déplacer le mail : [raison du serveur] » |
| Échec d'envoi SMTP | Le mail reste « En attente d'envoi » avec la raison ; boutons « Réessayer » et « Modifier » |
| `UIDVALIDITY` changé | Resynchro silencieuse du dossier |
| Mail mal formé | Affichage du texte brut disponible, avec une mention « Ce mail n'a pas pu être affiché correctement » |
| Plantage | Transactions SQLite et WAL : aucune corruption ; la file `ops` reprend au redémarrage |

Les logs (`tracing`, fichiers en rotation) enregistrent les événements de synchro et les erreurs. Ils ne contiennent jamais de mot de passe, de contenu de mail ni d'objet de mail.

## 11. Tests

- **Cœur Rust, tests unitaires :**
  - analyse MIME, sur des fichiers `.eml` de référence **fabriqués pour les tests** (structures typiques de Gmail, Yahoo et OVH, mails mal formés ; aucun vrai mail personnel dans le dépôt) ;
  - nettoyage HTML, sur un jeu d'attaques XSS connues ;
  - détection des rôles de dossiers ;
  - calcul des différences de synchro (fonctions pures : UID connus contre UID serveur) ;
  - file `ops` (reprise, backoff, annulation) ;
  - construction MIME des envois.
- **Cœur Rust, tests d'intégration :** contre un serveur **GreenMail** (IMAP + SMTP, conteneur Docker). Ils couvrent la synchro initiale et incrémentale, les drapeaux, les déplacements, la suppression, l'envoi puis la copie dans Envoyés, et les brouillons.
- **Interface :** Vitest et Testing Library sur les composants et l'état, avec la couche `api/` simulée.
- **Performances :** une base générée de 100 000 mails sert à vérifier les objectifs du § 1 (changement de boîte, recherche, défilement).
- **Tests manuels :** une liste de vérification par jalon, à dérouler sur les vrais comptes Gmail, Yahoo et OVH.
- **Intégration continue (GitHub Actions) :**
  - un job Linux lance les tests Rust, l'intégration GreenMail et les tests de l'interface ;
  - un job **Windows** compile l'application et produit l'installateur (`.msi` et `.exe` NSIS) en artefact téléchargeable.

## 12. Jalons

Chaque jalon a son propre plan d'implémentation et se termine par une version utilisable.

| # | Jalon | Terminé quand… |
|---|---|---|
| 1 | **Fondations, comptes, synchro, lecture** | On ajoute ses comptes Gmail, Yahoo et OVH avec l'assistant. Les dossiers apparaissent, la synchro hybride tourne et les nouveaux mails arrivent via IDLE. On lit un mail HTML sur la carte claire, images bloquées et autorisables, et on ouvre ou enregistre une pièce jointe. Ouvrir un mail le marque lu sur le serveur, via la file `ops` (reprise après coupure réseau incluse). Le style visuel est en place. |
| 2 | **Actions** | Marquer non lu, supprimer, déplacer, spam, créer et renommer des dossiers, sur la file `ops` du jalon 1 ; annulation locale et message en cas de refus du serveur. |
| 3 | **Recherche** | Index FTS5, recherche dans la boîte affichée et « Chercher partout », filtres ; objectif < 300 ms sur 100 000 mails. |
| 4 | **Rédaction** | Réponse sous le mail, fenêtre « Nouveau mail », éditeur, pièces jointes, choix « De », signatures, autocomplétion, brouillons, envoi et copie dans Envoyés. |
| 5 | **Installateur et finitions** | Installateur Windows produit par la CI ; vérification des objectifs de performance ; liste de vérification complète déroulée. Prêt à être donné aux proches. |

## 13. Environnement de développement

- Le cœur Rust et l'interface se développent et se testent sous Linux comme sous Windows.
- **L'application Windows finale est compilée sur Windows** : sur le PC du propriétaire (`npm run tauri build`) ou par le job Windows de la CI.
- Pendant le développement, l'interface peut tourner dans un navigateur avec une couche `api/` simulée, pour travailler le style sans le cœur Rust.
