#!/usr/bin/env python3

"""Benchmark render time per style (via render_rgb, the path the CLI uses).

Examples:
    python3 bench_abstract_art.py                          # all styles, 256x256
    python3 bench_abstract_art.py --size 128 --styles torus,quasar
    python3 bench_abstract_art.py --width 640 --height 480
    python3 bench_abstract_art.py --save baseline.json     # record results
    python3 bench_abstract_art.py --compare baseline.json  # show speedups
    python3 bench_abstract_art.py --stages --phase 0.37    # scene, shader, PNG per style
    python3 bench_abstract_art.py --cli                    # end-to-end CLI runs, peak memory

Each style is rendered ``--repeat`` times with the same inputs and the best
time is reported, which is the least noisy estimate on a busy machine.

``--stages`` splits each native style into its stages: the Python scene
builder on a cold process cache (every ``functools.lru_cache`` cleared, as for
a still image or the first animation frame) and warm (best of the repeats),
the Rust shader, and PNG encoding including the file write. ``--cli`` runs the
CLI in a fresh process per workload (stills, APNG and GIF animations, 4K) and
reports wall time and peak resident memory from ``os.wait4``.
"""

import argparse
import json
import os
import pathlib
import platform
import subprocess
import sys
import tempfile
import time

import abstract_art as art


def time_style(style, width, height, seed, palette, repeat):
    best = float("inf")
    for _ in range(repeat):
        start = time.perf_counter()
        art.render_rgb(width, height, seed, style, palette)
        best = min(best, time.perf_counter() - start)
    return best


def _best_of(repeat, call):
    best = float("inf")
    for _ in range(repeat):
        start = time.perf_counter()
        call()
        best = min(best, time.perf_counter() - start)
    return best


def clear_caches():
    """Empty every ``functools.lru_cache`` in the module (a cold start)."""
    for value in vars(art).values():
        if callable(getattr(value, "cache_clear", None)):
            value.cache_clear()


def time_stages(style, width, height, seed, palette, phase, repeat, directory):
    """Seconds for the scene (cold and warm), the native shader and PNG encoding."""
    stops = art.PALETTES[palette]
    builder = art.NATIVE_STYLES[style]
    clear_caches()
    start = time.perf_counter()
    scene = builder(width, height, seed, stops, phase)
    cold = time.perf_counter() - start
    warm = _best_of(repeat, lambda: builder(width, height, seed, stops, phase))
    pixels = art._core.render(style, width, height, art._seed32(seed), stops, scene)
    shader = _best_of(repeat, lambda: art._core.render(style, width, height, art._seed32(seed),
                                                       stops, scene))
    path = pathlib.Path(directory) / "bench.png"
    encode = _best_of(repeat, lambda: art.write_png(path, pixels, width, height))
    return {"scene_cold": cold, "scene_warm": warm, "shader": shader, "png": encode}


STAGE_COLUMNS = ("scene_cold", "scene_warm", "shader", "png")


def print_stages(results):
    print("%-15s" % "style" + "".join(" %10s" % column for column in STAGE_COLUMNS))
    for style in sorted(results, key=lambda name: -sum(results[name].values())):
        print("%-15s" % style + "".join(" %10.4f" % results[style][c] for c in STAGE_COLUMNS))
    print("%-15s" % "TOTAL" + "".join(" %10.4f" % sum(r[c] for r in results.values())
                                       for c in STAGE_COLUMNS))


# (label, CLI arguments). Each runs in its own process, so caches start cold.
CLI_WORKLOADS = (
    ("still 640x480 waves", ["--style", "waves", "--width", "640", "--height", "480"]),
    ("still 640x480 attractor", ["--style", "attractor", "--width", "640", "--height", "480"]),
    ("still 640x480 woodblock", ["--style", "woodblock", "--width", "640", "--height", "480"]),
    ("still 640x480 lunar", ["--style", "lunar", "--width", "640", "--height", "480"]),
    ("still 3840x2160 plasma", ["--style", "plasma", "--width", "3840", "--height", "2160"]),
    ("still 3840x2160 ink", ["--style", "ink", "--width", "3840", "--height", "2160"]),
    ("apng 320x240x48 plasma", ["--style", "plasma", "--width", "320", "--height", "240",
                                "--frames", "48"]),
    ("apng 640x480x48 attractor", ["--style", "attractor", "--width", "640", "--height", "480",
                                   "--frames", "48"]),
    ("gif 320x240x48 plasma", ["--style", "plasma", "--width", "320", "--height", "240",
                               "--frames", "48", "gif"]),
    ("gif 640x480x48 woodblock", ["--style", "woodblock", "--width", "640", "--height", "480",
                                  "--frames", "48", "gif"]),
)


def run_cli(arguments, seed, directory):
    """Wall seconds and peak resident memory (MiB) of one CLI run."""
    suffix = ".gif" if arguments[-1] == "gif" else ".png"
    arguments = [a for a in arguments if a != "gif"]
    command = [sys.executable, str(pathlib.Path(art.__file__)), *arguments, "--seed", str(seed),
               "--out", str(pathlib.Path(directory) / ("cli" + suffix))]
    start = time.perf_counter()
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL)
    _, status, usage = os.wait4(process.pid, 0)
    elapsed = time.perf_counter() - start
    if os.waitstatus_to_exitcode(status):
        raise RuntimeError("CLI failed: %s" % " ".join(command))
    return {"seconds": elapsed, "peak_mib": usage.ru_maxrss / 1024.0}


def environment():
    return {"python": platform.python_version(), "system": platform.platform(),
            "machine": platform.machine(), "libc": "%s %s" % platform.libc_ver(),
            "cpus": os.cpu_count(), "native": art._core is not None}


def load_baseline(path, width, height, seed, palette):
    """Seconds per style from a ``--save`` file recorded with the same inputs.

    Size, seed and palette must all match: another seed can produce a scene
    with a different amount of work, which would distort the speedups.
    """
    with open(path) as handle:
        saved = json.load(handle)
    expected = {"width": width, "height": height, "seed": seed, "palette": palette}
    mismatched = ["%s=%s" % (key, saved.get(key)) for key in expected if saved.get(key) != expected[key]]
    if mismatched:
        raise ValueError("baseline was recorded with different inputs: %s" % ", ".join(mismatched))
    return saved["seconds"]


def print_report(results, baseline):
    """Print styles slowest first, with speedups when a baseline is given."""
    header = "%-15s %9s" % ("style", "seconds")
    if baseline:
        header += " %9s %8s" % ("before", "speedup")
    print(header)
    for style in sorted(results, key=lambda name: results[name], reverse=True):
        line = "%-15s %9.3f" % (style, results[style])
        if style in baseline:
            line += " %9.3f %7.1fx" % (baseline[style], baseline[style] / results[style])
        print(line)
    total = sum(results.values())
    print("%-15s %9.3f" % ("TOTAL", total))
    shared = [style for style in results if style in baseline]
    if shared:
        # Compare like with like: only styles present in both runs.
        now = sum(results[style] for style in shared)
        before = sum(baseline[style] for style in shared)
        print("%-15s %9.3f %9.3f %7.1fx" % ("SHARED (%d)" % len(shared), now, before, before / now))


def main():
    parser = argparse.ArgumentParser(description="Benchmark abstract_art render times")
    parser.add_argument("--size", type=int, default=256, help="square canvas size (default 256)")
    parser.add_argument("--width", type=int, help="canvas width (overrides --size)")
    parser.add_argument("--height", type=int, help="canvas height (overrides --size)")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--palette", default="deep", choices=sorted(art.PALETTES))
    parser.add_argument("--repeat", type=int, default=1, help="runs per style; best is kept")
    parser.add_argument("--styles", help="comma-separated subset (default: all)")
    parser.add_argument("--save", metavar="JSON", help="write results to this file")
    parser.add_argument("--compare", metavar="JSON", help="compare against saved results")
    parser.add_argument("--phase", type=float, default=0.0, help="animation phase (--stages)")
    parser.add_argument("--stages", action="store_true",
                        help="time scene, shader and PNG encoding per native style")
    parser.add_argument("--cli", action="store_true",
                        help="time end-to-end CLI workloads with peak memory")
    args = parser.parse_args()
    if args.stages or args.cli:
        run_detailed(parser, args)
        return

    styles = args.styles.split(",") if args.styles else list(art.STYLES)
    unknown = [s for s in styles if s not in art.STYLES]
    if unknown:
        parser.error("unknown style(s): %s" % ", ".join(unknown))
    width = args.size if args.width is None else args.width
    height = args.size if args.height is None else args.height
    if width <= 0 or height <= 0 or args.repeat <= 0:
        parser.error("sizes and repeat must be positive")
    baseline = {}
    if args.compare:
        try:
            baseline = load_baseline(args.compare, width, height, args.seed, args.palette)
        except ValueError as error:
            parser.error(str(error))

    results = {style: time_style(style, width, height, args.seed, args.palette, args.repeat)
               for style in styles}
    print_report(results, baseline)

    if args.save:
        with open(args.save, "w") as handle:
            json.dump({"width": width, "height": height, "seed": args.seed, "palette": args.palette,
                       "seconds": results}, handle, indent=1)


def run_detailed(parser, args):
    """``--stages`` and ``--cli``: print tables and optionally save them with the environment."""
    if art._core is None:
        parser.error("--stages and --cli need the Rust core")
    report = {"environment": environment()}
    with tempfile.TemporaryDirectory() as directory:
        if args.stages:
            styles = args.styles.split(",") if args.styles else list(art.NATIVE_STYLES)
            width = args.size if args.width is None else args.width
            height = args.size if args.height is None else args.height
            stages = {style: time_stages(style, width, height, args.seed, args.palette, args.phase,
                                         args.repeat, directory) for style in styles}
            print_stages(stages)
            report["stages"] = {"width": width, "height": height, "seed": args.seed,
                                "palette": args.palette, "phase": args.phase, "seconds": stages}
        if args.cli:
            print("%-28s %9s %9s" % ("workload", "seconds", "peak MiB"))
            report["cli"] = {}
            for label, arguments in CLI_WORKLOADS:
                result = run_cli(arguments, args.seed, directory)
                report["cli"][label] = result
                print("%-28s %9.3f %9.1f" % (label, result["seconds"], result["peak_mib"]))
    if args.save:
        with open(args.save, "w") as handle:
            json.dump(report, handle, indent=1)


if __name__ == "__main__":
    main()
