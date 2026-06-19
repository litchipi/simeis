# Rapport Global

Membres du groupe : 
- Maxence MAHIEUX
- Rémi LAURENT
- Etienne LEMÉE

# Retours personnels par thème

## Système de build

1. 
2. Nous avons rencontré des difficultés pour intégrer les RUSTFLAGS dans notre configuration. Notre première approche consistait à les ajouter via le fichier `.cargo/config.toml` avec l'option `--cfg`, mais cette méthode s'est avérée inefficace. Nous avons finalement opté pour une définition directe dans le Makefile.
3.
4. Lors de la mise en place du workflow CI, nous avons omis d'installer la dépendance `typst` dans le fichier `.github/workflows/build.yml`. Le job `compile-doc` échouait systématiquement avec l'erreur "typst: No such file or directory". La solution a été d'ajouter l'action `typst-community/setup-typst@v4` pour installer l'outil avant son utilisation.
5.

## DIY Dependabot

1. Le workflow cyclique planifié avec `schedule` et une expression cron ne se déclenchait pas comme prévu. Malgré une configuration pour une exécution toutes les minutes, GitHub Actions ne garantit pas une exécution ponctuelle des workflows schedulés en raison de la charge de la plateforme.
2. Cette problématique a impacté directement la vérification automatique des dépendances.
3. La notification des mises à jour disponibles était également affectée par ce dysfonctionnement.

## Organisation CI

1. La découverte du fichier `CODEOWNERS` a été une révélation. Ce mécanisme permet d'assigner automatiquement des reviewers aux pull requests en fonction des fichiers modifiés, facilitant ainsi la séparation des responsabilités au sein de l'équipe.
2.
3. Les templates de pull requests et d'issues étaient déjà connus de l'équipe. Ils restent néanmoins très utiles pour standardiser la structure des contributions et garantir que les informations essentielles sont systématiquement fournies.
4.