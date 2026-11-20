---
name: Visualiseur web
summary: Les pages web dans l'éditeur, comme dans un navigateur, et la page d'un fichier HTML à côté de sa source.
---

# Visualiseur web

**Web : ouvrir une page** (dans la palette) ouvre un onglet de navigateur : une barre d'adresse, les boutons précédent, suivant et recharger, les outils de développement, et un bouton qui ouvre la page dans le navigateur du système. Dans la barre, une adresse sans `https://` suffit (`localhost:3000` passe en HTTP), un chemin ouvre le fichier, et des mots lancent une recherche. Un lien qui ouvre une nouvelle fenêtre ouvre un nouvel onglet. Une adresse tapée dans **Rechercher partout** (Maj Maj) propose de l'ouvrir dans le visualiseur, et Maj Maj marche aussi depuis la page.

Une page est un fichier `.url`, un raccourci Internet comme Windows en fait : ceux du projet s'ouvrent aussi dans le visualiseur. Une page ouverte depuis la palette rouvre là où elle en était au démarrage suivant, et disparaît avec son onglet.

Un fichier HTML montre sa page à côté de sa source avec le bouton d'aperçu en haut à droite de l'éditeur, ou **Web : page du fichier HTML à côté de sa source**. Elle se recharge à chaque enregistrement. **Ouvrir dans le visualiseur web**, dans le menu de l'arbre, l'ouvre dans un onglet à elle.

Le visualiseur utilise le moteur web du système (WebView2 sous Windows, WebKit sous macOS), avec ses propres cookies et données, gardés par l'IDE. Il n'est pas encore disponible sous Linux. Une page s'affiche dans la fenêtre principale seulement, et se cache le temps qu'un menu ou une fenêtre flottante passe par-dessus.
