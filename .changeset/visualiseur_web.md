---
default: minor
---

# Un visualiseur web dans l'éditeur

Le plugin intégré **Visualiseur web** ouvre des pages web dans un onglet de l'éditeur, comme un navigateur : **Web : ouvrir une page** dans la palette, une barre d'adresse (une adresse, un chemin ou des mots à chercher), précédent, suivant, recharger, les outils de développement et l'ouverture dans le navigateur du système. Un lien vers une nouvelle fenêtre ouvre un nouvel onglet. Les fichiers `.url` (raccourcis Internet) s'ouvrent dedans, et un fichier HTML montre sa page à côté de sa source, rechargée à chaque enregistrement. Il utilise le moteur web du système (WebView2 sous Windows, WebKit sous macOS) ; pas encore sous Linux.
