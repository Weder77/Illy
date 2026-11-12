# Illy 0.3.0 — liste de vérification des Paramètres et des mises à jour (sur Windows)

Coche chaque ligne. Si quelque chose ne va pas, note le numéro et ce que tu as vu.

## Installation de la 0.3.0 (une seule fois à la main)

1. [ ] Désinstalle l'ancienne Illy (Paramètres Windows → Applications), puis installe **`Illy_0.3.0_x64-setup.exe`** depuis la release GitHub **v0.3.0**. Tes comptes et tes mails sont toujours là.

## Paramètres

2. [ ] **Ouvrir / fermer** : la roue dentée ouvre « Paramètres » ; Échap ou ✕ le ferme.
3. [ ] **Ordre des comptes** : dans Comptes, glisse un compte au-dessus d'un autre. Le panneau de gauche suit le nouvel ordre, et il est gardé au redémarrage.
4. [ ] **Nom et couleur** : change le nom affiché d'un compte puis envoie-toi un mail : le nouveau nom apparaît comme expéditeur. Change sa couleur : la pastille change dans la liste « Toutes les boîtes ».
5. [ ] **Signature** : écris une signature (avec du gras) pour un compte. Un nouveau mail et une réponse depuis ce compte la contiennent. Dans un nouveau mail, changer « De » pour un autre compte remplace la signature.
6. [ ] **Serveurs OVH** : sur le compte OVH, mets une faute dans le serveur IMAP puis « Tester et enregistrer » : un message d'erreur, rien n'est changé et le compte continue de marcher. Corrige : « Serveurs enregistrés ». Sur Gmail, les serveurs ne sont pas modifiables.
7. [ ] **Ajouter un compte** depuis Comptes → « Ajouter un compte ».
8. [ ] **Apparence** : Clair, Sombre et Automatique (change le thème de Windows pendant qu'Illy est ouvert : Illy suit). Une couleur d'accent, les trois tailles de texte, la densité compacte, et l'extrait masqué. Tout change tout de suite, aussi dans une fenêtre « Nouveau mail » ouverte, et reste après redémarrage.
9. [ ] **Images** : « Toujours afficher » montre les images des newsletters sans bandeau. « Bloquées » les bloque à nouveau. Dans la liste des expéditeurs autorisés, « Retirer » fonctionne.
10. [ ] **Délai d'annulation** : passe à 30 s, envoie un mail : le bandeau compte à partir de 30.
11. [ ] **À propos** : la version est 0.3.0 ; « Ouvrir le dossier des journaux » ouvre l'Explorateur ; l'espace utilisé s'affiche.
12. [ ] **Vider le cache local** : pendant un envoi (dans les 30 s), c'est refusé avec « Attends la fin des envois en cours ». Ensuite, ça fonctionne : les mails reviennent, les comptes, réglages et brouillons sont gardés.
13. [ ] **Supprimer un compte** : la confirmation dit « Supprimer … d'Illy ? Tes mails restent sur le serveur ; seules les données de ce PC sont effacées. ». Le compte disparaît ; ses mails sont toujours dans le webmail. (Rajoute-le ensuite.)

## Mise à jour automatique (une vraie)

14. [ ] Je publie la **v0.3.1**. Au démarrage suivant (10 s après), la 0.3.0 affiche « Illy 0.3.1 est disponible · Installer · Plus tard ». « Plus tard » le masque.
15. [ ] Relance Illy, puis « Installer » : la progression s'affiche, l'installation se fait sans question, puis Illy redémarre en 0.3.1 (Paramètres → À propos).
16. [ ] **Paramètres → Mises à jour → Rechercher des mises à jour** : « Illy est à jour. »

## Où trouver les journaux en cas de problème

`%LOCALAPPDATA%\Illy\logs\` (ou Paramètres → À propos → « Ouvrir le dossier des journaux »).
