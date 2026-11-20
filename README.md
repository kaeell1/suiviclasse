# SuiviClasse (en Rust)

[![CI](https://github.com/kaeell1/suiviclasse/actions/workflows/ci.yml/badge.svg)](https://github.com/kaeell1/suiviclasse/actions/workflows/ci.yml)

Recréation en Rust de [SuiviClasse](https://mdubois.forge.apps.education.fr/suiviclasse/) :
application de suivi de classe pour enseignants — plan de classe interactif (photos élèves
Pronote), gestion des points (+/−), notes pense-bête par élève, saisie de séance et
configuration d'emploi du temps (EDT).

L'ensemble des données est stocké localement, dans un unique fichier JSON
(`data/suiviclasse.json` par défaut). L'interface web (HTML/CSS/JS) est embarquée dans le
binaire : aucun serveur ni base de données à installer.

## Démarrer

```bash
cargo run --release
# puis ouvrir http://localhost:8000
```

Options :
- `cargo run --release /chemin/vers/suiviclasse.json` : chemin du fichier de données ;
- `PORT=9000 cargo run --release` : port d'écoute (8000 par défaut).

## Utilisation

1. **EDT** → « Importer un fichier JSON » avec [`example/edt_exemple.json`](example/edt_exemple.json)
   (les classes des créneaux sont créées automatiquement). Les créneaux peuvent aussi être
   saisis à la main.
2. **Import / Export** → importer les fichiers JSON d'élèves exportés depuis Pronote
   (format `{ classe, students: [{ prenom, nom, photo }] }`). L'import met la classe à jour
   sans perdre les données de suivi déjà saisies. Création de classe manuelle possible.
3. **Plan de classe** → choisir une disposition prédéfinie (standard, 18 tables, U, îlots,
   salle de sciences, devoir surveillé), puis placer les élèves par glisser-déposer ou
   clic sur l'élève puis sur un siège. Le plan est enregistré automatiquement.
4. **Saisie d'une séance** → choisir le jour/créneau de l'EDT : la classe et le plan
   s'affichent avec le numéro de semaine (A/B selon la parité, configurable).
5. Un **clic** sur un élève = une action (+1 point / −1 point / action personnalisée avec
   un nombre de points libre). Un **double-clic** = la note pense-bête de l'élève.
6. **Suivi des élèves** → synthèse des points par élève, historique des actions
   (suppression d'une action possible) et notes.
7. **Import / Export** → sauvegarde complète au format JSON, réimportable telle quelle,
   et configuration des listes d'actions positives/négatives.

## Structure

```
src/
  main.rs   — serveur axum et API JSON
  model.rs  — types de données (classes, élèves, plans, actions, notes, EDT)
  store.rs  — stockage JSON avec écriture atomique
static/     — interface web embarquée (index.html, app.js, style.css)
example/    — fichiers d'exemple (EDT)
data/       — créé à l'exécution : données de l'application
```

## API

| Méthode | Route | Rôle |
|---|---|---|
| GET/PUT | `/api/config` | listes d'actions, lettre des semaines paires |
| GET/POST | `/api/classes` | lister / créer-remplacer une classe |
| PUT/DELETE | `/api/classes/:id` | modifier / supprimer |
| POST | `/api/classes/import-pronote` | import Pronote (mise à jour sans perte) |
| GET/PUT/DELETE | `/api/plans/:classe_id` | plan de classe |
| GET/POST | `/api/events/:classe_id` | actions (points) |
| DELETE | `/api/events/event/:id` | supprimer une action |
| GET/PUT | `/api/notes/:classe_id` | notes pense-bête |
| GET/PUT | `/api/edt` | emploi du temps |
| POST | `/api/edt/import` | import d'un fichier EDT |
| GET | `/api/export` | sauvegarde complète (téléchargement) |
| POST | `/api/import` | restauration d'une sauvegarde |

## Tests et intégration continue

```bash
cargo test
```

12 tests : unitaires (modèle, stockage, parsing) et d'intégration sur le routeur
(import Pronote sans perte, plan, events triés, notes, import EDT, export/import).

La CI GitHub ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) vérifie à chaque
push : syntaxe du JavaScript embarqué (`node --check`), build release verrouillé
(`Cargo.lock`) et `cargo test --locked`.

Basé sur le travail de Julien DURAND, Arnaud DURAND et Sébastien HENRIO.
