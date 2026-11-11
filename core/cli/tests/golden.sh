#!/bin/sh
# Golden hashes for the standalone binary.
#
#   golden.sh check BINARY [--container IMAGE] [--keep DIR]
#   golden.sh bless BINARY [--container IMAGE]
#
# Expands golden/workloads.txt (a line holding {style} once for every style
# BINARY --list-styles prints), renders each workload with BINARY into a
# temporary directory (or DIR, which must be empty or missing and is kept for
# the pixel test, golden_pixels.rs), and compares each output file's SHA-256
# with golden/SHA256SUMS; `bless` rewrites that file instead. With
# --container, BINARY runs inside IMAGE (which must not have Python), all
# workloads in one `docker run`.
#
# The hashes hold only for builds with --features portable. Exit status: 0 if
# everything matches, 1 on a mismatch or failure, 2 on a usage error.
# SIMULACRA_GOLDEN_DIR overrides the directory holding workloads.txt and
# SHA256SUMS (the self-test, golden_selftest.sh, uses it). POSIX sh and
# coreutils only: it runs under Git Bash on Windows and, for rendering, in a
# minimal Debian image.
set -eu

tab=$(printf '\t')

usage() {
    echo "golden.sh: $1" >&2
    echo "usage: golden.sh check BINARY [--container IMAGE] [--keep DIR]" >&2
    echo "       golden.sh bless BINARY [--container IMAGE]" >&2
    exit 2
}

fail() {
    echo "golden.sh: $1" >&2
    exit 1
}

absolute() {
    printf '%s/%s\n' "$(cd "$(dirname "$1")" && pwd)" "$(basename "$1")"
}

# render BINARY JOBS: renders each job (name<TAB>arguments) into the current
# directory; stops at the first failure. Also run inside the container.
render() {
    binary=$1
    errors=$(mktemp)
    set -f
    while IFS=$tab read -r name args; do
        # shellcheck disable=SC2086 # the arguments are split on purpose
        set -- $args
        status=0
        "$binary" "$@" --out "$name" < /dev/null > /dev/null 2> "$errors" || status=$?
        if [ "$status" -ne 0 ]; then
            echo "golden.sh: $name: simulacra $args exited with $status" >&2
            cat "$errors" >&2
            rm -f "$errors"
            exit 1
        fi
        if [ ! -f "$name" ]; then
            rm -f "$errors"
            fail "$name: simulacra $args wrote no file"
        fi
    done < "$2"
    set +f
    rm -f "$errors"
}

if [ "${1:-}" = render ]; then
    [ $# -eq 3 ] || usage "render takes BINARY JOBS"
    render "$2" "$3"
    exit 0
fi

command=${1:-}
case $command in
    check | bless) ;;
    *) usage "unknown command '${command}'" ;;
esac
[ $# -ge 2 ] || usage "missing BINARY"
binary=$2
shift 2
container=
keep=
while [ $# -gt 0 ]; do
    case $1 in
        --container) [ $# -ge 2 ] || usage "--container needs IMAGE"; container=$2; shift 2 ;;
        --keep) [ "$command" = check ] && [ $# -ge 2 ] || usage "--keep DIR is for check"; keep=$2; shift 2 ;;
        *) usage "unknown argument '$1'" ;;
    esac
done

if [ ! -f "$binary" ] && [ -f "$binary.exe" ]; then
    binary=$binary.exe
fi
[ -f "$binary" ] || usage "$binary is not a file"
binary=$(absolute "$binary")
here=$(cd "$(dirname "$0")" && pwd)
golden=${SIMULACRA_GOLDEN_DIR:-$here/golden}
golden=$(cd "$golden" && pwd)
self=$here/$(basename "$0")

if [ -n "$keep" ]; then
    if [ -d "$keep" ] && [ -n "$(ls -A "$keep")" ]; then
        usage "--keep $keep is not empty"
    fi
    mkdir -p "$keep"
    out=$(cd "$keep" && pwd)
    scratch=$(mktemp -d)
else
    out=$(mktemp -d)
    scratch=$(mktemp -d)
fi
cleanup() {
    rm -rf "$scratch"
    if [ -z "$keep" ]; then rm -rf "$out"; fi
}
trap cleanup EXIT

# Runs the binary with the given arguments, directly or in the container.
if [ -n "$container" ]; then
    if docker run --rm "$container" sh -c 'command -v python3 || command -v python' > /dev/null 2>&1; then
        fail "$container has Python, so the check would prove less"
    fi
    cp "$binary" "$out/.golden-bin"
    cp "$self" "$out/.golden.sh"
    user=$(id -u):$(id -g)
    run_binary() {
        docker run --rm --network none --user "$user" -v "$out:/work" -w /work \
            "$container" /work/.golden-bin "$@"
    }
else
    run_binary() { "$binary" "$@"; }
fi

run_binary --version
styles=$(run_binary --list-styles | tr -d '\r')

# Expand the workloads into name<TAB>arguments lines.
jobs=$scratch/jobs
: > "$jobs"
needs_styles=
set -f
tr -d '\r' < "$golden/workloads.txt" > "$scratch/workloads"
while IFS= read -r line; do
    # shellcheck disable=SC2086 # split the line into name and arguments
    set -- $line
    [ $# -gt 0 ] || continue
    case $1 in '#'*) continue ;; esac
    case $line in
        *'{style}'*)
            needs_styles=1
            for style in $styles; do
                # shellcheck disable=SC2046 # split the expanded line
                set -- $(printf '%s\n' "$line" | sed "s/{style}/$style/g")
                name=$1
                shift
                printf '%s\t%s\n' "$name" "$*" >> "$jobs"
            done ;;
        *)
            name=$1
            shift
            printf '%s\t%s\n' "$name" "$*" >> "$jobs" ;;
    esac
done < "$scratch/workloads"
set +f
if [ -n "$needs_styles" ] && [ -z "$styles" ]; then
    fail "--list-styles printed nothing"
fi
[ -s "$jobs" ] || fail "no workloads in $golden/workloads.txt"
cut -f1 "$jobs" > "$scratch/names"
bad=$(grep -v '^[A-Za-z0-9._-][A-Za-z0-9._-]*$' "$scratch/names" || true)
[ -z "$bad" ] || fail "bad workload name: $bad"
LC_ALL=C sort "$scratch/names" > "$scratch/names.sorted"
duplicates=$(uniq -d "$scratch/names.sorted")
[ -z "$duplicates" ] || fail "duplicate workload: $(printf '%s\n' "$duplicates" | tr '\n' ' ')"

# Render.
if [ -n "$container" ]; then
    cp "$jobs" "$out/.golden-jobs"
    docker run --rm --network none --user "$user" -v "$out:/work" -w /work \
        "$container" sh /work/.golden.sh render /work/.golden-bin /work/.golden-jobs
    rm -f "$out/.golden-bin" "$out/.golden.sh" "$out/.golden-jobs"
else
    (cd "$out" && render "$binary" "$jobs")
fi

# Hash, normalizing the binary-mode marker some sha256sum builds print.
(cd "$out" && xargs sha256sum < "$scratch/names.sorted") \
    | sed 's/^\([0-9a-f]*\) [ *]/\1  /' > "$scratch/actual"

if [ "$command" = bless ]; then
    LC_ALL=C sort -k 2 "$scratch/actual" > "$golden/SHA256SUMS"
    echo "wrote $(wc -l < "$golden/SHA256SUMS" | tr -d ' ') hashes to $golden/SHA256SUMS"
    exit 0
fi

# Check: the same names on both sides, then the same hashes.
manifest=$golden/SHA256SUMS
[ -s "$manifest" ] || fail "$manifest is empty or missing"
tr -d '\r' < "$manifest" | LC_ALL=C sort -k 2 > "$scratch/expected"
sed 's/^[0-9a-f]*  //' "$scratch/expected" > "$scratch/expected.names"
problems=0
for name in $(uniq -d "$scratch/expected.names"); do
    echo "duplicate in SHA256SUMS: $name" >&2
    problems=$((problems + 1))
done
for name in $(LC_ALL=C comm -23 "$scratch/names.sorted" "$scratch/expected.names"); do
    echo "no hash for $name" >&2
    problems=$((problems + 1))
done
for name in $(LC_ALL=C comm -13 "$scratch/names.sorted" "$scratch/expected.names"); do
    echo "hash without a workload: $name" >&2
    problems=$((problems + 1))
done
mismatches=0
while read -r name want got; do
    if [ "$want" != "$got" ]; then
        echo "$name: expected $want, got $got" >&2
        mismatches=$((mismatches + 1))
    fi
done <<JOINED
$(LC_ALL=C join -j 2 "$scratch/expected" "$scratch/actual")
JOINED
if [ "$mismatches" -gt 0 ]; then
    echo "only portable builds match these hashes: a mismatch in attractor or cymatics" \
        "usually means the binary was built without --features portable" >&2
fi
if [ $((problems + mismatches)) -gt 0 ]; then
    fail "$problems inventory problem(s), $mismatches mismatch(es)"
fi
echo "$(wc -l < "$scratch/actual" | tr -d ' ') workloads match"
