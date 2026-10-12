#!/usr/bin/env bash
set -euo pipefail
REPO="${1:-.}"
exec python3 "$(dirname "$0")/validate-parity.py" "$REPO"
