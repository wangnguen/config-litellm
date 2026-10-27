#!/usr/bin/env bash
# ccl-core holds the logic and no terminal: the ccl binary is one shell over
# it, and any other front end could be another. Fail when a terminal or
# argument-parsing crate reaches it, directly or through another dependency.
#
# Run from anywhere; exits non-zero naming every offender.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

found=$(cargo tree -p ccl-core -e normal --prefix none | grep -E '^(anyhow|clap|crossterm) ' | sort -u || true)
if [ -n "$found" ]; then
  echo "error: ccl-core must not depend on terminal crates, but it pulls in:"
  echo "$found"
  exit 1
fi
echo "ccl-core dependencies ok"
