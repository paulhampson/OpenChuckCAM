#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Runs main-run-mode.sh against a fake `gh` and checks the mode in each case (decision 0358,
# Option 4): light only when every condition holds, full for everything else, including errors.
# Usage: tools/ci/main-run-mode.test.sh   (needs bash, jq and zip)
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"

# The fake `gh api <path> [--jq filter]`: answers from variables, applies the filter with jq.
cat > "$work/bin/gh" <<'F'
#!/usr/bin/env bash
path=$2; shift 2
filter=.
while [ $# -gt 0 ]; do [ "$1" = --jq ] && filter=$2; shift; done
[ "${FAKE_FAIL:-}" = "$path" ] && exit 1
case "$path" in
  */commits/*/pulls) out=$FAKE_PRS ;;
  */git/commits/*) out="{\"tree\":{\"sha\":\"$FAKE_TREE\"}}" ;;
  */workflows/ci.yml/runs*) out=$FAKE_RUNS ;;
  */actions/runs/*/artifacts) out=$FAKE_ARTIFACTS ;;
  */actions/artifacts/*/zip) cat "$FAKE_ZIP"; exit 0 ;;
  *) exit 1 ;;
esac
jq -r "$filter" <<<"$out"
F
chmod +x "$work/bin/gh"

sha=aaaa000000000000000000000000000000000001
head=bbbb000000000000000000000000000000000002
good_prs="[{\"merged_at\":\"2026-10-30T00:00:00Z\",\"merge_commit_sha\":\"$sha\",\"base\":{\"ref\":\"main\"},\"head\":{\"sha\":\"$head\"}}]"
good_runs='{"workflow_runs":[{"id":7,"status":"completed","conclusion":"success","created_at":"2026-10-30T01:00:00Z"}]}'
good_arts='{"artifacts":[{"id":9,"name":"tested-tree","expired":false}]}'
mkzip() { (cd "$work" && printf 'tree=%s\nfull=%s\n' "$1" "$2" > tested-tree.txt && rm -f "$3" && zip -q "$3" tested-tree.txt); }
mkzip tree1 true "$work/good.zip"
mkzip tree1 false "$work/notfull.zip"
mkzip tree2 true "$work/other.zip"

fail=0
# run NAME EXPECTED-MODE [VAR=value ...]
run() {
  local name=$1 want=$2 got out="$work/out"; shift 2
  : > "$out"
  (
    export EVENT=push REPO=o/r SHA=$sha F_RUST=true F_STEP=true F_GATE3=true F_FUZZ=false F_RECORDS=true F_ANY=true F_IMAGE=false
    export GITHUB_OUTPUT=$out RUNNER_TEMP=$work PATH="$work/bin:$PATH"
    export FAKE_PRS=$good_prs FAKE_TREE=tree1 FAKE_RUNS=$good_runs FAKE_ARTIFACTS=$good_arts FAKE_ZIP=$work/good.zip
    for kv in "$@"; do export "$kv"; done
    bash "$here/main-run-mode.sh" > "$work/log" 2>&1
  )
  got=$(sed -n 's/^mode=//p' "$out")
  if [ "$got" = "$want" ]; then echo "ok   $name -> $got"; else echo "FAIL $name: wanted $want, got '$got'"; cat "$work/log"; fail=1; fi
}

run "tree equal, full green run"            light
run "tree differs (merge result untested)"  full  FAKE_TREE=tree9
run "artifact says not a full run"          full  FAKE_ZIP="$work/notfull.zip"
run "artifact holds another tree"           full  FAKE_ZIP="$work/other.zip"
run "no artifact"                           full  'FAKE_ARTIFACTS={"artifacts":[]}'
run "artifact expired"                      full  'FAKE_ARTIFACTS={"artifacts":[{"id":9,"name":"tested-tree","expired":true}]}'
run "newest run failed"                     full  'FAKE_RUNS={"workflow_runs":[{"id":7,"status":"completed","conclusion":"failure","created_at":"2026-10-30T01:00:00Z"}]}'
run "newest run cancelled"                  full  'FAKE_RUNS={"workflow_runs":[{"id":7,"status":"completed","conclusion":"cancelled","created_at":"2026-10-30T01:00:00Z"}]}'
run "no run for the head"                   full  'FAKE_RUNS={"workflow_runs":[]}'
run "direct push, no pull request"          full  'FAKE_PRS=[]'
run "two pull requests"                     full  "FAKE_PRS=[${good_prs:1:-1},${good_prs:1:-1}]"
run "pull request merged as another commit" full  "FAKE_PRS=${good_prs//$sha/cccc}"
run "API error on the pull request list"    full  "FAKE_FAIL=repos/o/r/commits/$sha/pulls"
run "API error on the tree"                 full  "FAKE_FAIL=repos/o/r/git/commits/$sha"
run "API error on the artifact download"    full  FAKE_ZIP=/nonexistent
run "scheduled run is full"                 full  EVENT=schedule
run "docs-only push (code jobs not due)"    full  F_RUST=false

# Outputs of a light run and of a scheduled run.
out="$work/out"; : > "$out"
( export EVENT=push REPO=o/r SHA=$sha F_RUST=true F_STEP=true F_GATE3=true F_FUZZ=false F_RECORDS=true F_ANY=true F_IMAGE=false \
    GITHUB_OUTPUT=$out RUNNER_TEMP=$work PATH="$work/bin:$PATH" FAKE_PRS=$good_prs FAKE_TREE=tree1 FAKE_RUNS=$good_runs \
    FAKE_ARTIFACTS=$good_arts FAKE_ZIP=$work/good.zip; bash "$here/main-run-mode.sh" > /dev/null )
grep -qx 'others=false' "$out" && grep -qx 'step=false' "$out" && grep -qx 'gate3=false' "$out" && grep -qx 'rust=true' "$out" \
  && ! grep -q windows "$out" && echo "ok   light run leaves out the other jobs and the Windows and macOS legs" \
  || { echo "FAIL light run outputs"; cat "$out"; fail=1; }
: > "$out"
( export EVENT=schedule REPO=o/r SHA=$sha GITHUB_OUTPUT=$out; bash "$here/main-run-mode.sh" > /dev/null )
grep -qx 'others=true' "$out" && grep -qx 'step=true' "$out" && grep -qx 'gate3=true' "$out" && grep -q windows "$out" \
  && echo "ok   scheduled run is due everywhere" || { echo "FAIL scheduled outputs"; cat "$out"; fail=1; }
exit $fail
