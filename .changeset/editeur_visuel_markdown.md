---
default: major
---

# Un éditeur visuel Markdown, JS/TS plus complet, et Rechercher dans les fichiers comme Maj Maj

Un fichier Markdown a un quatrième mode d'affichage, **Éditeur visuel** : le document rendu, dont un bloc cliqué passe en édition à l'endroit du clic, avec une barre d'outils (gras, italique, barré, code, lien, titres, listes, cases à cocher, citation, bloc de code, tableau, ligne) et Ctrl+B, Ctrl+I, Ctrl+K ; Échap le quitte. Dans l'aperçu, les puces et les numéros sont alignés sur le texte, les tableaux ont de la marge dans leurs cellules, un en-tête marqué et des lignes alternées, et les lignes vides en plus sont gardées. L'aperçu ne plante plus sur un long fichier ni sur un fichier modifié par une IA pendant qu'il est ouvert.

En JavaScript et TypeScript, **Trouver les usages** d'un `export default` (ou d'un nom exporté) trouve ses imports dans tout le projet, même sous un autre nom, à travers les ré-exports et les fichiers `index`. Le survol montre le type et la documentation d'un nom importé et d'un export par défaut, et le serveur TypeScript lit le projet entier : la barre d'état indique quand il le charge.

Ctrl+Maj+F ouvre **Rechercher dans les fichiers** dans la fenêtre de **Rechercher partout** : les correspondances du texte arrivent pendant la frappe, surlignées, et Ctrl+Entrée les montre toutes dans la fenêtre Recherche, pour les remplacer. Maj Maj ouvre de nouveau **Rechercher partout** (egui 0.37 compte Maj comme une touche, ce qui l'empêchait). Aller à une déclaration, un usage ou un résultat fait défiler l'éditeur sans animation.
