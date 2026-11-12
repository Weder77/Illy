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

### 2. Mettre la clé privée dans GitHub

Copie la clé privée dans le presse-papiers :

```powershell
Get-Content "$env:USERPROFILE\.tauri\illy.key" -Raw | Set-Clipboard
```

Sur GitHub : dépôt **Weder77/Illy** → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**. Crée deux secrets :

| Nom | Valeur |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | colle le presse-papiers (tout le contenu de `illy.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | le mot de passe choisi à l'étape 1 |

Toujours dans **Settings** → **Actions** → **General** → **Workflow permissions** : coche **Read and write permissions**, puis **Save**. C'est ce qui permet à la CI de créer les releases.

### 3. Me donner la clé publique

```powershell
Get-Content "$env:USERPROFILE\.tauri\illy.key.pub" -Raw | Set-Clipboard
```

Colle-la dans la conversation. Elle est publique, il n'y a aucun risque : elle sert seulement à vérifier les signatures, et elle sera inscrite dans `src-tauri/tauri.conf.json`.

## Publier une nouvelle version

1. Choisis le numéro (par exemple `0.3.1`) et mets-le à jour partout :
   ```powershell
   node scripts/bump-version.mjs 0.3.1
   ```
2. Commite, crée le tag et pousse :
   ```powershell
   git commit -am "version 0.3.1"
   git tag v0.3.1
   git push --follow-tags
   ```
3. La CI (workflow **Release**) compile, signe et publie la release GitHub, avec les installateurs et `latest.json`. Compte environ 10 minutes.
4. Au prochain démarrage, Illy affiche « Illy 0.3.1 est disponible · Installer ».
