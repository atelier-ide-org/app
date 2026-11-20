---
default: minor
---

# Les linters et formateurs des plugins, et deux corrections au démarrage

Un plugin natif peut désormais lire les documents des fichiers qu'il annonce (`Info::lints`) et y trouver des problèmes : ils s'affichent dans l'éditeur, la gouttière et **Problèmes** avec ceux du langage, même pour un fichier qu'aucun langage de l'IDE ne prend en charge (Markdown, shell, Dockerfile…). Il peut aussi formater ceux qu'il annonce (`Info::formats`) : **Reformater le code** passe d'abord par lui, et l'IDE garde son propre formateur quand le plugin le laisse. L'interface des plugins passe en 0.5 (`document`, `format`, `diagnostics`, `formatted`), sans changer d'ABI. Le plugin **Lint** s'en sert pour ESLint, Oxlint, Biome, Stylelint, Ruff, ShellCheck et d'autres, avec Prettier et Oxfmt.

Le bouton **Cloner** de l'écran d'accueil ouvre de nouveau la fenêtre de clonage : hors d'un dépôt Git, le plugin Git se réinitialisait à chaque image et l'effaçait aussitôt. Elle reste aussi ouverte quand on revient dans l'IDE après avoir copié l'adresse ailleurs.

Sous Windows, la fenêtre du démarrage n'a plus de rectangle gris autour de sa carte : la fenêtre est la carte, et c'est le système qui dessine son ombre et ses coins arrondis.
