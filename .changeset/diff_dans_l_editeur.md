---
default: patch
---

# Les diffs s'ouvrent dans l'éditeur, comme un fichier

Le diff d'une modification ou d'un commit ne s'ouvre plus dans une fenêtre à part, mais dans un onglet de l'éditeur, comme un fichier. **Côte à côte**, la version d'avant à gauche et celle d'après à droite sont deux vrais éditeurs en lecture seule, avec la coloration du langage et leurs vrais numéros de ligne : on peut y sélectionner et copier, et ils défilent ensemble, ligne en face de ligne. Entre les deux, comme dans IntelliJ, chaque changement de gauche est relié à sa place à droite par une bande de sa couleur, et une ligne marque dans l'autre éditeur l'endroit d'un ajout ou d'une suppression. Le diff s'ouvre sur son premier changement. En **unifié**, un seul éditeur montre les lignes supprimées au-dessus de celles qui les remplacent, avec les numéros de ligne d'avant et d'après. Les lignes ajoutées, supprimées et modifiées ont le fond de leur changement ; F7 et Maj+F7 vont d'une différence à l'autre, et la vue unifiée peut masquer les lignes identiques. Rouvrir le même diff revient à son onglet.
