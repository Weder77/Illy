# Publier une version d'Illy

## Configuration unique : la clé de signature des mises à jour

Illy n'installe une mise à jour que si elle est signée avec **ta** clé privée. Elle ne doit jamais être partagée ni mise dans le dépôt : elle va uniquement dans les secrets GitHub.

> Ce n'est pas une clé openssl : Tauri utilise des clés au format minisign, générées par son outil.

### 1. Générer la clé (sur ton PC, dans PowerShell)

Il faut Node.js. S'il n'est pas installé : `winget install OpenJS.NodeJS.LTS`, puis ferme et rouvre PowerShell.

```powershell
npx @tauri-apps/cli@2 signer generate -w "$env:USERPROFILE\.tauri\illy.key"
```

- Il te demande un **mot de passe** : choisis-en un et note-le, il sert à l'étape 2.
- Deux fichiers sont créés dans `C:\Users\<toi>\.tauri\` :
  - `illy.key` : la clé **privée**, secrète ;
  - `illy.key.pub` : la clé **publique**, que tu peux partager.
- **Sauvegarde `illy.key` et son mot de passe** (gestionnaire de mots de passe, clé USB…). Si tu la perds, les versions suivantes ne pourront plus se mettre à jour toutes seules : il faudra réinstaller Illy à la main une fois, avec une nouvelle clé.

### 2. Mettre la clé privée dans GitHub (fait)

Les deux secrets sont dans l'**environnement** `PRD` du dépôt (**Settings** → **Environments** → **PRD**), avec les règles de déploiement « branche `main` » et « tags `v*` » :

| Nom | Valeur |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | tout le contenu de `illy.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | le mot de passe choisi à l'étape 1 |

Seul le workflow **Release** (`.github/workflows/release.yml`) déclare `environment: PRD` : la CI des branches ne voit jamais la clé. Dans **Settings** → **Actions** → **General** → **Workflow permissions**, **Read and write permissions** doit être coché (création des releases).

### 3. Me donner la clé publique (fait)

```powershell
Get-Content "$env:USERPROFILE\.tauri\illy.key.pub" -Raw | Set-Clipboard
```

Colle-la dans la conversation. Elle est publique, il n'y a aucun risque : elle sert seulement à vérifier les signatures. Elle est inscrite dans `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

## Première installation avec les mises à jour (une seule fois)

La première version qui contient le module de mise à jour (0.3.0) s'installe à la main :

1. Désinstalle l'ancienne version d'Illy (Paramètres Windows → Applications). Tes comptes, mails et réglages restent dans `%LOCALAPPDATA%\Illy\` et sont retrouvés.
2. Sur la page de la release GitHub, télécharge et lance **`Illy_0.3.0_x64-setup.exe`** (l'installateur `.exe`, pas le `.msi` : c'est lui que les mises à jour utilisent, sans demande d'administrateur).

Les versions suivantes se proposent toutes seules.

## Publier une nouvelle version

1. Choisis le numéro (par exemple `0.3.1`) et mets-le à jour partout :
   ```powershell
   node scripts/bump-version.mjs 0.3.1
   ```
2. Commite et pousse, puis publie, au choix :
   - **depuis GitHub** : onglet **Actions** → **Publier une version** → **Run workflow** (sur la branche voulue). Il crée le tag `v0.3.1` et lance **Release** ;
   - **depuis ton PC**, avec le tag :
   ```powershell
   git commit -am "version 0.3.1"
   git tag v0.3.1
   git push
   git push origin v0.3.1
   ```
3. La CI (workflow **Release**) vérifie que le tag correspond à la version, compile, signe et publie la release GitHub, avec les installateurs et `latest.json`. Compte environ 15 minutes.
4. Au prochain démarrage (10 s après), ou par **Paramètres → Mises à jour → Rechercher**, Illy affiche « Illy 0.3.1 est disponible · Installer · Plus tard ». Rien ne s'installe sans ton clic.

Le tag peut être posé sur la branche de travail : la règle « tags `v*` » de l'environnement `PRD` l'autorise.
