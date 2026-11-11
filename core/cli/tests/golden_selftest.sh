#!/bin/sh
# Self-test for golden.sh: runs it against a stub `simulacra` (a shell script
# that writes its arguments to --out) in a temporary golden directory, and
# checks the exit status and message of each case. POSIX sh; no Rust needed.
# shellcheck disable=SC2012 # ls lists names this script chose
set -eu

here=$(cd "$(dirname "$0")" && pwd)
golden_sh=$here/golden.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0

mkdir "$work/stub"
cat > "$work/stub/simulacra" <<'STUB'
#!/bin/sh
case "${1:-}" in
    --version) echo "stub 0"; exit 0 ;;
    --list-styles) for s in ${STUB_STYLES-a b}; do echo "$s"; done; exit 0 ;;
esac
out=
args=
while [ $# -gt 0 ]; do
    if [ "$1" = --out ]; then out=$2; shift 2; continue; fi
    if [ -n "${STUB_FAIL:-}" ] && [ "$1" = "$STUB_FAIL" ]; then
        echo "stub failure on $1" >&2
        exit 3
    fi
    args="$args $1"
    shift
done
printf '%s\n' "$args" > "$out"
STUB
chmod +x "$work/stub/simulacra"
stub=$work/stub/simulacra

# A fresh golden directory with two workload lines: a template (one output
# per stub style) and a single workload.
fresh() {
    rm -rf "$work/golden" "$work/keep" "$work/rel-out"
    mkdir "$work/golden"
    cat > "$work/golden/workloads.txt" <<'LIST'
# comment

{style}.png --style {style} --width 4
single.png --style a --seed -7
LIST
}

# expect STATUS SUBSTRING COMMAND...: runs COMMAND and checks both.
expect() {
    want_status=$1
    want_text=$2
    shift 2
    status=0
    output=$(SIMULACRA_GOLDEN_DIR=$work/golden "$@" 2>&1) || status=$?
    if [ "$status" -ne "$want_status" ]; then
        echo "FAIL [$case_name]: exit $status, expected $want_status"
        echo "$output" | sed 's/^/    /'
        failures=$((failures + 1))
    elif ! printf '%s' "$output" | grep -qF -- "$want_text"; then
        echo "FAIL [$case_name]: output lacks '$want_text'"
        echo "$output" | sed 's/^/    /'
        failures=$((failures + 1))
    else
        echo "ok   [$case_name]"
    fi
}

case_name="bless then check"
fresh
expect 0 "wrote" sh "$golden_sh" bless "$stub"
expect 0 "3 workloads match" sh "$golden_sh" check "$stub"

case_name="changed output"
sed 's/^[0-9a-f]\{8\}/00000000/' "$work/golden/SHA256SUMS" > "$work/sums" && mv "$work/sums" "$work/golden/SHA256SUMS"
expect 1 "a.png: expected 00000000" sh "$golden_sh" check "$stub"
expect 1 "only portable builds" sh "$golden_sh" check "$stub"

case_name="hash without a workload"
fresh
SIMULACRA_GOLDEN_DIR=$work/golden sh "$golden_sh" bless "$stub" > /dev/null
echo "0000000000000000000000000000000000000000000000000000000000000000  ghost.png" >> "$work/golden/SHA256SUMS"
expect 1 "hash without a workload: ghost.png" sh "$golden_sh" check "$stub"

case_name="duplicate hash"
fresh
SIMULACRA_GOLDEN_DIR=$work/golden sh "$golden_sh" bless "$stub" > /dev/null
tail -n 1 "$work/golden/SHA256SUMS" > "$work/last"
cat "$work/last" >> "$work/golden/SHA256SUMS"
expect 1 "duplicate in SHA256SUMS" sh "$golden_sh" check "$stub"

case_name="no hash"
fresh
SIMULACRA_GOLDEN_DIR=$work/golden sh "$golden_sh" bless "$stub" > /dev/null
echo "extra.png --style b" >> "$work/golden/workloads.txt"
expect 1 "no hash for extra.png" sh "$golden_sh" check "$stub"

case_name="empty manifest"
fresh
: > "$work/golden/SHA256SUMS"
expect 1 "SHA256SUMS is empty" sh "$golden_sh" check "$stub"

case_name="duplicate workload"
fresh
echo "a.png --style b" >> "$work/golden/workloads.txt"
expect 1 "duplicate workload: a.png" sh "$golden_sh" bless "$stub"

case_name="bad name"
fresh
echo "bad/name.png --style b" >> "$work/golden/workloads.txt"
expect 1 "bad workload name: bad/name.png" sh "$golden_sh" bless "$stub"

case_name="empty styles"
fresh
echo "{style}.png --style {style}" > "$work/golden/workloads.txt"
STUB_STYLES='' expect 1 "--list-styles printed nothing" sh "$golden_sh" bless "$stub"

case_name="no workloads"
fresh
echo "# nothing" > "$work/golden/workloads.txt"
expect 1 "no workloads" sh "$golden_sh" bless "$stub"

case_name="failing render"
fresh
STUB_FAIL=b expect 1 "b.png: simulacra --style b --width 4 exited with 3" sh "$golden_sh" bless "$stub"
STUB_FAIL=b expect 1 "stub failure on b" sh "$golden_sh" bless "$stub"

case_name="missing binary"
fresh
expect 2 "is not a file" sh "$golden_sh" check "$work/nowhere"

case_name="non-empty keep"
fresh
SIMULACRA_GOLDEN_DIR=$work/golden sh "$golden_sh" bless "$stub" > /dev/null
mkdir "$work/keep" && : > "$work/keep/old.png"
expect 2 "not empty" sh "$golden_sh" check "$stub" --keep "$work/keep"

case_name="keep contents"
rm -rf "$work/keep"
expect 0 "3 workloads match" sh "$golden_sh" check "$stub" --keep "$work/keep"
listing=$(ls -A "$work/keep" | tr '\n' ' ')
if [ "$listing" != "a.png b.png single.png " ]; then
    echo "FAIL [keep contents]: --keep holds '$listing'"
    failures=$((failures + 1))
fi

case_name="relative paths"
rm -rf "$work/rel-out"
# shellcheck disable=SC2016 # expanded by the inner shell
expect 0 "3 workloads match" sh -c 'cd "$1" && sh "$2" check ./stub/simulacra --keep rel-out' sh "$work" "$golden_sh"
if [ "$(ls -A "$work/rel-out" | tr '\n' ' ')" != "a.png b.png single.png " ]; then
    echo "FAIL [relative paths]: rel-out holds '$(ls -A "$work/rel-out" | tr '\n' ' ')'"
    failures=$((failures + 1))
fi

case_name="CRLF files"
for file in workloads.txt SHA256SUMS; do
    sed 's/$/\r/' "$work/golden/$file" > "$work/crlf" && mv "$work/crlf" "$work/golden/$file"
done
expect 0 "3 workloads match" sh "$golden_sh" check "$stub"

if [ "$failures" -ne 0 ]; then
    echo "$failures golden.sh self-test(s) failed"
    exit 1
fi
echo "all golden.sh self-tests passed"
