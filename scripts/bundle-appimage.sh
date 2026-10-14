#!/usr/bin/env bash
# =============================================================================
# bundle-appimage.sh — Package Tequila as an AppImage
# =============================================================================
#
# This script:
#   1. Downloads linuxdeploy + GTK plugin into .tools/ (git-ignored)
#   2. Builds the release binary
#   3. Creates an AppDir with the binary, resources, and runtime deps
#   4. Bundles GTK4/libadwaita via linuxdeploy-plugin-gtk
#   5. Produces a portable .AppImage in dist/
#
# Usage:
#   ./scripts/bundle-appimage.sh           # → dist/tequila-<version>-linux-<arch>.AppImage
#
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$PROJECT_DIR/dist"
TOOLS_DIR="$PROJECT_DIR/.tools"
APP_DIR="$DIST_DIR/Tequila.AppDir"
ARCH="$(uname -m)"
APP_ID="com.github.anson2251.tequila"
VERSION="$(grep -m 1 '^version' "$PROJECT_DIR/crates/tequila/Cargo.toml" | sed 's/[^0-9.]//g')"

if [ -z "$VERSION" ]; then
    echo "ERROR: Could not determine package version from crates/tequila/Cargo.toml" >&2
    exit 1
fi

mkdir -p "$TOOLS_DIR" "$DIST_DIR"

# ---------------------------------------------------------------------------
# Download build tools
# ---------------------------------------------------------------------------
LINUXDEPLOY="$TOOLS_DIR/linuxdeploy-x86_64.AppImage"
GTK_PLUGIN="$TOOLS_DIR/linuxdeploy-plugin-gtk.sh"

if [ ! -f "$LINUXDEPLOY" ]; then
    echo ":: Downloading linuxdeploy..."
    wget -O "$LINUXDEPLOY" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
    chmod +x "$LINUXDEPLOY"
fi

if [ ! -f "$GTK_PLUGIN" ]; then
    echo ":: Downloading linuxdeploy-plugin-gtk..."
    wget -O "$GTK_PLUGIN" \
        "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
    chmod +x "$GTK_PLUGIN"
fi

# ---------------------------------------------------------------------------
# Build release binary
# ---------------------------------------------------------------------------
echo ":: Building release binary..."
cd "$PROJECT_DIR"
cargo build --release

# Clean dist
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# ---------------------------------------------------------------------------
# Create AppDir skeleton
# ---------------------------------------------------------------------------
echo ":: Creating AppDir..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR"/usr/bin
mkdir -p "$APP_DIR"/usr/share/applications
mkdir -p "$APP_DIR"/usr/share/metainfo
mkdir -p "$APP_DIR"/usr/share/tequila/lang
mkdir -p "$APP_DIR"/usr/share/icons/hicolor/512x512/apps
mkdir -p "$APP_DIR"/usr/share/glib-2.0/schemas

cp target/release/tequila "$APP_DIR/usr/bin/"

# Desktop entry
cat > "$APP_DIR/usr/share/applications/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Tequila
Comment=A modern Wine prefix manager
Exec=tequila
Icon=$APP_ID
Categories=System;Emulator;Utility;
Terminal=false
StartupNotify=true
EOF

# Icon (512x512 PNG required by linuxdeploy)
ICON_SRC=""
for candidate in data/icons/tequila.png data/icons/tequila-icon.png; do
    if [ -f "$candidate" ]; then
        ICON_SRC="$candidate"
        break
    fi
done
if [ -z "$ICON_SRC" ]; then
    echo "ERROR: no app icon found. Add a 512x512 PNG at data/icons/tequila.png" >&2
    exit 1
fi
cp "$ICON_SRC" \
   "$APP_DIR/usr/share/icons/hicolor/512x512/apps/$APP_ID.png"

# AppStream metainfo
cat > "$APP_DIR/usr/share/metainfo/$APP_ID.metainfo.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>$APP_ID</id>
  <name>Tequila</name>
  <summary>A modern Wine prefix manager</summary>
  <developer id="com.github.anson2251">
    <name>anson2251</name>
  </developer>
  <project_license>GPL-3.0-or-later</project_license>
  <metadata_license>CC0-1.0</metadata_license>
  <url type="homepage">https://github.com/anson2251/tequila</url>
</component>
EOF

# Language files (i18n.rs finds them via ancestor walk: share/tequila/lang)
cp data/lang/*.json "$APP_DIR/usr/share/tequila/lang/"

# ---------------------------------------------------------------------------
# Bundle GTK4/libadwaita runtime data
# ---------------------------------------------------------------------------

# Adwaita icon theme (symbolic icons for toolbar buttons)
if [ -d /usr/share/icons/Adwaita ]; then
    echo ":: Bundling Adwaita icon theme..."
    mkdir -p "$APP_DIR/usr/share/icons"
    cp -r /usr/share/icons/Adwaita "$APP_DIR/usr/share/icons/"
    rm -rf "$APP_DIR/usr/share/icons/Adwaita/cursors" 2>/dev/null || true
fi

# hicolor icon theme (fallback, always expected by GTK)
if [ -d /usr/share/icons/hicolor ]; then
    mkdir -p "$APP_DIR/usr/share/icons"
    cp -r /usr/share/icons/hicolor "$APP_DIR/usr/share/icons/"
fi

# GLib schemas (including org.gnome.desktop.interface for gtk-theme etc.)
if [ -d /usr/share/glib-2.0/schemas ]; then
    echo ":: Bundling GLib schemas..."
    cp /usr/share/glib-2.0/schemas/*.xml "$APP_DIR/usr/share/glib-2.0/schemas/" 2>/dev/null || true
    cp /usr/share/glib-2.0/schemas/*.gschema.override "$APP_DIR/usr/share/glib-2.0/schemas/" 2>/dev/null || true
fi

# Compile schemas
if [ -d "$APP_DIR/usr/share/glib-2.0/schemas" ]; then
    echo ":: Compiling GLib schemas..."
    glib-compile-schemas "$APP_DIR/usr/share/glib-2.0/schemas"
fi

# ---------------------------------------------------------------------------
# linuxdeploy apprun-hook — libadwaita runtime environment
# ---------------------------------------------------------------------------
# libadwaita's stylesheet is a GResource embedded in libadwaita-1.so, rather
# than a GTK theme under /usr/share/themes.  The GTK plugin writes a hook that
# forces GTK_THEME=Adwaita:* and sets GTK_DATA_PREFIX to the AppDir root; both
# are incorrect for GTK4/libadwaita.  Name this hook "zz-" so it is sourced
# after the plugin hook and can correct those values.
mkdir -p "$APP_DIR/apprun-hooks"
cat > "$APP_DIR/apprun-hooks/zz-tequila-libadwaita.sh" <<'HOOK'
# Sourced by AppRun before launching the binary.
APPDIR="${APPDIR:-"$(dirname "$(readlink -f "$0")")"}"
export GTK_DATA_PREFIX="$APPDIR/usr"
export XDG_DATA_DIRS="$APPDIR/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GSETTINGS_SCHEMA_DIR="$APPDIR/usr/share/glib-2.0/schemas"
# Let libadwaita select its embedded light/dark stylesheet itself.
unset GTK_THEME
HOOK
chmod +x "$APP_DIR/apprun-hooks/zz-tequila-libadwaita.sh"

# ---------------------------------------------------------------------------
# Symlinks for AppImage conventions
# ---------------------------------------------------------------------------
ln -sf usr/share/applications/$APP_ID.desktop "$APP_DIR/"
ln -sf usr/share/icons/hicolor/512x512/apps/$APP_ID.png "$APP_DIR/"

# ---------------------------------------------------------------------------
# Bundle GTK4/libadwaita with linuxdeploy + GTK plugin
# ---------------------------------------------------------------------------
echo ":: Running linuxdeploy..."

export LDAI_OUTPUT="$DIST_DIR/tequila-${VERSION}-linux-${ARCH}.AppImage"
export GTK_DATA_PREFIX="$APP_DIR/usr"

# Explicitly deploy libadwaita: its embedded GResource contains the complete
# libadwaita stylesheet, so no host theme files are required at runtime.
LIBADWAITA_LIBRARY="$(pkg-config --variable=libdir libadwaita-1)/libadwaita-1.so.0"
if [ ! -e "$LIBADWAITA_LIBRARY" ]; then
    echo "ERROR: libadwaita runtime library not found: $LIBADWAITA_LIBRARY" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Validate GDK PixBuf loaders (linuxdeploy-plugin-gtk hard-fails with a
# cryptic `cp: cannot stat 'gdk-pixbuf-2.0/2.10.0'` when the paths recorded
# in gdk-pixbuf-2.0.pc don't exist on the host)
# ---------------------------------------------------------------------------
# Reproduce exactly what the plugin will compute:
#   gdk_pixbuf_binarydir = pkg-config var, default $libdir/gdk-pixbuf-2.0/2.10.0
echo ":: Checking GDK PixBuf loaders..."
GDK_PIXBUF_LIBDIR="$(pkg-config --variable=libdir gdk-pixbuf-2.0 2>/dev/null || true)"
if [ -z "$GDK_PIXBUF_LIBDIR" ]; then
    echo "ERROR: pkg-config cannot find gdk-pixbuf-2.0." >&2
    echo "Install the dev package first, e.g.: sudo apt-get install libgdk-pixbuf-2.0-dev" >&2
    exit 1
fi
PLUGIN_PIXBUF_BINARYDIR="$(pkg-config --variable=gdk_pixbuf_binarydir gdk-pixbuf-2.0 2>/dev/null || true)"
if [ -z "$PLUGIN_PIXBUF_BINARYDIR" ]; then
    PLUGIN_PIXBUF_BINARYDIR="$GDK_PIXBUF_LIBDIR/gdk-pixbuf-2.0/2.10.0"
fi

has_pixbuf_loaders() {
    [ -d "$1" ] && [ -n "$(find "$1" -maxdepth 2 -name 'libpixbufloader-*.so' -print -quit 2>/dev/null)" ]
}

if has_pixbuf_loaders "$PLUGIN_PIXBUF_BINARYDIR"; then
    echo "   GDK PixBuf loaders: $PLUGIN_PIXBUF_BINARYDIR"
else
    echo "   Plugin-expected pixbuf dir is unusable: $PLUGIN_PIXBUF_BINARYDIR"
    # Look for usable loaders: first under the same prefix (version skew in
    # the versioned subdir), then anywhere on the system.
    REAL_MODULEDIR=""
    for d in "$GDK_PIXBUF_LIBDIR"/gdk-pixbuf-2.0/*/loaders; do
        if [ -d "$d" ] && has_pixbuf_loaders "$(dirname "$d")"; then REAL_MODULEDIR="$d"; break; fi
    done
    if [ -z "$REAL_MODULEDIR" ]; then
        REAL_MODULEDIR="$(find /usr/lib /usr/lib64 /usr/local/lib \
            -maxdepth 4 -type d -path '*gdk-pixbuf-2.0/*/loaders' 2>/dev/null | head -1 || true)"
    fi
    if [ -z "$REAL_MODULEDIR" ] && command -v ldconfig &>/dev/null; then
        _png_loader="$(ldconfig -p 2>/dev/null | grep -m1 'libpixbufloader-png\.so' | awk '{print $NF}' || true)"
        if [ -n "$_png_loader" ]; then REAL_MODULEDIR="$(dirname "$_png_loader")"; fi
    fi

    if [ -n "$REAL_MODULEDIR" ] && has_pixbuf_loaders "$(dirname "$REAL_MODULEDIR")"; then
        REAL_BINARYDIR="$(dirname "$REAL_MODULEDIR")"
        echo "   Found usable loaders at $REAL_MODULEDIR"
        echo "   Redirecting the GTK plugin via a shadow pkg-config file."
        # Shadow gdk-pixbuf-2.0.pc so the plugin's pkg-config queries resolve
        # to the real directories. Only the three path variables are rewritten;
        # everything else is byte-identical to the system .pc file.
        PC_PATHS="$(pkg-config --variable=pc_path pkg-config 2>/dev/null || echo /usr/lib/x86_64-linux-gnu/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig)"
        SRC_PC=""
        _old_ifs="$IFS"; IFS=':'
        # shellcheck disable=SC2162
        for _d in $PC_PATHS ${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}; do
            if [ -n "${_d:-}" ] && [ -f "$_d/gdk-pixbuf-2.0.pc" ]; then SRC_PC="$_d/gdk-pixbuf-2.0.pc"; break; fi
        done
        IFS="$_old_ifs"
        if [ -z "$SRC_PC" ]; then
            echo "ERROR: could not locate gdk-pixbuf-2.0.pc to shadow." >&2
            exit 1
        fi
        SHADOW_PC_DIR="$TOOLS_DIR/pkgconfig-shadow"
        mkdir -p "$SHADOW_PC_DIR"
        sed -e "s|^gdk_pixbuf_binarydir=.*|gdk_pixbuf_binarydir=$REAL_BINARYDIR|" \
            -e "s|^gdk_pixbuf_moduledir=.*|gdk_pixbuf_moduledir=$REAL_MODULEDIR|" \
            -e "s|^gdk_pixbuf_cache_file=.*|gdk_pixbuf_cache_file=$REAL_MODULEDIR/loaders.cache|" \
            "$SRC_PC" > "$SHADOW_PC_DIR/gdk-pixbuf-2.0.pc"
        export PKG_CONFIG_PATH="$SHADOW_PC_DIR${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        echo "   Shadow .pc: $SHADOW_PC_DIR/gdk-pixbuf-2.0.pc"
    else
        cat >&2 <<EOF
ERROR: GDK PixBuf image loaders not found on this host.
The GTK bundling plugin needs the loaders directory recorded in gdk-pixbuf-2.0.pc:
  expected: $PLUGIN_PIXBUF_BINARYDIR/loaders
Install the gdk-pixbuf runtime, e.g.:
  Debian/Ubuntu: sudo apt-get install libgdk-pixbuf-2.0-0 libgdk-pixbuf2.0-bin
  Fedora:        sudo dnf install gdk-pixbuf2
  Arch:          sudo pacman -S gdk-pixbuf2
EOF
        exit 1
    fi
fi

# gdk-pixbuf-query-loaders regenerates loaders.cache inside the AppDir; without
# it the plugin falls back to the host cache file (works, but may go stale).
if ! command -v gdk-pixbuf-query-loaders &>/dev/null; then
    echo "   WARNING: gdk-pixbuf-query-loaders not found; loaders.cache will not be regenerated." >&2
    echo "   Install it with: sudo apt-get install libgdk-pixbuf2.0-bin" >&2
fi

# The GTK plugin uses UPDINFO; set it from git tag if available
if git describe --tags --exact-match 2>/dev/null; then
    export UPDINFO="gh-releases-zsync|anson2251|tequila|latest|tequila-${VERSION}-linux-${ARCH}.AppImage.zsync"
fi

# Auto-enable AppImage extraction if FUSE is unavailable (common in CI)
if ! command -v fusermount &>/dev/null && ! command -v fusermount3 &>/dev/null; then
    export APPIMAGE_EXTRACT_AND_RUN=1
fi

"$LINUXDEPLOY" \
    --appdir "$APP_DIR" \
    --library "$LIBADWAITA_LIBRARY" \
    --plugin gtk \
    --output appimage

echo ":: Done! --> $LDAI_OUTPUT"

# Clean up AppDir
rm -rf "$APP_DIR"
