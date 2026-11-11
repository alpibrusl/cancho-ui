#!/usr/bin/env python3
"""Writes tests/routemap_tiles_corpus.json: what a RouteMap with `tiles:` (docs/routes.md section 9) asks the tile host for, and where each point falls in the
drawing. The expected values are computed here from the rules of the document, with Python's math and a different formula for the Mercator ordinate
(asinh(tan) where the compiler, the replay and the page use ln(tan + sec)): no code is shared with the compiler, the replay or the page.
`python3 scripts/gen-routemap-tiles-corpus.py` rewrites the file."""
import json, math, os, random

TEMPLATE = "https://tiles.example/{z}/{x}/{y}.png"
MAXLAT = 85.0511287798
MARGIN = 32
TILE = 256

def world(lat, lon, z):
    lat = max(-MAXLAT, min(MAXLAT, lat))
    s = TILE * 2 ** z
    return ((lon + 180.0) / 360.0 * s, (1.0 - math.asinh(math.tan(math.radians(lat))) / math.pi) / 2.0 * s)

def tile_range(lo, hi, n):
    return max(0, min(n - 1, math.floor(lo / TILE))), max(0, min(n - 1, math.floor(hi / TILE)))

def plan(points):
    if not points: return None
    single = all(p == points[0] for p in points)
    def cover(z):
        n = 2 ** z
        w = [world(la, lo, z) for la, lo in points]
        xs = [a for a, _ in w]; ys = [b for _, b in w]
        m = TILE if single else MARGIN
        x0, x1 = tile_range(min(xs) - m, max(xs) + m, n)
        y0, y1 = tile_range(min(ys) - m, max(ys) + m, n)
        return x0, x1, y0, y1
    if single:
        z = 13
    else:
        z = 1
        for cand in range(18, 0, -1):
            x0, x1, y0, y1 = cover(cand)
            if x1 - x0 + 1 <= 4 and y1 - y0 + 1 <= 4:
                z = cand; break
    x0, x1, y0, y1 = cover(z)
    tiles = [TEMPLATE.replace('{z}', str(z)).replace('{x}', str(x)).replace('{y}', str(y)) for y in range(y0, y1 + 1) for x in range(x0, x1 + 1)]
    px = [[world(la, lo, z)[0] - x0 * TILE, world(la, lo, z)[1] - y0 * TILE] for la, lo in points]
    return {"zoom": z, "grid": [x1 - x0 + 1, y1 - y0 + 1], "tiles": tiles, "px": px}

def pos(p): return f"{p[0]!r},{p[1]!r}"

cases = []
def add(why, points):
    w = plan(points)
    cases.append({"why": why, "points": [pos(p) for p in points], "want": w})

add("one place: zoom 13 and the 3 x 3 tiles around its tile", [(45.9237, 6.8694)])
add("the same place twice is one place", [(45.9237, 6.8694), (45.9237, 6.8694)])
add("two places 2 km apart", [(45.9237, 6.8694), (45.9111, 6.8870)])
add("a trip in the Alps (the example): Geneva, Chamonix, Aiguille du Midi, Courmayeur", [(46.2381, 6.1089), (45.9237, 6.8694), (45.8786, 6.8873), (45.7969, 6.9689)])
add("Madrid and Barcelona", [(40.4168, -3.7038), (41.3874, 2.1686)])
add("two places a few metres apart go to the highest zoom", [(40.41690, -3.70350), (40.41692, -3.70352)])
add("a continent: Lisbon to Moscow", [(38.7223, -9.1393), (55.7558, 37.6173)])
add("two continents: Madrid and Tokyo", [(40.4168, -3.7038), (35.6762, 139.6503)])
add("the world: Sydney, London, Santiago", [(-33.8688, 151.2093), (51.5074, -0.1278), (-33.4489, -70.6693)])
add("across the antimeridian is the world's width (a documented limit)", [(-17.7134, 178.0650), (-16.6900, -179.8000)])
add("the equator and the prime meridian", [(0.0, 0.0)])
add("a point at the northern limit of the tiles", [(85.0511287798, 10.0)])
add("a polar latitude is clamped to the edge of the world", [(89.9, 10.0)])
add("the poles themselves", [(90.0, 0.0), (-90.0, 0.0)])
add("a polar latitude with a place near the tile range", [(89.0, 20.0), (84.0, 22.0)])
add("the south edge of the world, one place", [(-85.06, 0.0)])
add("the far east edge of the world, one place (the tile column is clipped)", [(10.0, 180.0)])
add("the far west edge, one place", [(10.0, -180.0)])
add("a north-south line of places is tall and narrow", [(40.0, 3.0), (41.5, 3.01)])
add("an east-west line of places is wide and short", [(40.0, 3.0), (40.01, 4.5)])
add("a place on each side of a tile edge at zoom 13", [(45.0, 6.0), (45.0, 6.0439)])
rng = random.Random(81)
for i in range(30):
    clat = rng.uniform(-75, 75); clon = rng.uniform(-170, 170)
    spread = 10 ** rng.uniform(-3, 1.6)
    n = rng.randint(2, 8)
    pts = [(round(max(-90, min(90, clat + rng.uniform(-1, 1) * spread)), 5), round(max(-180, min(180, clon + rng.uniform(-1, 1) * spread * 1.5)), 5)) for _ in range(n)]
    add(f"random cluster {i + 1}: {n} places spread over about {spread:.3f} degrees", pts)

out = {"note": "Written by scripts/gen-routemap-tiles-corpus.py (independent of the compiler). `template` is the tile address; `px` is where each located point falls in the drawing's pixels, cols * 256 wide and rows * 256 high.", "template": TEMPLATE, "cases": cases}
path = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tests', 'routemap_tiles_corpus.json')
with open(path, 'w') as f:
    json.dump(out, f, indent=1, ensure_ascii=False)
    f.write('\n')
zs = sorted({c["want"]["zoom"] for c in cases})
print(f"{len(cases)} cases, zooms {zs}")
