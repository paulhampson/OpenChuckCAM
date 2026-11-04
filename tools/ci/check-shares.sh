#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Checks the test shares of one system before the archive is handed to the test jobs (ci.yml, job
# `build`; decision 0516, option C): each of the N shares (`--partition count:K/N`) holds at least
# one test, and together they hold every test of the full list exactly once, so no test goes unrun
# and none runs twice. Run from the workspace after the tests are built.
# Usage: tools/ci/check-shares.sh N
set -euo pipefail
n=${1:?number of shares}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
list() { cargo nextest list --workspace --locked --message-format oneline "$@"; }
list > "$work/all"
sort "$work/all" > "$work/all.sorted"
: > "$work/shares"
for k in $(seq 1 "$n"); do
  list --partition "count:$k/$n" > "$work/share"
  c=$(wc -l < "$work/share")
  echo "share $k of $n: $c tests"
  if [ "$c" -eq 0 ]; then
    echo "::error title=Empty test share::share $k of $n holds no test"
    exit 1
  fi
  cat "$work/share" >> "$work/shares"
done
sort "$work/shares" > "$work/shares.sorted"
total=$(wc -l < "$work/all.sorted")
echo "full list: $total tests"
if ! cmp -s "$work/all.sorted" "$work/shares.sorted"; then
  echo "::error title=Test shares do not match the full list::$(diff "$work/all.sorted" "$work/shares.sorted" | head -20 | tr '\n' ' ')"
  exit 1
fi
echo "Every test is in exactly one of the $n shares."
