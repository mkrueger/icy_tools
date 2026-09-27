#!/usr/bin/env python3
"""Render the app icon assets of the GUI tools from their SVG masters.

Each crate keeps its master at crates/<app>/build/icon.svg. This script writes
the Linux, macOS, Windows and (where present) web icons next to it.
Requires inkscape and ImageMagick (magick).
"""
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APPS = ["icy_term", "icy_draw", "icy_view", "icy_mail"]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
FAVICON_SIZES = [16, 32, 48]


def copy(src: Path, dst: Path):
    shutil.copyfile(src, dst)


def render(svg: Path, png: Path, size: int):
    subprocess.run(
        ["inkscape", str(svg), "--export-type=png", f"--export-filename={png}", f"--export-width={size}", f"--export-height={size}"],
        check=True,
        capture_output=True,
    )


def full_bleed(svg_text: str) -> str:
    """Full-bleed variant for maskable/touch icons: square tile, symbol inside the safe zone."""
    lines = svg_text.splitlines()
    edge = next(i for i, line in enumerate(lines) if 'x="17" y="17"' in line)
    end = next(i for i, line in enumerate(lines) if line.strip() == "</svg>")
    head = [line.replace('rx="52"', 'rx="0"') for line in lines[:edge]]
    head[0] = re.sub(r'viewBox="[^"]*"', 'viewBox="16 16 224 224"', head[0])
    symbol = ['  <g transform="translate(128 128) scale(0.8) translate(-128 -128)">', *lines[edge + 1 : end], "  </g>"]
    return "\n".join(head + symbol + lines[end:]) + "\n"


def build(app: str, tmp: Path):
    build_dir = ROOT / "crates" / app / "build"
    master = build_dir / "icon.svg"
    pngs = {}
    for size in sorted(set(ICO_SIZES + FAVICON_SIZES + [1024])):
        pngs[size] = tmp / f"{app}-{size}.png"
        render(master, pngs[size], size)

    for d in ("linux", "mac", "windows"):
        (build_dir / d).mkdir(exist_ok=True)
    copy(pngs[128], build_dir / "linux" / "128x128.png")
    copy(pngs[256], build_dir / "linux" / "256x256.png")
    copy(pngs[256], build_dir / "mac" / "128x128@2x.png")
    subprocess.run(["magick", *[str(pngs[s]) for s in ICO_SIZES], str(build_dir / "windows" / "app.ico")], check=True)

    web = build_dir / "web"
    if web.is_dir():
        copy(pngs[256], web / "icon-256.png")
        copy(pngs[1024], web / "icon-1024.png")
        subprocess.run(["magick", *[str(pngs[s]) for s in FAVICON_SIZES], str(web / "favicon.ico")], check=True)
        bleed = tmp / f"{app}-bleed.svg"
        bleed.write_text(full_bleed(master.read_text()))
        render(bleed, web / "maskable_icon_x512.png", 512)
        touch = tmp / f"{app}-touch.png"
        render(bleed, touch, 192)
        subprocess.run(["magick", str(touch), "-alpha", "off", str(web / "icon_ios_touch_192.png")], check=True)
    print("rendered", app)


def main():
    apps = sys.argv[1:] or APPS
    with tempfile.TemporaryDirectory() as tmp:
        for app in apps:
            build(app, Path(tmp))


if __name__ == "__main__":
    main()
