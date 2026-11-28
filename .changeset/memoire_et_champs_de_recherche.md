---
default: patch
---

# Moins de mémoire, et le texte des champs de recherche à côté de la loupe

L'historique d'annulation d'un fichier est désormais borné (10 000 étapes, 32 Mo de texte) : chaque reformatage ou collage gardait une copie du texte remplacé, et un fichier modifié toute une journée faisait grossir la mémoire sans fin. Les blocs de code de l'aperçu Markdown et des notebooks Jupyter partagent la même coloration syntaxique, au lieu de la recharger pour chaque notebook ou à chaque fermeture d'un fichier Markdown. Les polices de secours du système (Nerd Font, symboles) ne sont plus copiées à chaque changement de police.

Le texte tapé dans les champs de recherche ne passe plus sous la loupe à gauche, et le message de commit Git retrouve sa marge.
