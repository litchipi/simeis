#!/usr/bin/env bash
#
# build-deb.sh — Assemble le paquet Debian "simeis" à partir du tarball de release.
#
# Reproduit à l'identique le paquet construit à la main pour la 1.0.0 :
#   - binaire dans /usr/bin/simeis (strippé)
#   - manual.pdf + changelog (gz) + copyright dans /usr/share/doc/simeis/
#   - man page gz dans /usr/share/man/man1/
#   - unit systemd dans /usr/lib/systemd/system/ (usr-merge friendly)
#   - control + postinst + prerm + postrm dans DEBIAN/
#   - dépendances calculées dynamiquement via dpkg-shlibdeps
#
# Usage : ./packaging/debian/build-deb.sh <chemin-tarball> <version>
# Exemple : ./packaging/debian/build-deb.sh downloads/simeis-server_1.0.0.tar.gz 1.0.0
#
set -euo pipefail

# --- Emplacements de référence (robustes, indépendants du cwd) ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"   # packaging/debian/
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"                  # racine du repo
SRC_DIR="$SCRIPT_DIR"                                         # sources versionnées du paquet
BUILD_DIR="$REPO_ROOT/build"                                 # arbo de build temporaire
PKG_DIR="$BUILD_DIR/debian"                                  # racine du paquet à construire
SHLIBS_DIR="/tmp/shlibs"                                     # dossier de travail dpkg-shlibdeps

# =====================================================================
# Étape 1 — Parser et valider les arguments
# =====================================================================
echo "==> [1/16] Validation des arguments"
if [ "$#" -ne 2 ]; then
    echo "ERREUR: usage: $0 <chemin-tarball> <version>" >&2
    echo "        exemple: $0 downloads/simeis-server_1.0.0.tar.gz 1.0.0" >&2
    exit 1
fi
TARBALL="$1"
VERSION="$2"

if [ ! -f "$TARBALL" ] || [ ! -r "$TARBALL" ]; then
    echo "ERREUR: tarball introuvable ou illisible: $TARBALL" >&2
    exit 1
fi
# Chemin absolu du tarball (on va changer de dossier plus tard)
TARBALL="$(cd "$(dirname "$TARBALL")" && pwd)/$(basename "$TARBALL")"
echo "    tarball : $TARBALL"
echo "    version : $VERSION"

# =====================================================================
# Étape 2 — Préparer un dossier de build propre
# =====================================================================
echo "==> [2/16] Préparation de l'arbo de build ($BUILD_DIR)"
rm -rf "$BUILD_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/man/man1"
mkdir -p "$PKG_DIR/usr/share/doc/simeis"
mkdir -p "$PKG_DIR/usr/lib/systemd/system"

# =====================================================================
# Étape 3 — Extraire le tarball et vérifier son contenu
# =====================================================================
echo "==> [3/16] Extraction du tarball"
EXTRACT_DIR="$BUILD_DIR/extracted"
mkdir -p "$EXTRACT_DIR"
tar -xzf "$TARBALL" -C "$EXTRACT_DIR"

# Localisation robuste (le tarball peut contenir un sous-dossier)
SRC_BIN="$(find "$EXTRACT_DIR" -type f -name simeis | head -n1)"
SRC_PDF="$(find "$EXTRACT_DIR" -type f -name manual.pdf | head -n1)"
SRC_CHANGELOG="$(find "$EXTRACT_DIR" -type f -name changelog.debian | head -n1)"

if [ -z "$SRC_BIN" ];       then echo "ERREUR: binaire 'simeis' absent du tarball" >&2; exit 1; fi
if [ -z "$SRC_PDF" ];       then echo "ERREUR: 'manual.pdf' absent du tarball" >&2; exit 1; fi
if [ -z "$SRC_CHANGELOG" ]; then echo "ERREUR: 'changelog.debian' absent du tarball" >&2; exit 1; fi
echo "    binaire   : $SRC_BIN"
echo "    manual    : $SRC_PDF"
echo "    changelog : $SRC_CHANGELOG"

# =====================================================================
# Étape 4 — Installer le binaire (755) puis le stripper
# =====================================================================
echo "==> [4/16] Copie + strip du binaire"
DEST_BIN="$PKG_DIR/usr/bin/simeis"
cp "$SRC_BIN" "$DEST_BIN"
chmod 755 "$DEST_BIN"
strip "$DEST_BIN"

# =====================================================================
# Étape 5 — Copier la documentation PDF
# =====================================================================
echo "==> [5/16] Copie de manual.pdf"
cp "$SRC_PDF" "$PKG_DIR/usr/share/doc/simeis/manual.pdf"

# =====================================================================
# Étape 6 — Copier le changelog (déjà au format Debian) puis gzipper
# =====================================================================
echo "==> [6/16] Installation du changelog (gzip -n --best)"
cp "$SRC_CHANGELOG" "$PKG_DIR/usr/share/doc/simeis/changelog"
gzip -n --best "$PKG_DIR/usr/share/doc/simeis/changelog"

# =====================================================================
# Étape 7 — Copier le copyright (non compressé)
# =====================================================================
echo "==> [7/16] Copie du copyright"
cp "$SRC_DIR/copyright" "$PKG_DIR/usr/share/doc/simeis/copyright"

# =====================================================================
# Étape 8 — Installer la man page puis gzipper
# =====================================================================
echo "==> [8/16] Installation de la man page (gzip -n --best)"
cp "$SRC_DIR/simeis.1" "$PKG_DIR/usr/share/man/man1/simeis.1"
gzip -n --best "$PKG_DIR/usr/share/man/man1/simeis.1"

# =====================================================================
# Étape 9 — Installer l'unit systemd (644)
# =====================================================================
echo "==> [9/16] Installation de l'unit systemd"
cp "$SRC_DIR/simeis.service" "$PKG_DIR/usr/lib/systemd/system/simeis.service"
chmod 644 "$PKG_DIR/usr/lib/systemd/system/simeis.service"

# =====================================================================
# Étape 10 — Copier les fichiers de contrôle DEBIAN/
# =====================================================================
echo "==> [10/16] Copie des fichiers de contrôle (control + scripts mainteneur)"
cp "$SRC_DIR/control"  "$PKG_DIR/DEBIAN/control"
cp "$SRC_DIR/postinst" "$PKG_DIR/DEBIAN/postinst"
cp "$SRC_DIR/prerm"    "$PKG_DIR/DEBIAN/prerm"
cp "$SRC_DIR/postrm"   "$PKG_DIR/DEBIAN/postrm"
chmod 755 "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/prerm" "$PKG_DIR/DEBIAN/postrm"
chmod 644 "$PKG_DIR/DEBIAN/control"

# =====================================================================
# Étape 11 — Calculer les dépendances partagées (dpkg-shlibdeps)
# =====================================================================
echo "==> [11/16] Calcul des dépendances via dpkg-shlibdeps"
rm -rf "$SHLIBS_DIR"
mkdir -p "$SHLIBS_DIR/debian"
cat > "$SHLIBS_DIR/debian/control" <<'EOF'
Source: simeis
Package: simeis
Architecture: amd64
EOF
# dpkg-shlibdeps doit tourner depuis un dossier contenant debian/control.
# -O écrit la ligne "shlibs:Depends=..." sur la sortie standard.
SHLIBS_LINE="$(cd "$SHLIBS_DIR" && dpkg-shlibdeps -O "$DEST_BIN" 2>/dev/null)"
# Extraire la partie après le premier signe '='
SHLIBS_DEPENDS="${SHLIBS_LINE#*=}"
if [ -z "$SHLIBS_DEPENDS" ]; then
    echo "ERREUR: dpkg-shlibdeps n'a renvoyé aucune dépendance" >&2
    exit 1
fi
echo "    dépendances détectées : $SHLIBS_DEPENDS"

# =====================================================================
# Étape 12 — Substituer les placeholders dans le control
# =====================================================================
echo "==> [12/16] Substitution des placeholders __VERSION__ et __SHLIBS_DEPENDS__"
# Délimiteur '|' pour éviter tout conflit avec les '/' éventuels.
sed -i "s|__SHLIBS_DEPENDS__|${SHLIBS_DEPENDS}|" "$PKG_DIR/DEBIAN/control"
sed -i "s|__VERSION__|${VERSION}|"               "$PKG_DIR/DEBIAN/control"
echo "    ----- control final -----"
sed 's/^/    /' "$PKG_DIR/DEBIAN/control"
echo "    -------------------------"

# =====================================================================
# Étape 13 — Normaliser les permissions des dossiers (755)
# =====================================================================
echo "==> [13/16] Normalisation des permissions des dossiers (755)"
find "$PKG_DIR" -type d -print0 | xargs -0 chmod 755

# =====================================================================
# Étape 14 — Construire le .deb avec fakeroot
# =====================================================================
echo "==> [14/16] Construction du paquet (fakeroot dpkg-deb --build)"
fakeroot dpkg-deb --build "$PKG_DIR"           # produit $BUILD_DIR/debian.deb
DEB_NAME="simeis_${VERSION}_amd64.deb"
DEB_PATH="$REPO_ROOT/$DEB_NAME"
rm -f "$DEB_PATH"
mv "$BUILD_DIR/debian.deb" "$DEB_PATH"
echo "    paquet : $DEB_PATH"

# =====================================================================
# Étape 15 — Vérifier avec lintian (échec uniquement sur erreurs E:)
# =====================================================================
echo "==> [15/16] Analyse lintian"
set +e
LINTIAN_OUT="$(lintian "$DEB_PATH" 2>&1)"
set -e
echo "$LINTIAN_OUT"
if echo "$LINTIAN_OUT" | grep -q '^E:'; then
    echo "ERREUR: lintian a relevé des erreurs (E:), arrêt du build." >&2
    exit 1
fi
echo "    lintian OK (aucune erreur E:)"

# =====================================================================
# Étape 16 — Récapitulatif final
# =====================================================================
echo "==> [16/16] Récapitulatif"
echo "--------------------------------------------------------------"
echo "  Paquet : $DEB_PATH"
echo "  Taille : $(du -h "$DEB_PATH" | cut -f1)"
echo "  Sortie lintian :"
if [ -z "$LINTIAN_OUT" ]; then
    echo "    (aucun message — paquet parfaitement propre)"
else
    echo "$LINTIAN_OUT" | sed 's/^/    /'
fi
echo "--------------------------------------------------------------"
echo "OK — build terminé avec succès."
