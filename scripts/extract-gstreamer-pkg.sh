#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 [--force] <pkg_file> <output_dir>

Extract GStreamer runtime from official .pkg installer into a clean Unix-like
directory (bin, lib, share, etc.), suitable for setting GST_PLUGIN_PATH and
DYLD_LIBRARY_PATH.

The script uses 'pkgutil --expand-full' (if available) to completely expand
the package, then copies only relevant directories and adds license & version.

Arguments:
  pkg_file     Path to GStreamer .pkg installer (e.g., gstreamer-1.0-1.28.2-universal.pkg)
  output_dir   Target directory (e.g., ~/local/gstreamer)

Options:
  --force      Allow writing into non-empty output directory
  -h, --help   Show this help
EOF
}

FORCE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --force) FORCE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) break ;;
    esac
done

if [[ $# -ne 2 ]]; then
    echo "Error: Exactly two arguments required." >&2
    usage
    exit 1
fi

PKG_FILE="$1"
OUTPUT_DIR="$2"

# --- Validate output directory ---
if [[ -e "$OUTPUT_DIR" ]]; then
    if [[ ! -d "$OUTPUT_DIR" ]]; then
        echo "Error: Output path '$OUTPUT_DIR' exists but is not a directory." >&2
        exit 1
    fi
    if [[ -n "$(ls -A "$OUTPUT_DIR")" ]]; then
        if [[ $FORCE -eq 0 ]]; then
            echo "Error: Output directory '$OUTPUT_DIR' is not empty. Use --force to override." >&2
            exit 1
        else
            echo "Warning: Output directory is not empty, but --force given. Continuing." >&2
        fi
    fi
else
    mkdir -p "$OUTPUT_DIR"
fi

# --- Check input file ---
if [[ ! -f "$PKG_FILE" ]]; then
    echo "Error: Package file '$PKG_FILE' not found." >&2
    exit 1
fi

# --- Create temporary workspace ---
TMP_DIR=$(mktemp -d "/tmp/gst_extract.XXXXXX")
trap 'rm -r "$TMP_DIR"' EXIT

# --- Use pkgutil to expand ---
echo "Expanding $PKG_FILE ..."
# Prefer --expand-full (recursively expands all sub-packages)
if pkgutil --expand-full "$PKG_FILE" "$TMP_DIR/expanded" 2>/dev/null; then
    echo "Using pkgutil --expand-full (recursive expansion)."
    EXPANDED_ROOT="$TMP_DIR/expanded"
else
    echo "pkgutil --expand-full not supported, falling back to --expand."
    pkgutil --expand "$PKG_FILE" "$TMP_DIR/expanded"
    EXPANDED_ROOT="$TMP_DIR/expanded"
    # Recursively expand any .pkg directories inside (simulate --expand-full)
    echo "Recursively expanding sub-packages..."
    find "$EXPANDED_ROOT" -type d -name "*.pkg" -print0 | while IFS= read -r -d '' subpkg; do
        if [[ -d "$subpkg" && ! -f "$subpkg/.expanded" ]]; then
            tmp_sub="$(mktemp -d)"
            pkgutil --expand "$subpkg" "$tmp_sub"
            rm -r "$subpkg"
            mv "$tmp_sub" "$subpkg"
            touch "$subpkg/.expanded"
        fi
    done
fi

# --- Helper: copy contents from a directory if it exists ---
copy_if_exists() {
    local src="$1"
    local dst="$2"
    if [[ -d "$src" ]]; then
        mkdir -p "$dst"
        cp -R "$src/." "$dst/"
    fi
}

# --- Collect all Payload directories ---
echo "Merging extracted files..."
find "$EXPANDED_ROOT" -type d -name "Payload" -print0 | while IFS= read -r -d '' payload_dir; do
    echo "  Processing $payload_dir"
    cp -R "$payload_dir/." "$OUTPUT_DIR/"
done

# --- Extract version from Distribution file ---
VERSION="unknown"
DISTRIBUTION_FILE="$EXPANDED_ROOT/Distribution"
if [[ -f "$DISTRIBUTION_FILE" ]]; then
    # Try to get CFBundleShortVersionString from the GStreamer bundle
    VERSION=$(sed -n 's/.*CFBundleShortVersionString="\([^"]*\)".*/\1/p' "$DISTRIBUTION_FILE" | head -1)
    if [[ -z "$VERSION" ]]; then
        # Fallback: get version from first pkg-ref version attribute
        VERSION=$(sed -n 's/.*<pkg-ref[^>]* version="\([^"]*\)".*/\1/p' "$DISTRIBUTION_FILE" | head -1)
    fi
fi
echo "$VERSION" > "$OUTPUT_DIR/version.txt"
echo "Version $VERSION written to version.txt"

# --- Copy license file to root ---
LICENSE_SRC=$(find "$EXPANDED_ROOT" -name "license.txt" -type f | head -1)
if [[ -n "$LICENSE_SRC" ]]; then
    cp "$LICENSE_SRC" "$OUTPUT_DIR/LICENSE"
    echo "License copied to $OUTPUT_DIR/LICENSE"
else
    echo "Warning: license.txt not found in expanded package."
fi

# --- Create environment setup script ---
OUTPUT_DIR_ABS="$(cd "$OUTPUT_DIR" 2>/dev/null && pwd)" || OUTPUT_DIR_ABS="$OUTPUT_DIR"
if [[ ! -d "$OUTPUT_DIR_ABS" ]]; then
    echo "Error: Cannot determine absolute path for $OUTPUT_DIR" >&2
    exit 1
fi

cat > "$OUTPUT_DIR_ABS/setup_env.sh" <<EOF
#!/bin/bash
# Source this file to set environment variables for GStreamer
export GST_PLUGIN_SYSTEM_PATH_1_0="$OUTPUT_DIR_ABS/lib/gstreamer-1.0"
export GST_PLUGIN_PATH="$OUTPUT_DIR_ABS/lib/gstreamer-1.0"
export DYLD_LIBRARY_PATH="$OUTPUT_DIR_ABS/lib:\${DYLD_LIBRARY_PATH:-}"
export PATH="$OUTPUT_DIR_ABS/bin:\$PATH"
echo "GStreamer environment set for $OUTPUT_DIR_ABS"
echo "Version: $(cat "$OUTPUT_DIR_ABS/version.txt")"
EOF
chmod +x "$OUTPUT_DIR_ABS/setup_env.sh"
chmod +x "$OUTPUT_DIR_ABS"/bin/gst-*-1.0

# --- Create key=val env file for Tequila to read directly ---
cat > "$OUTPUT_DIR_ABS/env" <<EOF
GST_PLUGIN_SYSTEM_PATH_1_0=$OUTPUT_DIR_ABS/lib/gstreamer-1.0
GST_PLUGIN_PATH=$OUTPUT_DIR_ABS/lib/gstreamer-1.0
DYLD_LIBRARY_PATH=$OUTPUT_DIR_ABS/lib
PATH_PREPEND=$OUTPUT_DIR_ABS/bin
EOF
echo "Environment file written to $OUTPUT_DIR_ABS/env"

# --- Clean up Framework-specific clutter ---
echo "Cleaning up non-standard files/directories..."
cd "$OUTPUT_DIR"
to_remove=(
    "Commands"
    "GStreamer"
    "Headers"
    "Libraries"
    "Resources"
    "Versions"
    # Remove any stray .pkg directories that might have been copied incorrectly
    "*.pkg"
)
for item in "${to_remove[@]}"; do
    # Use globbing to match .pkg directories
    shopt -s nullglob
    for f in $item; do
        rm -r "$f"
    done
    shopt -u nullglob
done
# Remove .DS_Store
find . -name ".DS_Store" -delete 2>/dev/null || true

echo "Done. Output directory: $OUTPUT_DIR"
echo "To use with Wine, source the setup script:"
echo "  source $OUTPUT_DIR/setup_env.sh"
