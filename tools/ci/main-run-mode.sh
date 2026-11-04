#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Which run is this, and which jobs are due (ci.yml, job `changes`; decision 0358, Option 4).
#
# A scheduled run is full and due everywhere. A push to `main` is light only when the pushed
# commit's git tree is the tree that a full, green pull request run already tested (the
# `tested-tree` artifact of that run). Every failure, ambiguity or missing piece means full.
# Inputs are environment variables (the path-filter outputs and the repository); the result goes
# to $GITHUB_OUTPUT. `tools/ci/main-run-mode.test.sh` runs this script against a fake `gh`.
set -uo pipefail

: "${EVENT:?}" "${REPO:?}" "${SHA:?}" "${GITHUB_OUTPUT:?}"
tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"

# Succeeds only when every condition holds; prints why when it does not.
light_ok() {
  local prs n head tree run conc art tested
  prs=$(gh api "repos/$REPO/commits/$SHA/pulls" \
    --jq "[.[] | select(.merged_at != null and .merge_commit_sha == \"$SHA\" and .base.ref == \"main\")]") || return 1
  n=$(jq 'length' <<<"$prs") || return 1
  [ "$n" = 1 ] || { echo "pull requests merged as this commit: $n, not 1"; return 1; }
  head=$(jq -r '.[0].head.sha' <<<"$prs") || return 1
  [ -n "$head" ] && [ "$head" != null ] || { echo "no head sha"; return 1; }
  tree=$(gh api "repos/$REPO/git/commits/$SHA" --jq .tree.sha) || return 1
  [ -n "$tree" ] && [ "$tree" != null ] || { echo "no tree"; return 1; }
  run=$(gh api "repos/$REPO/actions/workflows/ci.yml/runs?event=pull_request&head_sha=$head&per_page=10" \
    --jq '[.workflow_runs[] | select(.status == "completed")] | sort_by(.created_at) | reverse | .[0] | "\(.id) \(.conclusion)"') || return 1
  conc=${run#* }
  run=${run%% *}
  [ "$conc" = success ] || { echo "newest completed run $run of the pull request head: $conc"; return 1; }
  art=$(gh api "repos/$REPO/actions/runs/$run/artifacts" \
    --jq '[.artifacts[] | select(.name == "tested-tree" and (.expired | not))] | .[0].id') || return 1
  { [ -n "$art" ] && [ "$art" != null ]; } || { echo "no tested-tree artifact on run $run"; return 1; }
  tested=$(gh api "repos/$REPO/actions/artifacts/$art/zip" > "$tmp/tested-tree.zip" \
    && unzip -p "$tmp/tested-tree.zip" tested-tree.txt) || return 1
  echo "pushed tree $tree; tested tree and flags:"
  echo "$tested"
  grep -qx "tree=$tree" <<<"$tested" || { echo "the pushed tree is not the tested tree"; return 1; }
  grep -qx "full=true" <<<"$tested" || { echo "the pull request run was not a full run"; return 1; }
}

mode=full
if [ "$EVENT" = push ] && [ "${F_RUST:-}" = true ]; then
  if light_ok; then mode=light; fi
fi
echo "mode: $mode"

if [ "$EVENT" = schedule ]; then
  rust=true step=true gate3=true fuzz=false records=true any=true image=false
else
  rust=${F_RUST:-} step=${F_STEP:-} gate3=${F_GATE3:-} fuzz=${F_FUZZ:-}
  records=${F_RECORDS:-} any=${F_ANY:-} image=${F_IMAGE:-}
fi
others=$rust
libs=$rust

# The build and test layout (decision 0516, option C): one build job per system makes a nextest
# archive, and the test shares run from it. Windows first in every list. A pull request (or a
# merge-queue entry) builds on macOS, which compiles every target, but runs no macOS test shares
# and no macOS Each library alone: those run on every push to `main`, light or full, and on
# scheduled and manual runs (owner's choice of option C, 4 Nov 2026).
WIN_SHARDS=8 UBU_SHARDS=6 MAC_SHARDS=2
pre=false
if [ "$EVENT" = pull_request ] || [ "$EVENT" = merge_group ]; then pre=true; fi
# list N: the share numbers 1..N as a JSON array ("[0]" for none, so a matrix is never empty).
list() { if [ "$1" -gt 0 ]; then printf '[%s]' "$(seq -s, 1 "$1")"; else printf '[0]'; fi; }
if [ "$mode" = light ]; then
  # The merged tree was tested on Windows and Ubuntu; macOS was only built, so it runs here.
  win="" ubu=$UBU_SHARDS mac=$MAC_SHARDS
  libs_matrix='{"include":[{"os":"macos-latest","share":1,"shares":1}]}'
  others=false step=false gate3=false
elif [ "$pre" = true ]; then
  win=$WIN_SHARDS ubu=$UBU_SHARDS mac=0
  libs_matrix='{"include":[{"os":"windows-latest","share":1,"shares":2},{"os":"windows-latest","share":2,"shares":2},{"os":"ubuntu-latest","share":1,"shares":2},{"os":"ubuntu-latest","share":2,"shares":2}]}'
else
  win=$WIN_SHARDS ubu=$UBU_SHARDS mac=$MAC_SHARDS
  libs_matrix='{"include":[{"os":"windows-latest","share":1,"shares":2},{"os":"windows-latest","share":2,"shares":2},{"os":"ubuntu-latest","share":1,"shares":2},{"os":"ubuntu-latest","share":2,"shares":2},{"os":"macos-latest","share":1,"shares":1}]}'
fi
win_list="" ubu_list="" mac_list=""
[ -n "$win" ] && win_list=$(list "$win")
[ -n "$ubu" ] && ubu_list=$(list "$ubu")
[ -n "$mac" ] && mac_list=$(list "$mac")
{
  echo "mode=$mode"
  echo "win_shards=$win"
  echo "win_list=$win_list"
  echo "ubu_shards=$ubu"
  echo "ubu_list=$ubu_list"
  echo "mac_shards=$mac"
  echo "mac_list=$mac_list"
  echo "libs_matrix=$libs_matrix"
  echo "libs=$libs"
  echo "rust=$rust"
  echo "others=$others"
  echo "records=$records"
  echo "any=$any"
  echo "fuzz=$fuzz"
  echo "step=$step"
  echo "gate3=$gate3"
  echo "linuxcnc_image=$image"
} >> "$GITHUB_OUTPUT"
