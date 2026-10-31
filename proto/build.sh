#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds both prototypes (lane DOCSITE, throwaway). MDBOOK and ZOLA name the binaries.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
fonts="$repo/apps/openchuck-ui/assets/fonts"

# mdBook: the shared stylesheet and the application's own Plex files go into the theme at build time.
mkdir -p "$here/mdbook/theme/fonts"
cp "$fonts"/IBMPlex*.ttf "$here/mdbook/theme/fonts/"
: > "$here/mdbook/theme/fonts/fonts.css"
cp "$here/shared/style.css" "$here/mdbook/src/style.css"
cp "$here/versions.html.in" "$here/mdbook/src/versions.html"
start=$(date +%s.%N)
"${MDBOOK:-mdbook}" build "$here/mdbook" -d "$here/out/mdbook"
python3 -I "$here/shared/fixnav.py" "$here/out/mdbook"
echo "mdbook: $(echo "$(date +%s.%N) - $start" | bc) s"

if command -v "${ZOLA:-zola}" >/dev/null 2>&1; then
  mkdir -p "$here/zola/static/fonts"
  cp "$fonts"/IBMPlex*.ttf "$here/zola/static/fonts/"
  cp "$here/shared/style.css" "$here/zola/static/style.css"
  cp "$here/versions.html.in" "$here/zola/static/versions.html"
  cp "$here/mdbook/src/tutorial/images/06-workholding.png" "$here/zola/content/tutorial/05-hold-the-work/"
  cp "$here/out/mdbook"/elasticlunr-*.min.js "$here/zola/static/elasticlunr.min.js"
  cp "$here/zola/static-search.js" "$here/zola/static/search.js"
  start=$(date +%s.%N)
  (cd "$here/zola" && "${ZOLA:-zola}" build -o "$here/out/zola" --force)
  echo "zola: $(echo "$(date +%s.%N) - $start" | bc) s"
else
  echo "zola not found: Zola prototype not built"
fi
