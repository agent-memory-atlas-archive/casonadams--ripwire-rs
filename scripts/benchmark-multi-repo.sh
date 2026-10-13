#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-..}"
exec python3 "$(dirname "$0")/benchmark-multi-repo.py" "$TARGET"
