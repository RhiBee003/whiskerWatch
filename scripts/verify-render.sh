#!/usr/bin/env bash
# Check whether whiskerwatch.onrender.com is the old React SPA or the Rust app.
set -euo pipefail

URL="${1:-https://whiskerwatch.onrender.com}"
html="$(curl -sS "$URL")"

echo "URL: $URL"
curl -sSI "$URL" | grep -iE '^(HTTP/|date:|last-modified:|server:|rndr-id:)' || true
echo

if echo "$html" | grep -q 'id="root"'; then
  echo "STATUS: OLD REACT STATIC SITE (<div id=\"root\">)"
  echo "Action: Delete Static Site in Render, apply render.yaml Blueprint (see RENDER_SETUP.md)."
  exit 1
fi

if echo "$html" | grep -qi 'whisker'; then
  echo "STATUS: Looks like the Rust WhiskerWatch site (no React root mount)."
  exit 0
fi

echo "STATUS: Unknown — inspect HTML manually."
echo "$html" | head -20
exit 2
