# Illy 0.2.0 — liste de vérification de la rédaction (sur Windows, avec tes vrais comptes)

Coche chaque ligne. Si quelque chose ne va pas, note le numéro et ce que tu as vu.

## Avant de commencer

- Installe la version 0.2.0 : GitHub → dépôt **Illy** → onglet **Actions** → dernier passage vert de **CI** sur la branche `claude/sweet-tesla-peflkh` → artefact **illy-windows**. L'installation remplace la 0.1.0 et garde tes comptes.
- Garde le webmail de chaque compte ouvert à côté (Gmail, Yahoo, OVH) pour vérifier ce qui arrive sur le serveur.
- Prépare un fichier de plus de 25 Mo (une vidéo, par exemple) et un fichier au nom accentué (par exemple `Facture été 2026.pdf`).

## Vérifications

1. [ ] **Nouveau mail** : « Nouveau mail » ouvre une fenêtre à part. « De » propose le compte de la boîte affichée. Écris à une autre de tes adresses, avec un objet et du texte en gras, puis **Envoyer**. Le mail arrive, avec le gras.
2. [ ] **Répondre** : sur un mail reçu, « Répondre » ouvre l'éditeur sous le mail. À = l'expéditeur, Objet = « Re: … », la citation « Le …, … a écrit : » est en dessous. Envoie : la réponse arrive dans la même conversation chez le destinataire, et le mail d'origine apparaît comme « répondu » dans le webmail.
3. [ ] **Répondre à tous** : sur un mail envoyé à plusieurs personnes dont toi, « Répondre à tous » met les autres en Cc, **sans toi**.
4. [ ] **Transférer avec pièce jointe** : « Transférer » sur un mail avec un PDF. Le PDF est dans les pièces jointes du transfert, et il arrive chez le destinataire.
5. [ ] **Annuler dans les 10 s** : envoie un mail puis clique tout de suite « Annuler » dans le bandeau du bas. Rien ne part, et l'éditeur se rouvre avec ton texte.
6. [ ] **Copie dans Envoyés** : après un envoi, le mail est dans le dossier Envoyés du webmail **une seule fois** (Gmail la range lui-même : pas de doublon).
7. [ ] **Brouillon dans le webmail** : commence un mail, attends une minute ou ferme la fenêtre. Le brouillon apparaît dans le dossier Brouillons du webmail. Dans Illy, ce brouillon (dossier Brouillons) propose « Modifier ce brouillon ».
8. [ ] **Reprendre une réponse** : commence une réponse sous un mail, clique sur un autre mail, puis reviens : « Reprendre la réponse » rouvre ton texte.
9. [ ] **Envoi hors ligne** : coupe le Wi-Fi, envoie un mail. Après le délai, la barre de titre affiche « 1 envoi en attente ». Remets le Wi-Fi : le mail part tout seul, une seule fois.
10. [ ] **Mot de passe refusé à l'envoi** : révoque le mot de passe d'application d'un compte, puis envoie un mail depuis ce compte. Le bandeau affiche « Échec de l'envoi à … » avec « Réessayer » et « Modifier », et le compte passe en « Mot de passe refusé ».
11. [ ] **Pièce jointe de plus de 25 Mo** : joins le gros fichier. « Envoyer » demande « Les pièces jointes font … Mo. Gmail et la plupart des serveurs refusent au-delà de 25 Mo. Envoyer quand même ? ». « Annuler » n'envoie rien.
12. [ ] **Clic droit** : un clic droit sur un mail de la liste affiche Répondre, Répondre à tous, Transférer. Les flèches et Entrée fonctionnent, Échap ferme le menu.
13. [ ] **Accents** : un objet accentué (« Réunion d'équipe — été »), un nom accentué et la pièce jointe `Facture été 2026.pdf` arrivent intacts chez le destinataire.
14. [ ] **Images collées** : copie une capture d'écran (Impr. écran) et colle-la dans le texte avec Ctrl+V. Elle s'affiche dans le mail reçu.
15. [ ] **Glisser-déposer** : dépose un fichier depuis l'Explorateur sur l'éditeur : il devient une pièce jointe.
16. [ ] **Une seule copie du brouillon** : écris un mail pendant 3 ou 4 minutes sans l'envoyer. Dans le webmail, le dossier Brouillons n'en contient **qu'une** copie (pas une par minute).
17. [ ] **Refus du serveur** : écris à une adresse qui n'existe pas sur ton domaine (par exemple `zzz-inexistant@` ton domaine OVH). Le bandeau affiche « Échec de l'envoi à … : Refusé par le serveur d'envoi : … » avec la raison du serveur, et le compte **n'est pas** marqué « Mot de passe refusé ».
18. [ ] **Fermer Illy avec une fenêtre Nouveau mail ouverte** : la fenêtre se ferme aussi, et le brouillon se retrouve dans le webmail.
19. [ ] **Fermer Illy pendant l'annulation** : envoie un mail et ferme Illy tout de suite (avant la fin des 10 s). Relance Illy : le mail part, **une seule fois**.

## Où trouver les journaux en cas de problème

`%LOCALAPPDATA%\Illy\logs\` (un fichier par jour). Les journaux ne contiennent ni le contenu ni l'objet de tes mails.
