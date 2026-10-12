#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SP-7: run the report's digest in three separate processes (cavalier_contours keeps an
# ahash table whose seed differs per process) and check the digests agree.
set -eu
cd "$(dirname "$0")"
cargo build --release --bin report
a=$(./target/release/report --digest)
b=$(./target/release/report --digest)
c=$(./target/release/report --digest)
echo "$a $b $c"
[ "$a" = "$b" ] && [ "$b" = "$c" ] && echo "identical" || { echo "DIFFERENT"; exit 1; }
