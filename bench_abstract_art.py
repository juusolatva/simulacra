#!/usr/bin/env python3

"""Benchmark render time per style.

Examples:
    python3 bench_abstract_art.py                          # all styles, 256x256
    python3 bench_abstract_art.py --size 128 --styles torus,quasar
    python3 bench_abstract_art.py --width 640 --height 480
    python3 bench_abstract_art.py --save baseline.json     # record results
    python3 bench_abstract_art.py --compare baseline.json  # show speedups

Each style is rendered ``--repeat`` times with the same inputs and the best
time is reported, which is the least noisy estimate on a busy machine.
"""

import argparse
import json
import time

import abstract_art as art


def time_style(style, width, height, seed, palette, repeat):
    best = float("inf")
    for _ in range(repeat):
        start = time.perf_counter()
        art.render(width, height, seed, style, palette)
        best = min(best, time.perf_counter() - start)
    return best


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
    args = parser.parse_args()

    styles = args.styles.split(",") if args.styles else list(art.STYLES)
    unknown = [s for s in styles if s not in art.STYLES]
    if unknown:
        parser.error("unknown style(s): %s" % ", ".join(unknown))
    width = args.width or args.size
    height = args.height or args.size
    if width <= 0 or height <= 0 or args.repeat <= 0:
        parser.error("sizes and repeat must be positive")

    baseline = {}
    if args.compare:
        with open(args.compare) as handle:
            saved = json.load(handle)
        if [saved.get("width"), saved.get("height")] != [width, height]:
            parser.error("baseline was recorded at %sx%s" % (saved.get("width"), saved.get("height")))
        baseline = saved["seconds"]

    results = {}
    for style in styles:
        results[style] = time_style(style, width, height, args.seed, args.palette, args.repeat)

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
    footer = "%-15s %9.3f" % ("TOTAL", total)
    if baseline:
        before = sum(baseline[s] for s in results if s in baseline)
        footer += " %9.3f %7.1fx" % (before, before / total)
    print(footer)

    if args.save:
        with open(args.save, "w") as handle:
            json.dump({"width": width, "height": height, "seed": args.seed, "palette": args.palette,
                       "seconds": results}, handle, indent=1)


if __name__ == "__main__":
    main()
