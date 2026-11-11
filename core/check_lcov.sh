#!/bin/sh
# Checks an LCOV report from core/coverage.sh so that coverage which silently
# stopped contributing fails CI instead of only lowering the figure:
#
#   - every DA line is well formed;
#   - the binary's main() (the line holding `let args = Args::parse();` in
#     core/cli/src/main.rs) has a hit: only the binary, run by the CLI
#     integration tests, executes it, so a miss means its instrumentation was
#     lost;
#   - something under core/engine/src/styles has a hit (the shaders).
#
# Usage: sh core/check_lcov.sh LCOV_FILE. POSIX sh and awk.
set -eu

[ $# -eq 1 ] || { echo "usage: check_lcov.sh LCOV_FILE" >&2; exit 2; }
report=$1
[ -f "$report" ] || { echo "check_lcov.sh: $report is not a file" >&2; exit 2; }
main_rs=$(cd "$(dirname "$0")" && pwd)/cli/src/main.rs
main_line=$(grep -n 'let args = Args::parse();' "$main_rs" | head -n 1 | cut -d: -f1)
if [ -z "$main_line" ]; then
    echo "check_lcov.sh: 'let args = Args::parse();' is not in $main_rs" >&2
    exit 1
fi

awk -v main_line="$main_line" '
    /^SF:/ {
        file = substr($0, 4)
        in_main = file ~ /core\/cli\/src\/main\.rs$/
        in_styles = index(file, "core/engine/src/styles/") > 0
        next
    }
    /^DA:/ {
        if ($0 !~ /^DA:[0-9]+,[0-9]+(,.*)?$/) {
            printf "malformed DA line %d: %s\n", NR, $0
            failed = 1
            next
        }
        split(substr($0, 4), field, ",")
        if (in_main && field[1] == main_line && field[2] > 0) main_hit = 1
        if (in_styles && field[2] > 0) styles_hit = 1
    }
    END {
        if (!main_hit) { print "the binary'"'"'s main() has no hits: its instrumentation was lost"; failed = 1 }
        if (!styles_hit) { print "no hits in core/engine/src/styles"; failed = 1 }
        exit failed
    }
' "$report"
