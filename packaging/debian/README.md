# Packaging Debian de simeis

Ce dossier contient **les sources versionnées** du paquet Debian `simeis`, ainsi que
le script qui les assemble en un fichier `.deb` installable. La construction est
automatisée par la CI GitHub Actions à chaque release, mais elle est entièrement
reproductible en local.

## Contenu du dossier

| Fichier | Rôle |
|---|---|
| `control` | Métadonnées du paquet. Contient deux placeholders : `VERSION` et `SHLIBS_DEPENDS`, substitués au build. |
| `postinst` | Script post-installation : crée l'utilisateur système `simeis`, recharge systemd, active et démarre le service. |
| `prerm` | Script pre-removal : arrête et désactive le service. |
| `postrm` | Script post-removal : sur `purge`, supprime l'utilisateur système. |
| `copyright` | Fichier de licence (MIT) au format Debian. |
| `simeis.1` | Page de manuel (source non compressée). Compressée au build. |
| `simeis.service` | Unit systemd. Installée dans `/usr/lib/systemd/system/` (conforme usr-merge). |
| `build-deb.sh` | Script qui assemble le tout en un `.deb`. |

## Comment ça marche

Le binaire, la documentation (`manual.pdf`) et le `changelog.debian` ne sont **pas**
versionnés ici : ils proviennent du tarball `simeis-server_<VERSION>.tar.gz` publié
par la CI upstream sur chaque release GitHub.

`build-deb.sh` prend ce tarball en entrée, en extrait ces trois éléments, les
combine avec les sources de ce dossier, calcule dynamiquement les dépendances
partagées (`dpkg-shlibdeps`), construit le paquet avec `fakeroot dpkg-deb`, puis le
vérifie avec `lintian`.

Le paquet produit :
- s'installe via `sudo apt install ./simeis_<VERSION>_amd64.deb` (ou `dpkg -i`) ;
- crée un utilisateur système `simeis` ;
- installe et démarre un service systemd écoutant sur `0.0.0.0:8080` ;
- dépend de `cmatrix` et `adduser` (plus les libs détectées automatiquement) ;
- se désinstalle proprement avec `dpkg -r simeis` (ou `dpkg --purge simeis`).

## Construire le paquet en local (VM Debian/Ubuntu)

Pré-requis :

```sh
sudo apt-get update
sudo apt-get install -y dpkg-dev fakeroot lintian
```

Récupérer un tarball de release puis lancer le script depuis la **racine du repo** :

```sh
# Exemple avec la release 1.0.0
wget https://github.com/Godlonx/simeis/releases/download/1.0.0/simeis-server_1.0.0.tar.gz

./packaging/debian/build-deb.sh simeis-server_1.0.0.tar.gz 1.0.0
```

Le script affiche chaque étape ; en cas de succès, il indique le chemin et la taille
du paquet.

## Où trouver le `.deb` final

- **En local** : à la racine du repo, sous le nom `simeis_<VERSION>_amd64.deb`
  (par exemple `simeis_1.0.0_amd64.deb`). Il est ignoré par git (`.gitignore`).
- **En CI** : uploadé automatiquement comme asset sur la release GitHub
  correspondante. L'URL publique est affichée à la fin du job `build-deb`.

## Déclencher la CI manuellement

Le workflow `.github/workflows/build-deb.yml` se lance automatiquement à la
publication d'une release. Il peut aussi être déclenché à la main
(*Actions → Build Debian package → Run workflow*) :
- avec un champ `version` (ex. `1.0.0`) pour cibler une release précise ;
- ou sans rien, pour utiliser la dernière release publiée.
