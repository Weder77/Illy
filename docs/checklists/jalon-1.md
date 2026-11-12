# Illy — liste de vérification du jalon 1 (sur Windows, avec tes vrais comptes)

Coche chaque ligne. Si quelque chose ne va pas, note le numéro et ce que tu as vu : c'est exactement ce dont j'ai besoin pour corriger.

## Avant de commencer

- Télécharge l'installateur : GitHub → dépôt **Illy** → onglet **Actions** → dernier passage vert de **CI** sur la branche `claude/sweet-tesla-peflkh` → en bas, artefact **illy-windows** (fichier zip contenant le `.msi` et le `.exe`).
- Comme l'application n'est pas signée, Windows SmartScreen peut afficher « Windows a protégé votre ordinateur ». Clique sur **Informations complémentaires**, puis **Exécuter quand même**.
- Prépare tes mots de passe d'application :
  - Gmail : https://myaccount.google.com/apppasswords (la validation en deux étapes doit être active) ;
  - Yahoo : https://login.yahoo.com/account/security → « Générer un mot de passe d'application ».

## Vérifications

1. [ ] **Installation, premier lancement** : l'assistant « Bienvenue dans Illy » s'ouvre tout seul, sans bouton Annuler.
2. [ ] **Ajout des comptes** : Gmail (mot de passe d'application), Yahoo (mot de passe d'application), OVH (choisir « Hébergé chez OVH »). Après « Tester et ajouter », la boîte de réception de chaque compte se remplit en moins de 10 s.
3. [ ] **Dossiers Gmail** : pas de « Tous les messages », « Important » ni « Suivis ». Tes libellés apparaissent comme des dossiers. Les accents des noms de dossiers sont corrects.
4. [ ] **Nouveau mail** : envoie-toi un mail depuis ton téléphone. Il apparaît dans Illy en moins de 10 s, sans rien toucher.
5. [ ] **Lu, sans archivage** : ouvre un mail non lu. Il passe en lu dans Illy **et** dans le webmail, et il reste à sa place (rien d'archivé, rien de déplacé).
6. [ ] **Images** : une newsletter affiche le bandeau « Images bloquées ». « Afficher les images » les montre. « Toujours pour cet expéditeur » les montre aussi après fermeture et réouverture d'Illy.
7. [ ] **Liens** : un clic sur un lien d'un mail l'ouvre dans ton navigateur habituel.
8. [ ] **Pièces jointes** : un PDF s'ouvre d'un clic. Le bouton ⤓ (« Enregistrer sous… ») fonctionne. Un `.exe` ne peut pas être ouvert directement (cadenas), seulement enregistré.
9. [ ] **Mot de passe révoqué** : révoque le mot de passe d'application Gmail. Au prochain redémarrage d'Illy, le compte affiche le triangle orange « Mot de passe refusé ». « Mettre à jour » avec un nouveau mot de passe le remet en route.
10. [ ] **Hors connexion** : coupe le Wi-Fi. Les mails déjà téléchargés restent lisibles. Ouvre un mail non lu, puis remets le Wi-Fi : il passe en lu dans le webmail en moins d'une minute.
11. [ ] **Mémoire** : dans le Gestionnaire des tâches, Illy (avec ses processus « WebView2 ») reste sous ~200 Mo au repos, une fois la synchro initiale terminée.
12. [ ] **Démarrage** : quand les mails sont déjà synchronisés, la boîte de réception s'affiche en moins de 2 s.

## Où trouver les journaux en cas de problème

`%LOCALAPPDATA%\Illy\logs\` (colle ce chemin dans l'Explorateur Windows). Les journaux ne contiennent ni mot de passe, ni contenu, ni objet de mail.
