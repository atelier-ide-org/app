# Changelog

Every version of Atelier IDE. The entries come from the files of `.changeset/` (see [Versions & releases](https://atelier-ide-org.github.io/docs/contributing/releases)).

## 0.6.0 (2026-11-25)

### Breaking Changes

#### Un éditeur visuel Markdown, JS/TS plus complet, et Rechercher dans les fichiers comme Maj Maj

Un fichier Markdown a un quatrième mode d'affichage, **Éditeur visuel** : le document rendu, dont un bloc cliqué passe en édition à l'endroit du clic, avec une barre d'outils (gras, italique, barré, code, lien, titres, listes, cases à cocher, citation, bloc de code, tableau, ligne) et Ctrl+B, Ctrl+I, Ctrl+K ; Échap le quitte. Dans l'aperçu, les puces et les numéros sont alignés sur le texte, les tableaux ont de la marge dans leurs cellules, un en-tête marqué et des lignes alternées, et les lignes vides en plus sont gardées. L'aperçu ne plante plus sur un long fichier ni sur un fichier modifié par une IA pendant qu'il est ouvert.

En JavaScript et TypeScript, **Trouver les usages** d'un `export default` (ou d'un nom exporté) trouve ses imports dans tout le projet, même sous un autre nom, à travers les ré-exports et les fichiers `index`. Le survol montre le type et la documentation d'un nom importé et d'un export par défaut, et le serveur TypeScript lit le projet entier : la barre d'état indique quand il le charge.

Ctrl+Maj+F ouvre **Rechercher dans les fichiers** dans la fenêtre de **Rechercher partout** : les correspondances du texte arrivent pendant la frappe, surlignées, et Ctrl+Entrée les montre toutes dans la fenêtre Recherche, pour les remplacer. Maj Maj ouvre de nouveau **Rechercher partout** (egui 0.37 compte Maj comme une touche, ce qui l'empêchait). Aller à une déclaration, un usage ou un résultat fait défiler l'éditeur sans animation.

### Features

#### Glisser les fichiers de l'arbre hors de l'IDE

Un fichier ou un dossier de l'arbre du projet, ou toute la sélection, se glisse hors de la fenêtre comme depuis l'Explorateur : sur le bureau, dans un dossier, dans une page web qui reçoit des fichiers ou dans un autre programme. Ce sont des copies : le projet garde ses fichiers. Pour l'instant sous Windows.

#### Les linters et formateurs des plugins, et deux corrections au démarrage

Un plugin natif peut désormais lire les documents des fichiers qu'il annonce (`Info::lints`) et y trouver des problèmes : ils s'affichent dans l'éditeur, la gouttière et **Problèmes** avec ceux du langage, même pour un fichier qu'aucun langage de l'IDE ne prend en charge (Markdown, shell, Dockerfile…). Il peut aussi formater ceux qu'il annonce (`Info::formats`) : **Reformater le code** passe d'abord par lui, et l'IDE garde son propre formateur quand le plugin le laisse. L'interface des plugins passe en 0.5 (`document`, `format`, `diagnostics`, `formatted`), sans changer d'ABI. Chaque problème peut porter ses corrections : Alt+Entrée les propose sur le problème, avec « corriger tous les problèmes » d'un outil quand il en a plusieurs. Une ampoule apparaît dans la gouttière sur la ligne du curseur quand elle a un problème : un clic dessus ouvre ces corrections. Le plugin **Lint** s'en sert pour ESLint, Oxlint, Biome, Stylelint, Ruff, ShellCheck et d'autres, avec Prettier et Oxfmt.

Le bouton **Cloner** de l'écran d'accueil ouvre de nouveau la fenêtre de clonage : hors d'un dépôt Git, le plugin Git se réinitialisait à chaque image et l'effaçait aussitôt. Elle reste aussi ouverte quand on revient dans l'IDE après avoir copié l'adresse ailleurs.

Sous Windows, la fenêtre du démarrage n'a plus de rectangle gris autour de sa carte : la fenêtre est la carte, et c'est le système qui dessine son ombre et ses coins arrondis.

#### Un visualiseur web dans l'éditeur

Le plugin intégré **Visualiseur web** ouvre des pages web dans un onglet de l'éditeur, comme un navigateur : **Web : ouvrir une page** dans la palette, une barre d'adresse (une adresse, un chemin ou des mots à chercher), précédent, suivant, recharger, les outils de développement et l'ouverture dans le navigateur du système. Un lien vers une nouvelle fenêtre ouvre un nouvel onglet. Une adresse tapée dans **Rechercher partout** (Maj Maj) s'ouvre dans le visualiseur, et Maj Maj marche aussi depuis la page. Les fichiers `.url` (raccourcis Internet) s'ouvrent dedans, et un fichier HTML montre sa page à côté de sa source, rechargée à chaque enregistrement. Il utilise le moteur web du système (WebView2 sous Windows, WebKit sous macOS) ; pas encore sous Linux.

Une IA le pilote par le serveur MCP de l'IDE, avec les outils `web_*` : ouvrir une page, lire ses éléments numérotés (`web_snapshot`) et son texte, cliquer, taper, faire défiler, attendre, exécuter du JavaScript, lire la console et en prendre une capture (`web_screenshot`, en image).

## 0.5.0 (2026-11-18)

### Breaking Changes

#### Groupes d'éditeurs et fenêtres, Ctrl+clic instantané, et une interface qui ne bloque plus sur le disque

L'éditeur se divise comme dans IntelliJ : à droite ou en bas, autant de fois que voulu, chaque groupe avec ses propres onglets. Un onglet glissé sur un bord d'un groupe le divise, sur son milieu le rejoint, hors de la fenêtre ouvre une fenêtre à lui. Les séparateurs redimensionnent les groupes, dessinés comme des îlots arrondis.

Ctrl+clic va à la déclaration dans l'image même du clic, en reprenant la réponse trouvée pendant que Ctrl était tenu sur le mot ; un fichier qui n'était pas ouvert est attendu un court instant. Avec Ctrl tenu, une adresse sous la souris est soulignée et un clic l'ouvre : dans le navigateur, ou dans l'éditeur pour une adresse `file://`.

**Search Everywhere** liste les fichiers dès son ouverture, trouve les fichiers par leur chemin et les correspondances du texte dans les fichiers du projet, coupe ses lignes avec `…` et montre l'icône du plugin de chaque commande. Ctrl+Shift+F ouvre de nouveau **Find in Files**, et non la recherche dans le fichier.

Dans `package.json`, Ctrl+Espace dans la version d'une dépendance propose ses versions publiées sur npm, la plus récente d'abord. L'arborescence du projet copie, coupe et colle fichiers et dossiers (Ctrl+C, Ctrl+X, Ctrl+V).

Le serveur MCP peut être déclaré une fois **pour tous les projets**, dans la configuration de l'utilisateur (celle de la distribution pour un projet WSL) : plus de `.mcp.json` dans chaque projet. La barre d'état indique WSL devant le chemin d'un projet qui vit dans une distribution.

Le serveur MCP a des **contrôles de qualité** pour le code TypeScript, JavaScript et Rust : `crapper` (score CRAP de chaque fonction), `dryer` (fonctions dupliquées), `verifier` (fonctions à fusionner dans leur unique appelant) et `mutator` (test par mutation, dans des copies du projet) ; `checker` lance les trois premiers, `cleaner` les quatre. Chacun est aussi une commande de l'agent (`/mcp__atelier__checker`…) qui corrige ce qu'il trouve. Une analyse en cours s'affiche dans la barre d'état, avec un bouton pour la mettre en pause.

Sur un disque lent, comme un partage WSL, la fenêtre ne se fige plus : les dates des fichiers, le parcours du projet, la liste des fichiers, les erreurs de syntaxe, les symboles et le texte de Git sont lus en arrière-plan, et la liste des fichiers d'un gros projet est bien plus rapide.

Dans le terminal, Ctrl+C interrompt de nouveau, et un clic droit sur une sélection la copie. Valider une suppression déjà indexée dans Git n'échoue plus.

L'aperçu Markdown ne plante plus sur un bloc HTML suivi d'une lettre accentuée, et dispose ces blocs comme écrits (lignes centrées, tableaux, tailles des images, images du web). Les fichiers `.mts` et `.cts` sont du TypeScript. Les barres de défilement sont fines et translucides.

Les dépendances passent à egui 0.37 (toujours avec le rendu OpenGL, qu'utilisent les plugins natifs), resvg 0.49, quick-xml 0.42, sysinfo 0.40 et vt100 0.18.

## 0.4.0 (2026-11-04)

### Breaking Changes

#### Le lecteur multimédia devient un plugin, et un onglet Langages

Le lecteur de musique et de vidéos n'est plus intégré : c'est désormais le plugin natif `media-viewer`, à installer depuis **Plugins › Marketplace**, comme `database`, `msoffice-viewer` et `gltf-viewer`. rodio et le téléchargement de ffmpeg sortent des dépendances de l'IDE.

La fenêtre **Plugins** a trois onglets : **Plugins** (intégrés et installés), **Marketplace** (les plugins publiés, avec les fichiers qu'ils ouvrent) et **Langages**. Un langage est automatique par défaut : activé tant que le projet contient un de ses fichiers (un `.rs` pour Rust, un `.py` pour Python…).

L'onglet **Plugins** a un bouton **Tout mettre à jour** quand des plugins installés ont une version plus récente.

Avec Git, les marqueurs des lignes modifiées forment une barre continue sur les lignes voisines, et la souris sur une modification ou une suppression montre ce qu'étaient les lignes. Les sous-modules Git ont une icône et une couleur à eux dans l'arborescence.

Le serveur MCP a une commande `pr` : elle vérifie que tous les commits de la branche suivent les règles du dépôt, fait écrire un titre et une description exacts de ce que fait la branche, sans aucune attribution (« Generated with… », `Co-authored-by`), pousse la branche et ouvre la pull request avec `gh`.

Dans la barre d'état, un clic sur la fin de ligne (LF, CRLF, CR) la change dans tout le fichier, et un clic sur l'encodage rouvre le fichier avec un autre encodage ou le convertit (UTF-8, UTF-8 BOM, UTF-16, Windows-1252, ISO-8859-15, Shift_JIS…). Un fichier qui n'est pas en UTF-8 s'ouvre désormais (encodage détecté par sa marque d'ordre des octets, sinon Windows-1252) et s'enregistre dans son encodage.

La fenêtre de l'installeur a une ombre.

Le `plugin.json` d'un plugin natif liste les extensions des fichiers qu'il prend en charge, et leurs icônes sont dans son dossier `extensions/` (`a+b.svg`). `atelier-cli` (dépôt `api`) crée un plugin (`atelier-cli plugin create`) et produit le fichier à installer (`atelier-cli plugin compile`).

## 0.3.0 (2026-11-03)

### Breaking Changes

#### Plugins natifs v2 : une interface stable, et un onglet Disponibles

Les plugins natifs passent à la version 2 de leur interface (`atelier-plugin-api` 0.3) : l'IDE et un plugin ne partagent plus que des types et des fonctions C. Un plugin dessine avec son propre egui, que l'IDE nourrit et dont il peint les maillages et les textures, avec son apparence, ses polices et ses icônes. Il se compile donc seul, avec n'importe quel compilateur, et se charge dans toutes les versions d'Atelier qui lisent la même version de l'interface : plus besoin de le recompiler à chaque version de l'IDE. Les plugins de la version 1 (Atelier 0.2) ne se chargent plus : installez leur nouvelle version.

**Plugins › Disponibles** liste les plugins publiés que cet Atelier sait charger, avec **Installer**, et **Mettre à jour** quand une version plus récente est sortie.

L'explorateur de bases de données (MongoDB, PostgreSQL, MySQL) et la visionneuse Microsoft Office (Word, Excel, PowerPoint, export PDF) ne sont plus intégrés : ce sont désormais les plugins natifs `database` et `msoffice-viewer`, à installer depuis **Plugins › Disponibles**, comme la visionneuse 3D `gltf-viewer`. Les connexions enregistrées sont gardées.

Chaque notification a un bouton pour copier son message, dans le panneau des notifications et dans ses bulles.

## 0.2.0 (2026-11-03)

### Breaking Changes

#### Plugins natifs, et la visionneuse 3D devient un plugin à installer

Un plugin externe peut apporter une bibliothèque dynamique (`"library"` dans son manifeste, rangé dans `.atelier/plugins/<plugin>/plugin.json`), qui dessine dans l'IDE comme un plugin intégré et ouvre des fichiers dans un onglet de l'éditeur. Elle n'est chargée qu'une fois le plugin activé dans le projet. Atelier refuse une bibliothèque qui n'a pas été compilée exactement comme lui (même compilateur, mêmes crates) au lieu de la charger. Les plugins implémentent le crate `atelier-plugin-api`.

Un plugin natif s'installe depuis son fichier (`<plugin>.atelier-plugin`) : **Plugins › Installer un plugin depuis un fichier** (le bouton +). Il est installé une fois, dans l'IDE, pour tous les projets, et activé ; sa page a un bouton **Désinstaller**.

La visionneuse de modèles 3D (glTF, GLB) n'est plus intégrée : c'est désormais le plugin natif `gltf-viewer`, publié pour chaque version d'Atelier (release `plugins/gltf-viewer/vX.Y.Z` du dépôt des téléchargements). Installez `gltf-viewer.atelier-plugin` pour continuer à ouvrir les modèles en 3D.

### Features

#### Fiches des plugins, partagées avec la documentation

Le nom, le résumé et l'icône de chaque plugin intégré viennent de sa fiche : le front matter de sa page de documentation, dans le dossier `doc` du plugin. La fenêtre Plugins et ses menus montrent donc les mêmes textes et le même logo que le catalogue de la documentation ; les langages y ont désormais leur logo au lieu de l'engrenage.

La documentation a un catalogue des plugins, avec une recherche et un filtre par catégorie, et la page de chaque plugin vit à côté de son code.

#### Plugin YAML / TOML, et coloration des langages par leur plugin

**YAML / TOML.** Un nouveau plugin colore et reformate (Ctrl+Alt+L) les fichiers YAML et TOML en gardant les commentaires, les guillemets et l'ordre des clés ; un reformatage qui changerait les données du fichier est refusé. Le menu contextuel d'un fichier `.yaml` ou `.yml` de l'arbre du projet propose « Convertir en JSON », qui écrit le JSON à côté du fichier et l'ouvre.

**Coloration.** La coloration d'un langage vient de son plugin (JavaScript, Rust, Java, Markdown, YAML / TOML) : un plugin désactivé laisse ses fichiers en texte brut. Les fichiers TOML, que la coloration ne connaissait pas, sont désormais colorés, et le plugin Cargo colore `Cargo.lock`.

#### Plugins activés par projet, dans `.atelier`

Les plugins activés et désactivés sont propres à chaque projet : leur état est écrit dans `.atelier/config.json` (section `plugins`), qui peut se partager avec le projet, et non plus dans les réglages de l'IDE. À l'ouverture d'un projet, chaque plugin qu'il ne mentionne pas encore y est inscrit : Git en `"auto"`, activé tant qu'il existe un `.git` dans le projet (un dossier, ou le fichier d'un worktree ou d'un sous-module) : il s'active de lui-même après un `git init` et se désactive si le dépôt disparaît, jusqu'à ce qu'on le règle à la main dans la fenêtre Plugins ; les autres comme ils l'étaient dans les réglages de l'IDE. Les plugins ne font plus partie des réglages exportés ou synchronisés.

#### Schémas JSON, et celui de package.json

Les fichiers JSON signalent leurs erreurs de syntaxe pendant la frappe ; les commentaires et les virgules finales des réglages d'outils (`tsconfig.json`…) ne sont pas des erreurs.

Un fichier JSON peut avoir un schéma : celui que nomme sa clé `$schema` (un fichier du projet, ou l'adresse d'un schéma qu'apporte un plugin), sinon celui qu'un plugin apporte pour son nom. Ses valeurs sont alors vérifiées, ses clés et leurs valeurs complétées, et une clé est décrite sous la souris. Le plugin Node.js apporte le schéma de `package.json`.

#### Sous-plugins, et GitHub sous Git

**Sous-plugins.** Un plugin peut avoir des sous-plugins, affichés sous lui dans la fenêtre Plugins : un sous-plugin ne fonctionne que quand son parent est activé, et se désactive à part.

**GitHub.** Le premier, sous Git. Sa fenêtre Pull Request montre la pull request de la branche courante, la décision et le commentaire de chaque revue, et les commentaires des revues sur le code, par conversation ; une conversation ouvre son fichier à sa ligne. Il passe par la CLI GitHub (`gh`). Il apporte aussi les schémas des workflows GitHub Actions et de `dependabot.yml`.

**Schémas YAML.** Les schémas valent aussi pour les fichiers YAML : ceux qu'apporte un plugin, ou celui que nomme un commentaire `# yaml-language-server: $schema=…`. Valeurs vérifiées, clés et valeurs complétées, description sous la souris.

### Fixes

#### Les téléchargements passent dans `atelier-ide-org/releases`

Atelier déménage dans l'organisation GitHub `atelier-ide-org`. Le lanceur cherche désormais les mises à jour dans le dépôt public [`atelier-ide-org/releases`](https://github.com/atelier-ide-org/releases), qui ne contient que les fichiers des releases. Il se met à jour de lui-même : rien à faire. La documentation a son propre dépôt, `atelier-ide-org/docs`, et une nouvelle adresse : <https://atelier-ide-org.github.io/docs/>.

## 0.1.4 (2026-10-31)

### Features

#### CSS, SCSS, Sass et Less

Les fichiers `.css`, `.scss`, `.sass` et `.less` sont colorés (sélecteurs, propriétés, variables, mixins, interpolations) et analysés sans serveur : erreurs de syntaxe, structure, complétion des propriétés, des valeurs, des couleurs, des pseudo-classes, des `@règles`, des variables et des mixins, aller à la déclaration et occurrences des variables, mixins, classes et keyframes. Le serveur de langage de VS Code (« CSS : installer » dans Rechercher partout, avec Node.js) ajoute les erreurs complètes, la documentation et le renommage pour CSS, SCSS et Less.

Une pastille s'affiche devant chaque couleur (`#hex`, `rgb()`, `hsl()`, noms). Un clic ouvre un sélecteur de couleur (teinte, opacité, couleur d'avant et d'après, hexadécimal, canaux, format d'écriture) qui modifie la couleur dans le texte ; un Ctrl+Z annule tous les changements faits avec le sélecteur.

Reformater le code (Ctrl+Alt+L) remet une déclaration et un sélecteur par ligne, même pour un fichier minifié. Le clic droit propose aussi **Minifier** (écrit `nom.min.css`) et **Transformer en SCSS, Less ou Sass** : variables, mixins, `@extend` et interpolations suivent, et les règles CSS qui se suivent sont imbriquées. Le fichier est écrit à côté, sans jamais en écraser un.

Ctrl+/ entoure les lignes CSS de `/* */`.

#### Fenêtres centrées, écran de démarrage et plantages expliqués

Une fenêtre s'ouvre toujours au milieu de l'écran où se trouve la souris, à sa taille par défaut ou à ce que l'écran permet. Elle ne revient plus toute petite dans le coin en haut à gauche après avoir été fermée réduite, et rouvre agrandie si la précédente l'était. Son bouton dans la barre des tâches suit la fenêtre quand elle passe sur un autre écran.

Chaque projet ouvert dans une nouvelle fenêtre est un programme à part : fermer la première fenêtre ne ferme plus les autres.

Lancé sans dossier, Atelier ouvre un écran de démarrage avec les projets récents, « Ouvrir… », « Cloner un dépôt… » et **« Nouveau projet… »** (un nom et un emplacement), au lieu d'ouvrir le dossier du programme.

Quand Atelier s'arrête de façon inattendue (erreur, mémoire saturée), la cause est écrite dans `logs/crash.log` du dossier de données et la fenêtre suivante le signale. Les lignes très longues (code minifié) ne sont affichées que sur leurs 10 000 premiers caractères : elles pouvaient prendre des gigaoctets de mémoire. Fermer l'onglet d'un gros fichier libère aussi ce que les plugins gardaient pour lui.

#### Lecteur multimédia : musique et vidéos

Les fichiers MP3, WAV, FLAC, OGG et M4A s'ouvrent dans un lecteur, sans rien à installer. Les vidéos MP4, MKV, AVI, MOV et WebM (et les sons comme Opus) sont lues avec ffmpeg : celui du PATH, ou un build que le lecteur propose de télécharger (Windows et Linux), vérifié par sa somme SHA-256.

Lecture et pause (Espace ou clic sur l'image), déplacement dans le fichier (barre de temps, ← et →), volume et son coupé. Un fichier se met en pause quand son onglet n'est plus affiché.

#### Menu contextuel de l'éditeur et « Ajouter au .gitignore »

Un clic droit dans le texte ouvre un menu : couper, copier, coller, tout sélectionner, étendre la sélection, commenter, dupliquer, reformater le code et, dans du code, les actions contextuelles, aller à la déclaration, trouver les utilisations, la documentation et renommer. Les commandes des plugins pour le fichier s'y ajoutent. Le clic droit garde la sélection quand il tombe dedans.

Dans l'arborescence du projet, le groupe Git du clic droit propose **« Ajouter au .gitignore »** pour un fichier, un dossier ou toute la sélection. Une notification prévient quand git suit déjà le fichier, que le `.gitignore` n'ignore alors pas.

#### Plugin Cargo, et Jupyter séparé de Runtimes

**Cargo.** Une fenêtre Cargo, sur le rail de droite des projets qui ont un `Cargo.toml`, liste les paquets et les workspaces du projet avec leurs cibles (binaires, exemples, tests, benchmarks). Un double-clic exécute une cible ; le menu d'un `Cargo.toml` compile, vérifie, teste, lance Clippy, formate, ouvre la documentation, met à jour les dépendances ou nettoie.

**Jupyter.** Les notebooks ont leur propre plugin, Jupyter, qui se désactive sans toucher aux environnements du projet ; le plugin Runtimes ne s'occupe plus que des Python, JDK et Node.js du projet et de son `.venv`.

#### Python

Sans serveur, Atelier connaît les fichiers Python par lui-même : erreurs de syntaxe, structure (classes, méthodes, fonctions, noms du module), occurrences d'un nom dans sa portée, aller à la déclaration (paramètres, affectations, boucles, imports, `self.x`), complétion des noms du fichier, des membres de `self`, des mots-clés et des fonctions intégrées, et modèles (`main`, `init`, `try`, `prop`…).

pyright (ou basedpyright, pris dans le venv du projet, le `PATH`, ou installé avec « Python : installer » dans Rechercher partout) prend le relais : types, erreurs, documentation, renommage, avec les paquets du `.venv` du projet. Reformater le code passe par ruff, sinon black. Les fichiers `.pyi` et `.pyw` sont aussi du Python.

### Fixes

#### Couleurs du code dans les template strings

En TypeScript, le code entre `${` et `}` d'une template string n'est plus coloré comme la chaîne : il garde les couleurs du code. En JavaScript, les appels de méthode (`liste.join(…)`) prennent la couleur des fonctions, comme en TypeScript.

## 0.1.3 (2026-10-16)

### Features

#### Éditeur, navigation et intelligence du code d'IntelliJ

**Éditeur.** Plusieurs curseurs : Alt+J ajoute l'occurrence suivante de la sélection, Alt+Maj+J retire la dernière, Ctrl+Alt+Maj+J les prend toutes, Alt+Maj+clic ajoute ou retire un curseur. La sélection en colonne se fait en glissant avec Alt, ou après Alt+Maj+Inser. Ctrl+Maj+Haut/Bas déplace l'instruction entière, bloc compris. Ctrl+Alt+T entoure la sélection (accolades, `if`, `try`, guillemets…) et propose vos modèles. Le fichier `templates.json` du dossier de données ajoute des modèles de code à la complétion et à « Entourer de ». L'éditeur peut être partagé en deux (Partager à droite ou en bas, dans le menu des onglets), les lignes qui ouvrent les blocs restent en haut, le fil d'Ariane montre où est le curseur. F11 pose un signet, Maj+F11 les liste, Ctrl+Maj+1…9 en pose un numéroté et Ctrl+1…9 y va. L'historique local garde une copie à chaque enregistrement (« Afficher l'historique local ») et montre en choisissant une version ce que sa restauration changerait, avant de la restaurer. Les fichiers de 4 à 64 Mo s'ouvrent en lecture seule.

**Navigation.** Ctrl+Maj+A cherche une action, Ctrl+Maj+E liste les emplacements récents avec leur ligne, Ctrl+Tab change de fichier, Ctrl+Q affiche la documentation, Ctrl+Maj+I la définition, Ctrl+H et Ctrl+Alt+H les hiérarchies de types et d'appels. Ctrl+P ne sert plus à Rechercher partout : il affiche les paramètres de l'appel.

**Intelligence du code.** Les indications du serveur de langage (types, noms des arguments) s'affichent dans le code. Ctrl+P donne la signature de l'appel, avec le paramètre courant. Ctrl+Alt+Maj+T liste les refactorings du serveur pour la sélection. « Inspecter le code » cherche les erreurs de syntaxe de tous les fichiers du projet et les met dans Problèmes ; la fenêtre Commit prévient quand le code à valider en contient. L'orthographe des commentaires, des chaînes et des fichiers Markdown est vérifiée (anglais et français) : Alt+Entrée propose les mots proches et l'ajout au dictionnaire. Ces réglages sont dans Éditeur › Général.

**Correction.** Rechercher partout (double Maj) ne fige plus l'IDE sur un gros projet : la liste des fichiers se fait en arrière-plan et les recherches sont gardées d'une image à l'autre.

**Modèles.** Les modèles postfixes (`valeur.if`, `.var`, `.for`, `.return`, `.log`… selon le langage) prennent l'expression avant le point. Les modèles de code, postfixes et d'entourage de l'utilisateur se modifient dans Éditeur › Modèles de code. « Entourer de » connaît aussi les blocs de Python. Alt+Entrée, Ctrl+Alt+V/C/N/M : intentions et refactorings de l'IDE (extraire une variable, une constante, une méthode, inliner). « Inspecter le code » ajoute les erreurs des serveurs et de `tsc`.

### Fixes

#### Liens sur plusieurs lignes, icônes, sélection lisible et copie dans le terminal

Un lien qui tient sur plusieurs lignes s'ouvre en entier, qu'il soit coupé par le terminal ou par un programme qui coupe lui-même son texte, comme une IA : il est souligné sur toutes ses lignes.

La sélection est dessinée sous le texte, qui reste lisible.

Les icônes des invites et des outils en ligne de commande s'affichent avec une Nerd Font installée sur la machine, et les symboles manquants avec celle du système (Segoe UI Symbol sous Windows).

Un programme qui copie lui-même du texte (OSC 52), comme une IA en plein écran, le met dans le presse-papiers. Alt et une lettre envoient Échap puis la lettre : Alt+V colle une image dans une IA, Alt+B et Alt+F avancent d'un mot dans le shell.

## 0.1.2 (2026-10-15)

### Features

#### Runtimes du projet et notebooks Jupyter

Le nouveau plugin **Runtimes & Jupyter** installe un Python, des JDK et des Node.js dans le projet (`.atelier/runtimes`), vérifiés par leur SHA-256. Les nouveaux terminaux du projet les ont en tête du `PATH` (avec `JAVA_HOME` et le `.venv` activé). Les versions épinglées par `.python-version`, `.java-version`, `.sdkmanrc`, `.nvmrc` ou `.node-version` sont lues et proposées à l'installation.

Les fichiers `.ipynb` s'ouvrent en notebook : cellules éditées et exécutées sur place (Shift+Entrée) avec le Python du projet, sorties texte, images et Markdown, et ouverture dans JupyterLab.

### Fixes

#### Ctrl+clic vers un autre fichier sans sélection parasite

Un Ctrl+clic qui ouvrait la déclaration dans un autre fichier sélectionnait du texte dans ce fichier si la souris bougeait avant le relâchement du bouton.

#### Renommer (Maj+F6) s'applique à nouveau avec Entrée

Dans le champ de Renommer, Entrée ne faisait rien : le renommage ne s'appliquait jamais.

#### Renommer un nom importé renomme aussi sa déclaration

En JavaScript et TypeScript, Renommer (Maj+F6) sur une fonction importée, depuis un fichier qui l'utilise, ne changeait que ce fichier : l'import ne correspondait plus à rien. Le fichier qui la déclare et les autres fichiers qui l'importent sont maintenant renommés aussi, comme quand on renomme depuis la déclaration.

#### Sélection à la souris, copie et molette dans le terminal

Une sélection à la souris, dans l'éditeur comme dans le terminal, commençait quelques pixels après l'endroit cliqué : le premier caractère manquait, et Ctrl+C copiait parfois une sélection vide dans le terminal. Elle commence maintenant là où le bouton a été enfoncé.

Dans le terminal, la molette fonctionne aussi dans les programmes qui prennent tout l'écran (éditeurs, pagers, IA) : elle leur est envoyée comme des événements de molette s'ils suivent la souris, sinon comme les flèches ↑ et ↓.

## 0.1.1 (2026-10-11)

### Features

#### Installateur Windows, commande atelier, fenêtre de démarrage et nouveau logo

Sous Windows, `atelier-setup-windows-x86_64.exe` installe Atelier sans droits administrateur : raccourci dans le menu Démarrer, désinstalleur, et l'IDE déjà prêt sans téléchargement au premier démarrage. Comme VS Code, il met la commande `atelier` dans le PATH (cmd, PowerShell et Git Bash) : `atelier .` ouvre le dossier courant.

Comme Discord ou VS Code, le lanceur affiche une fenêtre avec le logo pendant qu'il cherche une mise à jour, la télécharge (avec sa progression) et l'installe, puis démarre l'IDE. En cas d'échec, elle explique pourquoi et propose de réessayer.

Aucune fenêtre de console ne s'ouvre plus au démarrage (installation de la commande `atelier`, liste des distributions WSL). Quand aucune release n'est trouvée, le message explique pourquoi (dépôt privé : `ATELIER_GITHUB_TOKEN`, ou pas encore de release) au lieu d'afficher « http status 404 ».

## 0.1.0 (2026-10-11)

### Breaking Changes

#### Première version publiée

Éditeur multilangage à l'interface d'IntelliJ, avec Git, terminal, plugins, serveur MCP et bootstrap de mise à jour pour Windows, Linux et macOS.
