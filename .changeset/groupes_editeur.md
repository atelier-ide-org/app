---
default: minor
---

# Groupes d'éditeurs et fenêtres, Ctrl+clic instantané, et une interface qui ne bloque plus sur le disque

L'éditeur se divise comme dans IntelliJ : à droite ou en bas, autant de fois que voulu, chaque groupe avec ses propres onglets. Un onglet glissé sur un bord d'un groupe le divise, sur son milieu le rejoint, hors de la fenêtre ouvre une fenêtre à lui. Les séparateurs redimensionnent les groupes, dessinés comme des îlots arrondis.

Ctrl+clic va à la déclaration dans l'image même du clic, en reprenant la réponse trouvée pendant que Ctrl était tenu sur le mot ; un fichier qui n'était pas ouvert est attendu un court instant. Avec Ctrl tenu, une adresse sous la souris est soulignée et un clic l'ouvre : dans le navigateur, ou dans l'éditeur pour une adresse `file://`.

**Search Everywhere** liste les fichiers dès son ouverture, trouve les fichiers par leur chemin et les correspondances du texte dans les fichiers du projet, coupe ses lignes avec `…` et montre l'icône du plugin de chaque commande. Ctrl+Shift+F ouvre de nouveau **Find in Files**, et non la recherche dans le fichier.

Dans `package.json`, Ctrl+Espace dans la version d'une dépendance propose ses versions publiées sur npm, la plus récente d'abord. L'arborescence du projet copie, coupe et colle fichiers et dossiers (Ctrl+C, Ctrl+X, Ctrl+V).

Le serveur MCP peut être déclaré une fois **pour tous les projets**, dans la configuration de l'utilisateur (celle de la distribution pour un projet WSL) : plus de `.mcp.json` dans chaque projet. La barre d'état indique WSL devant le chemin d'un projet qui vit dans une distribution.

Sur un disque lent, comme un partage WSL, la fenêtre ne se fige plus : les dates des fichiers, le parcours du projet, la liste des fichiers, les erreurs de syntaxe, les symboles et le texte de Git sont lus en arrière-plan, et la liste des fichiers d'un gros projet est bien plus rapide.

Dans le terminal, Ctrl+C interrompt de nouveau, et un clic droit sur une sélection la copie. Valider une suppression déjà indexée dans Git n'échoue plus.

L'aperçu Markdown ne plante plus sur un bloc HTML suivi d'une lettre accentuée, et dispose ces blocs comme écrits (lignes centrées, tableaux, tailles des images, images du web). Les fichiers `.mts` et `.cts` sont du TypeScript. Les barres de défilement sont fines et translucides.
