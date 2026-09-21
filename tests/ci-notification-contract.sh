#!/usr/bin/env bash
# The homelab CI-notification contract pins the webhook name, the Cleytin
# mention and the allowed_mentions shape in this workflow. Keep it passing.
set -euo pipefail

file="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/.github/workflows/ci.yml"

grep -qs 'DISCORD_CI_WEBHOOK' "$file" ||
  { echo "missing CI webhook" >&2; exit 1; }
grep -Fqs 'CLEYTIN_USER_ID: "1532710143517659356"' "$file" ||
  { echo "missing Cleytin user ID" >&2; exit 1; }
grep -Fqs 'content:("<@"+$cleytin+">")' "$file" ||
  { echo "CI does not mention Cleytin" >&2; exit 1; }
grep -Fqs 'allowed_mentions:{users:[$cleytin]}' "$file" ||
  { echo "CI does not allow the Cleytin mention" >&2; exit 1; }

echo "ci notification contract OK"
