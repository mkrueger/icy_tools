"""Generates the "ANSI Block" icon family on a 32x32 grid (8 px cells, 256 px SVG)."""
from pathlib import Path
import sys

N = 32
CELL = 8
BLACK = "#000000"
WHITE = "#FFFFFF"
YELLOW = "#FFFF55"
DARKGRAY = "#555555"
LIGHTGRAY = "#AAAAAA"
LIGHTRED = "#FF5555"

TOOLS = {
    # name: (base, bright, description)
    "icy-term": ("#00AAAA", "#55FFFF", "Icy Term", "Pixel-Prompt mit gelbem Cursor auf cyanfarbener ANSI-Kachel."),
    "icy-draw": ("#AA00AA", "#FF55FF", "Icy Draw", "Pixel-Bleistift mit gelber Spitze auf magentafarbener ANSI-Kachel."),
    "icy-view": ("#0000AA", "#5555FF", "Icy View", "Pixel-Auge mit gelber Pupille auf blauer ANSI-Kachel."),
    "icy-play": ("#00AA00", "#55FF55", "Icy Play", "Play-Dreieck mit Fortschrittsbalken auf grüner ANSI-Kachel."),
    "icy-mail": ("#AA5500", "#FFAA55", "Icy Mail", "Pixel-Briefumschlag mit gelber Neu-Markierung auf brauner ANSI-Kachel."),
}


def tile_mask():
    inset = [5, 3, 2, 1, 1]
    mask = set()
    for y in range(2, 30):
        edge = min(y - 2, 29 - y)
        o = inset[edge] if edge < len(inset) else 0
        for x in range(2 + o, 30 - o):
            mask.add((x, y))
    return mask


def symbol_term():
    px = {}
    for r in range(13):
        start = 6 - abs(r - 6)
        for c in range(start, start + 4):
            px[(6 + c, 7 + r)] = WHITE
    for r in range(10, 13):
        for c in range(13, 21):
            px[(6 + c, 7 + r)] = YELLOW
    return px


def symbol_draw():
    px = {}
    for x in range(N):
        for y in range(N):
            u, v = x - y, x + y - 31
            if abs(v) > 3 or u < -16 or u > 16:
                continue
            p = (x, y - 1)
            if u < -8:
                if abs(v) * 2 > u + 16:
                    continue
                px[p] = DARKGRAY if u < -13 else YELLOW
            elif u > 12:
                px[p] = LIGHTRED
            elif u > 9:
                px[p] = LIGHTGRAY
            else:
                px[p] = WHITE
    return px


def symbol_view():
    px = {}
    cx, cy = 16, 14.5
    for x in range(N):
        for y in range(N):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            outer = abs(dx) < 11.5 and abs(dy) <= 7.6 * (1 - (dx / 11.5) ** 2)
            inner = abs(dx) < 9.0 and abs(dy) <= 5.0 * (1 - (dx / 9.0) ** 2)
            if outer and not inner:
                px[(x, y)] = WHITE
            if abs(dx) < 3 and abs(dy) < 3:
                px[(x, y)] = YELLOW
    return px


def symbol_play():
    px = {}
    a, b, c = (10.0, 5.0), (24.0, 12.5), (10.0, 20.0)

    def side(p, q, r):
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])

    for x in range(N):
        for y in range(N):
            p = (x + 0.5, y + 0.5)
            if side(a, b, p) >= 0 and side(b, c, p) >= 0 and side(c, a, p) >= 0:
                px[(x, y)] = WHITE
    for x in range(8, 24):
        for y in (22, 23):
            px[(x, y)] = YELLOW if x < 17 else DARKGRAY
    return px


def symbol_mail():
    px = {}
    x0, x1, y0, y1 = 6, 25, 9, 21
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            px[(x, y)] = WHITE
    for i in range(9):
        for t in range(2):
            px[(x0 + 1 + i, y0 + 1 + i - t)] = DARKGRAY
            px[(x1 - 1 - i, y0 + 1 + i - t)] = DARKGRAY
    for x in range(22, 29):
        for y in range(4, 11):
            inner = 23 <= x <= 27 and 5 <= y <= 9
            px[(x, y)] = YELLOW if inner else BLACK
    return px


SYMBOLS = {
    "icy-term": symbol_term,
    "icy-draw": symbol_draw,
    "icy-view": symbol_view,
    "icy-play": symbol_play,
    "icy-mail": symbol_mail,
}


def dither(x, y):
    # The ░ and ▒ shade characters as a fade towards the bottom edge.
    if y == 24:
        return x % 4 == 0
    if y == 25:
        return x % 4 == 2
    if y >= 26:
        return (x + y) % 2 == 0
    return False


def render(name):
    base, bright, title, desc = TOOLS[name]
    tile = tile_mask()
    grid = {}
    for (x, y) in tile:
        edge = any((x + dx, y + dy) not in tile for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        if edge:
            grid[(x, y)] = BLACK
        elif y == 3 or (y == 4 and (x + y) % 2 == 0):
            grid[(x, y)] = bright
        elif dither(x, y):
            grid[(x, y)] = BLACK
        else:
            grid[(x, y)] = base
    symbol = SYMBOLS[name]()
    for (x, y) in symbol:
        s = (x + 1, y + 1)
        if s not in symbol and s in grid and grid[s] != BLACK:
            grid[s] = BLACK
    for p, color in symbol.items():
        if p in grid:
            grid[p] = color

    rects = []
    for y in range(N):
        x = 0
        while x < N:
            color = grid.get((x, y))
            if color is None:
                x += 1
                continue
            start = x
            while x < N and grid.get((x, y)) == color:
                x += 1
            rects.append((color, start, y, x - start))
    by_color = {}
    for color, x, y, w in rects:
        by_color.setdefault(color, []).append(f"M{x * CELL} {y * CELL}h{w * CELL}v{CELL}h-{w * CELL}z")
    paths = "\n".join(f'  <path fill="{c}" d="{"".join(d)}"/>' for c, d in by_color.items())
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256" '
        f'shape-rendering="crispEdges" role="img" aria-labelledby="title desc">\n'
        f'  <title id="title">{title} - ANSI Block</title>\n'
        f'  <desc id="desc">{desc}</desc>\n{paths}\n</svg>\n'
    )


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name in TOOLS:
        (out / f"{name}.svg").write_text(render(name))
        print("wrote", out / f"{name}.svg")
