"""Check a built `simulacra` binary the way a user would run it, without Python.

The release-binaries workflow runs this on every target after downloading the
packaged binary. Only the checker needs Python: the binary is run as a plain
executable, and with ``--container IMAGE`` (Linux) inside a container image
that has neither Python nor a Rust toolchain.

``check BINARY``
    Prints ``--version`` and ``--list-styles``, then renders each listed style
    with the parameters of its portable fixture (``core/engine/tests/fixtures``)
    and requires the decoded pixels to hash to the fixture's SHA-256: the
    acceptance test of PLAN.md Phase 3. Styles join the check as their scene
    builders are ported, through ``--list-styles``. With ``--hashes FILE`` it
    also renders larger workloads (sizes, negative and large seeds, phases,
    palettes, custom stops, a short APNG animation and a GIF) and writes their
    hashes to ``FILE`` for ``compare``.

``compare FILE...``
    Requires every target's workload hashes to be identical. The portable
    math backend should give the same image everywhere, not only at the
    fixtures' 48 x 36.

Standard library only; run it with ``python -I`` from any directory.
"""

import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from fractions import Fraction
from pathlib import Path

FIXTURES = Path(__file__).resolve().parents[2] / "engine" / "tests" / "fixtures"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"

# The frame rate the workloads ask for, and the frame delay in seconds each
# format must then store: 1/12 in an APNG's fcTL, and in a GIF whole
# hundredths of a second, round(100 / fps) but at least 2 (_gif_delay).
FPS = 12
APNG_DELAY = Fraction(1, FPS)
GIF_DELAY = Fraction(max(2, round(100 / FPS)), 100)

# (width, height, seed, phase, palette, custom stops, frames). Sizes are not
# multiples of one another, include a tall canvas, and the seeds cover negative
# and 64-bit values (seed32 folds them), so the comparison is not just the
# fixture again at a larger scale. The last two are animations, an APNG and a
# GIF (the format follows the output name), hashed over all of their frames in
# order; a GIF frame is hashed as the RGB its colour table maps it to.
WORKLOADS = [
    (640, 480, 1, 0.0, "deep", None, 1),
    (640, 480, 17, 0.37, "lagoon", None, 1),
    (301, 517, -7, 1.25, "ember", None, 1),
    (517, 301, 2**40 + 3, -0.6, None, "0c061c,5814a8,ff8800", 1),
    (160, 120, 5, 0.8, "neon", None, 4),
    (150, 110, -3, 0.2, "sunset", None, 3, "gif"),
]


def decode_png(data):
    """Return (width, height, packed RGB) of an 8-bit RGB, non-interlaced PNG.

    For an APNG this is the first frame (its ``IDAT``); see ``decode_frames``.
    """
    width, height, frames, _ = decode_frames(data)
    return width, height, frames[0]


def _chunks(data):
    """Yield ``(tag, body)`` for each chunk after the signature, checking CRCs."""
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("not a PNG file")
    pos = len(PNG_SIGNATURE)
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        tag = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length : pos + 12 + length])
        if zlib.crc32(tag + body) != crc:
            raise ValueError(f"bad CRC in {tag!r}")
        pos += 12 + length
        yield tag, body
        if tag == b"IEND":
            return
    raise ValueError("missing IEND")


class _FrameReader:
    """Collects an image's compressed frames, chunk by chunk.

    Frame 0 is the ``IDAT`` image. In an APNG it must be the first animation
    frame (an ``fcTL`` before ``IDAT``); a default image outside the animation
    is not supported, since the binary never writes one.
    """

    def __init__(self):
        self.header = None
        self.frame_count = None  # from acTL
        self.loops = False  # acTL asks to play forever
        self.delays = []  # each fcTL's delay in seconds
        self.sequence = 0  # fcTL and fdAT chunks seen so far
        self.controls = 0  # fcTL chunks seen so far
        self.frames = [[]]

    def add(self, tag, body):
        if tag == b"IHDR":
            self.header = struct.unpack(">IIBBBBB", body)
        elif tag == b"acTL":
            self.frame_count, plays = struct.unpack(">II", body)
            self.loops = plays == 0
        elif tag == b"IDAT":
            if self.frame_count is not None and self.controls != 1:
                raise ValueError("the APNG's IDAT image is not its first frame")
            self.frames[0].append(body)
        elif tag in (b"fcTL", b"fdAT"):
            self._add_animation_chunk(tag, body)

    def _add_animation_chunk(self, tag, body):
        if self.frame_count is None:
            raise ValueError(f"{tag!r} without acTL")
        if struct.unpack(">I", body[:4])[0] != self.sequence:
            raise ValueError(f"{tag!r} out of sequence")
        self.sequence += 1
        if tag == b"fdAT":
            # Frame 0's pixels are the IDAT image; fdAT belongs to later frames.
            if self.controls < 2:
                raise ValueError("fdAT outside a frame after the first")
            self.frames[-1].append(body[4:])
            return
        if self.header is None or struct.unpack(">II", body[4:12]) != self.header[:2]:
            raise ValueError("an APNG frame does not cover the canvas")
        numerator, denominator = struct.unpack(">HH", body[20:24])
        # A zero denominator means hundredths of a second (APNG specification).
        self.delays.append(Fraction(numerator, denominator or 100))
        self.controls += 1
        if self.controls > 1:
            self.frames.append([])

    def check(self):
        """Return (width, height) once the whole file has been read."""
        if self.header is None:
            raise ValueError("missing IHDR")
        width, height, depth, color, _, _, interlace = self.header
        if (depth, color, interlace) != (8, 2, 0):
            raise ValueError(f"expected 8-bit RGB without interlace, got {self.header}")
        if self.frame_count is not None and self.controls == 0:
            raise ValueError("acTL without fcTL")
        if self.frame_count is not None and self.frame_count != len(self.frames):
            raise ValueError(f"acTL announces {self.frame_count} frames, found {len(self.frames)}")
        if not all(self.frames):
            raise ValueError("a frame has no image data")
        return width, height


def decode_frames(data):
    """Return (width, height, [packed RGB per frame], (loops, delays)) of a PNG
    or APNG; ``loops`` is whether it plays forever and ``delays`` holds each
    frame's delay in seconds (both empty for a still PNG).

    The image must be 8-bit RGB without interlace. An APNG needs ``acTL``
    before any ``fcTL`` or ``fdAT``, exactly one ``fcTL`` before ``IDAT`` (which
    holds the first frame), ``fdAT`` only in later frames, image data in every
    frame, every frame covering the whole canvas, the frame count ``acTL``
    announces, and sequence numbers counting up from 0.
    """
    reader = _FrameReader()
    for tag, body in _chunks(data):
        reader.add(tag, body)
    width, height = reader.check()
    stride = width * 3
    decoded = []
    for pieces in reader.frames:
        raw = zlib.decompress(b"".join(pieces))
        if len(raw) != height * (stride + 1):
            raise ValueError("image data has the wrong length")
        decoded.append(_unfilter(raw, stride, height))
    return width, height, decoded, (reader.loops, reader.delays)


def _lzw_decode(data):
    """Decode GIF LZW data with minimum code size 8 into colour indices."""
    clear, end = 256, 257
    fresh = [bytes([i]) for i in range(256)] + [b"", b""]
    table, out = list(fresh), bytearray()
    width, bits, buffer, position, previous = 9, 0, 0, 0, None
    while True:
        while bits < width:
            if position == len(data):
                raise ValueError("LZW data ends without an end code")
            buffer |= data[position] << bits
            position += 1
            bits += 8
        code = buffer & ((1 << width) - 1)
        buffer >>= width
        bits -= width
        if code == clear:
            table, width, previous = list(fresh), 9, None
            continue
        if code == end:
            return bytes(out)
        if code < len(table):
            entry = table[code]
        elif previous is not None and code == len(table):
            entry = previous + previous[:1]
        else:
            raise ValueError(f"LZW code {code} is not in the table")
        if previous is not None and len(table) < 4096:
            table.append(previous + entry[:1])
            if len(table) == 1 << width and width < 12:
                width += 1
        out += entry
        previous = entry


def _skip_sub_blocks(data, pos):
    """Return (the joined data sub-blocks starting at ``pos``, the position after them)."""
    pieces = []
    while data[pos]:
        pieces.append(data[pos + 1 : pos + 1 + data[pos]])
        pos += 1 + data[pos]
    return b"".join(pieces), pos + 1


def _gif_image(data, pos, size, table):
    """Decode the image at ``pos`` (after 0x2C); return (packed RGB, next position)."""
    left, top, width, height, flags = struct.unpack("<HHHHB", data[pos : pos + 9])
    if (left, top, width, height) != (0, 0, *size) or flags & 0xC0:
        raise ValueError("expected full-canvas GIF frames without a local table or interlace")
    if data[pos + 9] != 8:
        raise ValueError("expected LZW minimum code size 8")
    packed, pos = _skip_sub_blocks(data, pos + 10)
    indices = _lzw_decode(packed)
    if len(indices) != width * height:
        raise ValueError("a GIF frame has the wrong number of pixels")
    return b"".join(table[i] for i in indices), pos


def _gif_extension(data, pos, timing):
    """Read the extension at ``pos`` (0x21) into ``timing``; return the next position.

    A graphics control block adds its frame delay (seconds) to ``timing[1]``;
    a NETSCAPE2.0 block with loop count 0 sets ``timing[0]`` (plays forever).
    Other extensions are skipped.
    """
    label = data[pos + 1]
    payload, pos = _skip_sub_blocks(data, pos + 2)
    if label == 0xF9:
        timing[1].append(Fraction(struct.unpack("<H", payload[1:3])[0], 100))
    elif label == 0xFF and payload[:11] == b"NETSCAPE2.0":
        timing[0] = payload[11:14] == b"\x01\x00\x00"
    return pos


def decode_gif(data):
    """Return (width, height, [packed RGB per frame], (loops, delays)) of a GIF
    with a global table, timing as ``decode_frames`` gives it.

    Every frame must cover the canvas without a local table or interlacing, as
    the binary writes them.
    """
    if data[:6] != b"GIF89a":
        raise ValueError("not a GIF89a file")
    width, height, flags = struct.unpack("<HHB", data[6:11])
    if flags != 0xF7:
        raise ValueError("expected a global table of 256 colours")
    table = [data[13 + 3 * i : 16 + 3 * i] for i in range(256)]
    pos, frames, timing = 13 + 768, [], [False, []]
    while data[pos] != 0x3B:
        if data[pos] == 0x21:
            pos = _gif_extension(data, pos, timing)
        elif data[pos] == 0x2C:
            rgb, pos = _gif_image(data, pos + 1, (width, height), table)
            frames.append(rgb)
        else:
            raise ValueError(f"unexpected GIF block {data[pos]:#x}")
    return width, height, frames, tuple(timing)


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def _unfilter_row(kind, row, prior):
    """Undo one scanline's filter (PNG specification, section 9), 3 bytes per pixel."""
    out = bytearray(row)
    for i in range(len(out)):
        left = out[i - 3] if i >= 3 else 0
        up = prior[i]
        corner = prior[i - 3] if i >= 3 else 0
        if kind == 1:
            out[i] = (out[i] + left) & 0xFF
        elif kind == 2:
            out[i] = (out[i] + up) & 0xFF
        elif kind == 3:
            out[i] = (out[i] + (left + up) // 2) & 0xFF
        elif kind == 4:
            out[i] = (out[i] + _paeth(left, up, corner)) & 0xFF
    return out


def _unfilter(raw, stride, height):
    rows = bytearray()
    prior = bytes(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        row = raw[start + 1 : start + 1 + stride]
        if kind > 4:
            raise ValueError(f"unknown filter type {kind}")
        prior = row if kind == 0 else _unfilter_row(kind, row, prior)
        rows += prior
    return bytes(rows)


class Runner:
    """Runs the binary directly or inside a container image without Python."""

    def __init__(self, binary, workdir, container=None):
        self.workdir = workdir
        self.container = container
        if container:
            # Copy the binary into the mounted directory; the image has no Rust,
            # no Python and nothing of the build environment.
            shutil.copy2(binary, workdir / binary.name)
            # Write the image as the caller where there is a uid (not on Windows).
            user = ["--user", f"{os.getuid()}:{os.getgid()}"] if hasattr(os, "getuid") else []
            self.command = [
                "docker", "run", "--rm", "--network", "none", *user,
                "-v", f"{workdir}:/work", "-w", "/work",
                container, f"/work/{binary.name}",
            ]
        else:
            self.command = [str(binary.resolve())]

    def run(self, *args):
        result = subprocess.run(
            [*self.command, *args],
            cwd=self.workdir,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise SystemExit(
                f"simulacra {' '.join(args)} exited with {result.returncode}:\n"
                f"{result.stdout}{result.stderr}"
            )
        return result.stdout

    def render(self, name, width, height, seed, phase, palette, stops, frames=1, kind="png"):
        """Render one image or animation; return the SHA-256 of all its frames' RGB."""
        out = f"out.{kind}"
        args = [
            "--style", name, "--width", str(width), "--height", str(height),
            "--seed", str(seed), "--phase", repr(phase), "--out", out,
            "--frames", str(frames), "--fps", str(FPS),
        ]
        args += ["--palette-stops", stops] if stops else ["--palette", palette]
        self.run(*args)
        decode = decode_gif if kind == "gif" else decode_frames
        got_width, got_height, rgb, timing = decode((self.workdir / out).read_bytes())
        if (got_width, got_height) != (width, height):
            raise SystemExit(f"{name}: wrote {got_width} x {got_height}, asked for {width} x {height}")
        if len(rgb) != frames:
            raise SystemExit(f"{name}: wrote {len(rgb)} frames, asked for {frames}")
        if timing != expected_timing(kind, frames):
            raise SystemExit(
                f"{name}: wrote looping and delays {timing}, expected {expected_timing(kind, frames)}"
            )
        return hashlib.sha256(b"".join(rgb)).hexdigest()


def expected_timing(kind, frames):
    """(loops, delays) a file of ``frames`` frames must store at ``FPS``.

    Python writes a GIF's loop block only for more than one frame, but a
    graphics control block (with the delay) for every frame; a still PNG has
    neither, and an APNG loops forever with a delay in every fcTL.
    """
    if kind == "gif":
        return frames > 1, [GIF_DELAY] * frames
    if frames > 1:
        return True, [APNG_DELAY] * frames
    return False, []


def workload_key(name, workload):
    return f"{name} " + " ".join(str(v) for v in workload if v is not None)


def expected_keys(styles):
    return {workload_key(name, workload) for name in styles for workload in WORKLOADS}


def require_no_python(container):
    """The container image must not offer Python, or the check proves less."""
    probe = subprocess.run(
        ["docker", "run", "--rm", container, "sh", "-c",
         "command -v python3 || command -v python"],
        capture_output=True, text=True, check=False,
    )
    if probe.returncode == 0:
        raise SystemExit(f"{container} has Python at {probe.stdout.strip()}")


def check(args):
    binary = Path(args.binary)
    if not binary.is_file() and binary.with_suffix(".exe").is_file():
        binary = binary.with_suffix(".exe")
    if not binary.is_file():
        raise SystemExit(f"{binary} is not a file")
    if args.container:
        require_no_python(args.container)
    with tempfile.TemporaryDirectory() as tmp:
        runner = Runner(binary, Path(tmp), args.container)
        print(runner.run("--version").strip())
        styles = runner.run("--list-styles").split()
        if not styles:
            raise SystemExit("--list-styles printed nothing")
        failures = []
        for name in styles:
            fixture = json.loads((FIXTURES / f"{name}.json").read_text(encoding="utf-8"))
            digest = runner.render(
                name, fixture["width"], fixture["height"], fixture["seed"],
                fixture["phase"], fixture["palette"], None,
            )
            verdict = "ok" if digest == fixture["sha256"] else "MISMATCH"
            print(f"fixture {name}: {verdict} {digest}")
            if digest != fixture["sha256"]:
                failures.append(name)
        if failures:
            raise SystemExit(
                f"{', '.join(failures)} differ from the portable fixtures "
                "(is the binary built with --features portable?)"
            )
        if args.hashes:
            hashes = {}
            for name in styles:
                for workload in WORKLOADS:
                    key = workload_key(name, workload)
                    hashes[key] = runner.render(name, *workload)
                    print(f"workload {key}: {hashes[key]}")
            Path(args.hashes).write_text(
                json.dumps({"target": args.target, "styles": styles, "hashes": hashes}, indent=1) + "\n",
                encoding="utf-8",
            )
    print(f"{binary}: {len(styles)} styles match their portable fixtures")


def compare(args):
    reports = [json.loads(Path(p).read_text(encoding="utf-8")) for p in args.files]
    if len(reports) < 2:
        raise SystemExit("compare needs the hashes of at least two targets")
    for path, report in zip(args.files, reports):
        # Every report must hold every workload for its styles, so reports
        # that all lack the same entries (or are all empty) cannot pass.
        if not report.get("styles"):
            raise SystemExit(f"{path}: no styles")
        missing = expected_keys(report["styles"]) - report["hashes"].keys()
        extra = report["hashes"].keys() - expected_keys(report["styles"])
        if missing or extra:
            raise SystemExit(
                f"{path}: missing workloads {sorted(missing)}, unexpected {sorted(extra)}"
            )
    reference = reports[0]
    differences = []
    for report in reports[1:]:
        keys = reference["hashes"].keys() | report["hashes"].keys()
        for key in sorted(keys):
            mine = reference["hashes"].get(key)
            theirs = report["hashes"].get(key)
            if mine != theirs:
                differences.append(
                    f"{key}: {reference['target']} {mine}, {report['target']} {theirs}"
                )
    if differences:
        raise SystemExit("targets render differently:\n" + "\n".join(differences))
    targets = ", ".join(r["target"] for r in reports)
    print(f"{len(reference['hashes'])} workloads render identically on {targets}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    check_parser = commands.add_parser("check", help="check one binary")
    check_parser.add_argument("binary")
    check_parser.add_argument("--container", help="run the binary in this image (needs docker)")
    check_parser.add_argument("--hashes", help="also render the workloads and write their hashes here")
    check_parser.add_argument("--target", default=sys.platform, help="target name for the hashes file")
    check_parser.set_defaults(func=check)
    compare_parser = commands.add_parser("compare", help="compare targets' workload hashes")
    compare_parser.add_argument("files", nargs="+")
    compare_parser.set_defaults(func=compare)
    args = parser.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main()
