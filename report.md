# Rapport du projet CI/CD

Projet réalisé par Maxence MAHIEUX, Etienne LEMEE et Rémi LAURENT.

## Somaire

- [Organisation du dépôt](#organisation-du-dépôt)
  - [Stratégie de branches](#stratégie-de-branches)
  - [CODEOWNERS](#codeowners)
  - [Template de Pull Request](#template-de-pull-request)
  - [Template d'issue](#template-dissue)
- [Les différents workflows](#les-différents-workflows)
  - [Intégration sur develop et main](#intégration-sur-develop-et-main)
  - [Qualité et tests transverses](#qualité-et-tests-transverses)
  - [Contrôle qualité continu](#contrôle-qualité-continu)
  - [Protection et analyse des branches de release](#protection-et-analyse-des-branches-de-release)
  - [Publication et maintenance](#publication-et-maintenance)
- [Création d'une release](#création-dune-realease)
- [Difficultés rencontrés](#difficultés-rencontrés)

## Organisation du dépôt

Le dépôt est structuré pour encadrer la collaboration à 3 (Maxence, Etienne et Rémi) et automatiser un maximum de vérifications avant la fusion du code. Cette organisation repose sur quatre piliers : une stratégie de branches, un fichier `CODEOWNERS`, des templates de contribution et un ensemble de workflows d'intégration continue.

### Stratégie de branches

Le projet suit un modèle de type *Git Flow* simplifié, articulé autour de branches à durée de vie longue et de branches de travail éphémères :

| Branche | Rôle |
|---|---|
| `main` | Branche stable et de référence. Elle ne reçoit que du code validé et testé. |
| `develop` | Branche d'intégration où sont regroupées les fonctionnalités en cours avant stabilisation. |
| `release/*` | Branches de préparation d'une version (ex. `release/1.1.0`), sur lesquelles se déroulent les analyses avancées avant publication. |

Les branches de travail suivent une convention de nommage `préfixe/numéro-description`, où le numéro renvoie à l'issue GitHub correspondante :

- `feature/*` (ou `feat/*`) - développement d'une nouvelle fonctionnalité (ex. `feature/3-buy-ship`) ;
- `fix/*` - correction de bug (ex. `fix/package-version`) ;
- `doc/*` - rédaction ou mise à jour de la documentation (ex. `doc/38-creating-the-report-file`) ;
- `test/*` - mise en place ou évolution des tests (ex. `test/51-setting-up-the-functional-test-skeleton`).

Cette convention rend l'historique lisible et permet de relier immédiatement chaque branche à sa potentielle issue et pull request. Elle est par ailleurs **contrôlée automatiquement** : le workflow `CI-Release-PR-Check` rejette toute Pull Request ciblant une branche `release/*` qui ne provient pas de `main` ou d'une branche `fix/*`. On garantit ainsi qu'une release n'intègre que du code stable ou des correctifs ciblés, jamais une fonctionnalité en cours.

### CODEOWNERS

Le fichier `.github/CODEOWNERS` attribue à chaque partie du dépôt un ou plusieurs responsables. Toute Pull Request modifiant un dossier concerné requiert automatiquement la revue de son propriétaire avant fusion :

| Chemin | Propriétaire |
|---|---|
| `/*` (racine) | @Godlonx |
| `/.github/` | @MaxenceMahieux |
| `/doc/` | @EtienneLm |
| `/sdk/` | @EtienneLm |
| `/simeis-data/` | @Godlonx |
| `/simeis-server/` | @MaxenceMahieux |
| `/tests/` | @Godlonx |
| `/example/rust` | @MaxenceMahieux |
| `/example/python` | @Godlonx |

Ce découpage responsabilise chaque membre sur son périmètre (serveur, données, SDK, documentation, CI, tests) tout en assurant qu'aucune modification ne soit fusionnée sans une relecture par une personne compétente sur la zone concernée.

### Template de Pull Request

Le fichier `.github/pull_request_template.md` pré-remplit chaque nouvelle Pull Request afin d'homogénéiser les contributions. Il impose de préciser :

- **le type de changement** (bug fix, nouvelle fonctionnalité, refactoring, documentation, CI/CD, autre) ;
- **une checklist de qualité** à cocher : code testé et relu localement, respect des règles de style, code commenté, documentation à jour, ajout de tests, passage de l'ensemble des tests et exécution de `make check` et `make test` ;
- **une description** du changement et de sa justification, avec un lien vers l'issue résolue.

On a fait le choix de garder une template plutôt simple afin de ne pas créer des process qui serait trop chronophage.

### Template d'issue

Le dépôt fournit un template d'issue structuré au format formulaire GitHub, `.github/ISSUE_TEMPLATE/bug_report.yml`, dédié aux rapports de bug. Il applique automatiquement le label `bug` et le préfixe de titre `[BUG]`, puis guide le rapporteur à travers des champs normalisés :

- une **description** claire de l'incident (obligatoire) ;
- une **URL de reproduction** éventuelle ;
- les **étapes de reproduction** détaillées (obligatoire) ;
- des **captures d'écran** et des **logs** si pertinents ;
- le **comportement attendu** (obligatoire) ;
- un niveau d'**impact** (bloquant, majeur, mineur…) ainsi que le **navigateur** et le **système d'exploitation** concernés.

En standardisant la remontée d'anomalies, ce formulaire garantit que chaque ticket contient les informations nécessaires à sa prise en charge, sans allers-retours inutiles.

## Les différents workflows

Le dépôt compte **12 workflows** GitHub Actions organisés autour de la stratégie de branches : plus une contribution se rapproche d'une release, plus les vérifications sont exigeantes.

### Intégration sur `develop` et `main`

Ces workflows valident chaque Pull Request avant qu'elle ne rejoigne les branches d'intégration, en mode *debug* pour `develop` puis en *release* à l'approche de `main`.

| Workflow | Déclencheur | Rôle |
|---|---|---|
| `CI-Build` (`build.yml`) | PR vers `develop` | Build (`make build`), doc Typst, tests et vérifications de style. |
| `CI-Build-Release` (`build-release.yml`) | PR vers `main` | Idem en mode release (`make build-release`). |
| `CI-Build-Release-Matrix` (`build-release-matrix.yml`) | PR `feature/*` vers `main` | Compile sur une matrice de 3 OS (Linux, macOS, Windows) × 4 versions de Rust, pour garantir la portabilité. |

### Qualité et tests transverses

Ces workflows se déclenchent sur **toute** Pull Request, quelle que soit sa cible.

| Workflow | Rôle |
|---|---|
| `CI-QUALITY-CHECK` (`quality-check.yml`) | Lint et formatage Rust (`make full-check`) et Python (`black --check`). |
| `CI-Code-Coverage` (`code-coverage.yml`) | Mesure la couverture avec `cargo-tarpaulin` et pose le label `not enough tests` sous 50 %. |
| `CI-Property-Based-Tests` (`property-based-tests.yml`) | Tests basés sur les propriétés (`tests/propertybased.py`) sur des entrées aléatoires. |
| `CI-Check-TODOs` (`check-todos.yml`) | Échoue si un commentaire `TODO` n'est pas rattaché à une issue ouverte (format `TODO (#N)`). |

### Protection et analyse des branches de release

| Workflow | Déclencheur | Rôle |
|---|---|---|
| `CI-Release-PR-Check` (`release-source-branch-pr-check.yml`) | PR vers `release/*` | Vérifie que la branche source est `main` ou `fix/*`, échoue sinon. |
| `CI-Advanced-Analysis` (`advanced-analysis.yml`) | *push* sur `release/*` | Analyses lourdes : tests `heavy-testing`, tests fonctionnels (`run_tests.py`), et audit des dépendances (`cargo-audit`, `cargo-udeps`). |

### Publication et maintenance

| Workflow | Déclencheur | Rôle |
|---|---|---|
| `CI-Create-Update-Release` (`create-delete-release.yml`) | Fermeture d'une PR sur `release/*` | Génère le *changelog*, construit binaire et PDF, empaquette en `tar.gz`, crée le tag et publie la release. |
| `Build Debian package` (`build-deb.yml`) | Publication d'une release (ou manuel) | Construit le paquet `.deb` et l'attache à la release. |
| `CI-Update` (`update.yml`) | Planifié (*cron*) | Exécute `make update` pour maintenir les dépendances à jour. |

## Création d'une realease

La publication d'une version est entièrement automatisée et repose sur les branches `release/*`. Le processus se déroule en 4 étapes.

1. **Création de la branche de release.** On crée une branche `release/x.y.z` (par exemple `release/1.1.0`), dont le nom porte le numéro de version.

2. **Ouverture de la Pull Request.** On ouvre une PR vers cette branche depuis `main` ou une branche `fix/*`. Le workflow `CI-Release-PR-Check` vérifie l'origine de la PR et la bloque si elle ne respecte pas cette règle. Chaque *push* déclenche par ailleurs `CI-Advanced-Analysis`, qui lance les analyses lourdes (tests `heavy-testing`, tests fonctionnels et audit des dépendances).

3. **Fusion et publication.** Une fois la PR fusionnée, le workflow `CI-Create-Update-Release` prend le relais automatiquement. Il :
   - génère le *changelog* (aux formats Markdown et Debian) à partir des PR fusionnées, classées par type (fonctionnalités, corrections, autres) ;
   - compile le binaire en mode release et la documentation PDF ;
   - empaquette le tout dans une archive `tar.gz` ;
   - crée le tag Git correspondant à la version et publie la release GitHub avec ces artefacts.

4. **Construction du paquet Debian.** La publication de la release déclenche `Build Debian package`, qui récupère l'archive, construit le paquet `.deb` avec `packaging/debian/build-deb.sh` et l'attache à la release.

Au final, une seule action manuelle (fusionner la PR vers `release/*`) suffit pour obtenir une release complète : binaire, documentation, changelog et paquet Debian, sans intervention supplémentaire.

## Difficultés rencontrés

La principale difficulté du projet a été la partie 2 du TP5 : la création du paquet Debian. C'est une tâche longue qui enchaîne beaucoup d'étapes de configuration (page de manuel, service systemd, scripts d'installation `postinst`/`prerm`/`postrm`, fichier `control`, gestion de l'utilisateur système `simeis`, droits sur le binaire), et la moindre erreur casse la construction du paquet. Il nous est arrivé plusieurs fois de nous tromper dans un chemin, d'oublier d'installer une dépendance ou un fichier, ou de mal configurer le service, ce qui obligeait à recommencer et à retester l'installation sur une VM Debian à chaque correction.

À cette complexité s'ajoutait celle de l'automatisation dans la CI. Il ne suffisait pas que le paquet se construise en local : il fallait que toute la chaîne fonctionne parfaitement pour que la release se crée et que le `.deb` soit généré puis attaché automatiquement. Coordonner les workflows `CI-Create-Update-Release` et `Build Debian package`, s'assurer que les artefacts étaient bien produits, uploadés au bon endroit et récupérables depuis l'URL de la release, a demandé de nombreux essais et ajustements avant d'obtenir un pipeline entièrement fonctionnel.