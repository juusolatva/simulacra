#!/bin/sh
# Compares the two math backends (cutover PR 3): builds the engine benchmark
# and the binary with the platform libm and with --features portable, then
#
#   1. times each libm function on both (examples/libm_bench.rs),
#   2. times scene building and shading of every style as a still
#      (examples/bench.rs, best of 3, ROUNDS times per backend),
#   3. times whole runs of the binary writing an animated PNG of every style
#      (ROUNDS times per backend).
#
# The backends alternate (which goes first changes every round, and for every
# style in the animations), so thermal and background drift hit both alike.
#
#   sh core/bench_backends.sh [--rounds N] [--frames N] [--width N] [--height N] OUTDIR
#
# Defaults: 3 rounds, 12 frames, 1920 x 1080 (about 15 minutes on a 4-core
# laptop). OUTDIR (created; must not exist yet) receives env.txt, libm.tsv,
# stills-<backend>-<round>.tsv, anim.tsv, log.txt and summary.tsv, which is
# also printed: per style the best still and the best and median animation
# time per backend, and the portable / system ratio. Needs cargo; POSIX sh
# and coreutils otherwise, so it also runs under Git Bash on Windows. Run it
# from anywhere; builds go to core/target/bench-system and bench-portable.
set -eu

usage() {
    echo "bench_backends.sh: $1" >&2
    echo "usage: bench_backends.sh [--rounds N] [--frames N] [--width N] [--height N] OUTDIR" >&2
    exit 2
}

rounds=3
frames=12
width=1920
height=1080
out=
while [ $# -gt 0 ]; do
    case $1 in
        --rounds | --frames | --width | --height)
            [ $# -ge 2 ] || usage "$1 needs a value"
            case $2 in '' | *[!0-9]*) usage "$1: not a whole number: $2" ;; esac
            [ "$2" -gt 0 ] || usage "$1 must be positive"
            case $1 in
                --rounds) rounds=$2 ;;
                --frames) frames=$2 ;;
                --width) width=$2 ;;
                --height) height=$2 ;;
            esac
            shift 2
            ;;
        -*) usage "unknown option $1" ;;
        *)
            [ -z "$out" ] || usage "more than one OUTDIR"
            out=$1
            shift
            ;;
    esac
done
[ -n "$out" ] || usage "missing OUTDIR"
[ ! -e "$out" ] || usage "$out already exists"

case $(date +%s%N) in
    *[!0-9]*) usage "date +%s%N must print nanoseconds (GNU coreutils)" ;;
esac

core=$(cd "$(dirname "$0")" && pwd)
exe=
case $(uname -s) in MINGW* | MSYS* | CYGWIN*) exe=.exe ;; esac
mkdir -p "$out"
out=$(cd "$out" && pwd)

bin() { echo "$core/target/bench-$1/release/$2$exe"; }

for backend in system portable; do
    features=
    [ "$backend" = portable ] && features="--features portable"
    echo "building the $backend backend" >&2
    # shellcheck disable=SC2086 # empty or one flag and its value
    cargo build --quiet --release --manifest-path "$core/Cargo.toml" \
        --target-dir "$core/target/bench-$backend" \
        -p simulacra_engine -p simulacra_cli --bin simulacra --example bench $features
done
cargo build --quiet --release --manifest-path "$core/Cargo.toml" \
    --target-dir "$core/target/bench-portable" \
    -p simulacra_engine --features portable --example libm_bench
for backend in system portable; do
    "$(bin "$backend" examples/bench)" --width 1 --height 1 --repeat 1 --styles waves |
        head -n 1 | grep -q " $backend math" || usage "the $backend build reports other math"
done

temperature() {
    if command -v sensors > /dev/null 2>&1; then
        sensors 2> /dev/null | awk '/^(Package id 0|Tctl|Tdie):/ {
            for (i = 2; i <= NF; i++) if ($i ~ /^\+[0-9]/) { print $i; exit } }'
    fi
}
state() {
    load=
    [ -r /proc/loadavg ] && load="load $(cut -d' ' -f1 /proc/loadavg) "
    echo "$(date +%T) ${load}$(temperature)"
}

{
    echo "date: $(date '+%Y-%m-%dT%H:%M:%S%z')"
    echo "system: $(uname -srm)"
    if [ -r /proc/cpuinfo ]; then
        echo "cpu: $(awk -F': *' '/^model name/ { print $2; exit }' /proc/cpuinfo)"
    fi
    echo "threads: $(nproc 2> /dev/null || echo unknown)"
    echo "rustc: $(rustc --version)"
    echo "commit: $(git -C "$core" rev-parse --short HEAD 2> /dev/null || echo unknown)"
    for ac in /sys/class/power_supply/AC*/online /sys/class/power_supply/ADP*/online; do
        [ -r "$ac" ] && echo "on AC: $(cat "$ac")"
    done
    governor=/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
    [ -r "$governor" ] && echo "governor: $(cat "$governor")"
    echo "workload: ${width}x${height}, $frames frames, $rounds rounds"
} > "$out/env.txt"
cat "$out/env.txt" >&2

# order N: the backends, system first for odd N.
order() {
    if [ $(($1 % 2)) -eq 1 ]; then echo "system portable"; else echo "portable system"; fi
}

echo "libm functions: $(state)" | tee -a "$out/log.txt" >&2
"$(bin portable examples/libm_bench)" > "$out/libm.tsv"

for round in $(seq 1 "$rounds"); do
    for backend in $(order "$round"); do
        echo "stills, round $round, $backend: $(state)" | tee -a "$out/log.txt" >&2
        "$(bin "$backend" examples/bench)" --width "$width" --height "$height" --repeat 3 \
            > "$out/stills-$backend-$round.tsv"
    done
done

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
printf 'round\tstyle\tbackend\tms\n' > "$out/anim.tsv"
styles=$("$(bin portable simulacra)" --list-styles)
for round in $(seq 1 "$rounds"); do
    echo "animations, round $round: $(state)" | tee -a "$out/log.txt" >&2
    i=0
    for style in $styles; do
        i=$((i + 1))
        for backend in $(order $((round + i))); do
            binary=$(bin "$backend" simulacra)
            # A relative --out: Git Bash would rewrite an absolute one.
            start=$(date +%s%N)
            (cd "$scratch" && "$binary" --style "$style" --width "$width" --height "$height" \
                --seed 1 --phase 0.37 --frames "$frames" --out a.png > /dev/null)
            end=$(date +%s%N)
            rm -f "$scratch/a.png"
            printf '%s\t%s\t%s\t%s\n' "$round" "$style" "$backend" \
                $(((end - start) / 1000000)) >> "$out/anim.tsv"
        done
    done
done
echo "done: $(state)" | tee -a "$out/log.txt" >&2

# Summary: stills as the best scene + shade time over all rounds; animations
# as the best and the median run. A ratio with a zero time (possible at tiny
# sizes or with a coarse timer) prints as n/a and stays out of the geometric
# mean.
{
    for f in "$out"/stills-*-*.tsv; do
        backend=$(basename "$f" | cut -d- -f2)
        awk -F'\t' -v b="$backend" '!/^#/ && $1 != "style" && $1 != "total" {
            print "still", $1, b, $2 + $3 }' "$f"
    done
    awk -F'\t' 'NR > 1 { print "anim", $2, $3, $4 }' "$out/anim.tsv"
} | LC_ALL=C sort -k1,1 -k2,2 -k3,3 -k4,4n | awk '
    function median(key,   n) { n = count[key]
        return n % 2 ? v[key, (n + 1) / 2] : (v[key, n / 2] + v[key, n / 2 + 1]) / 2 }
    function ratio(p, s) { return p > 0 && s > 0 ? sprintf("%.3f", p / s) : "n/a" }
    { key = $1 SUBSEP $2 SUBSEP $3; v[key, ++count[key]] = $4; styles[$2] = 1 }
    END {
        print "style\tstill_system_ms\tstill_portable_ms\tstill_ratio\tanim_system_ms\tanim_portable_ms\tanim_ratio\tanim_system_median_ms\tanim_portable_median_ms"
        fflush()
        sorted = "LC_ALL=C sort"
        for (s in styles) {
            ss = v["still", s, "system", 1]; sp = v["still", s, "portable", 1]
            as = v["anim", s, "system", 1]; ap = v["anim", s, "portable", 1]
            printf "%s\t%.1f\t%.1f\t%s\t%.0f\t%.0f\t%s\t%.0f\t%.0f\n", s, ss, sp, ratio(sp, ss),
                as, ap, ratio(ap, as), median("anim" SUBSEP s SUBSEP "system"),
                median("anim" SUBSEP s SUBSEP "portable") | sorted
            tss += ss; tsp += sp; tas += as; tap += ap
            if (sp > 0 && ss > 0) { lss += log(sp / ss); nss++ }
            if (ap > 0 && as > 0) { las += log(ap / as); nas++ }
        }
        close(sorted)
        printf "total\t%.1f\t%.1f\t%s\t%.0f\t%.0f\t%s\n", tss, tsp, ratio(tsp, tss), tas, tap, ratio(tap, tas)
        printf "geomean\t\t\t%s\t\t\t%s\n", nss ? sprintf("%.3f", exp(lss / nss)) : "n/a",
            nas ? sprintf("%.3f", exp(las / nas)) : "n/a"
    }' > "$out/summary.tsv"
cat "$out/libm.tsv"
echo
cat "$out/summary.tsv"
