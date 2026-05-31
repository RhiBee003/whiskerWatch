#!/usr/bin/env bash
# Trigger a Render deploy for whiskerwatch when RENDER_API_KEY and RENDER_SERVICE_ID are set.
set -euo pipefail

if [[ -z "${RENDER_API_KEY:-}" ]]; then
  echo "ERROR: RENDER_API_KEY is not set." >&2
  echo "Create one at https://dashboard.render.com/u/settings#api-keys" >&2
  exit 1
fi

if [[ -z "${RENDER_SERVICE_ID:-}" ]]; then
  echo "ERROR: RENDER_SERVICE_ID is not set." >&2
  echo "Find it on the service Settings page (starts with srv-)." >&2
  exit 1
fi

echo "Triggering deploy for service ${RENDER_SERVICE_ID}..."
response="$(curl -sS -w "\n%{http_code}" -X POST \
  "https://api.render.com/v1/services/${RENDER_SERVICE_ID}/deploys" \
  -H "Authorization: Bearer ${RENDER_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{}')"

body="${response%$'\n'*}"
code="${response##*$'\n'}"

if [[ "$code" != "201" && "$code" != "200" ]]; then
  echo "Deploy request failed (HTTP ${code}):" >&2
  echo "$body" >&2
  exit 1
fi

echo "$body" | python3 -m json.tool 2>/dev/null || echo "$body"
echo "Deploy triggered. Watch progress in the Render dashboard."
