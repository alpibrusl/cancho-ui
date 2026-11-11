#!/usr/bin/env python3
"""Writes tests/routemap_corpus.json: what a RouteMap (docs/routes.md) says about a list of rows. The expected values are computed here, from the formulas of
docs/routes.md section 4, with Python's math: no code is shared with the compiler, the replay or the page. `python3 scripts/gen-routemap-corpus.py` rewrites the file."""
import json, math, os

R = 6371.0088
def rnd(x): return math.floor(x + 0.5)            # Math.round
def num(x): return str(int(x)) if float(x).is_integer() else repr(x)
def tenth(x): return num(rnd(x * 10) / 10)
def across_round(km):
    if km >= 100: return rnd(km / 10) * 10
    if km >= 10: return rnd(km)
    return rnd(km * 10) / 10
def hav(a, b):
    p1, p2 = math.radians(a[0]), math.radians(b[0])
    h = math.sin((p2 - p1) / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(math.radians(b[1] - a[1]) / 2) ** 2
    return 2 * R * math.asin(min(1, math.sqrt(h)))

def pos_of(text):
    parts = text.split(',')
    if len(parts) != 2: return None
    try:
        import re
        pat = re.compile(r'^-?(0|[1-9][0-9]*)(\.[0-9]+)?$')
        if not pat.match(parts[0]) or not pat.match(parts[1]): return None
        la, lo = float(parts[0]), float(parts[1])
    except ValueError: return None
    return (la, lo) if abs(la) <= 90 and abs(lo) <= 180 else None

def kind_ok(k):
    import re
    return re.match(r'^[A-Za-z0-9_-]{1,12}$', k) is not None

def jget(o, key):
    """json.get(row, key, ""): a String, a Boolean or a whole number read as text; anything else is the empty text."""
    v = o.get(key, '')
    if isinstance(v, bool): return 'true' if v else 'false'
    if isinstance(v, str): return v
    if isinstance(v, (int, float)) and float(v).is_integer(): return str(int(v))
    return ''

def want(rows, route):
    total = len(rows); rows = rows[:200]; count = len(rows)
    located = []; lst = []; km = 0.0
    for r in rows:
        try: o = json.loads(r)
        except Exception: o = {}
        if not isinstance(o, dict): o = {}
        name = jget(o, 'name') or '(unnamed)'
        kind = jget(o, 'kind')
        kp = f' ({kind})' if kind_ok(kind) else ''
        p = pos_of(jget(o, 'pos'))
        if p is None:
            lst.append(f'{name}{kp}, no position')
        else:
            n = len(located) + 1
            if not located: tail = 'start'
            else:
                d = hav(located[-1], p); km += d; tail = f'{tenth(d)} km from the previous point'
            located.append(p); lst.append(f'{n}. {name}{kp}, {tail}')
    nl = len(located)
    across = None
    if nl:
        lats = [p[0] for p in located]; lons = [p[1] for p in located]
        mean = sum(lats) / nl
        per = R * math.pi / 180
        across = math.hypot((max(lons) - min(lons)) * per * math.cos(math.radians(mean)), (max(lats) - min(lats)) * per)
    on = nl if route else 0
    if not route: km = 0.0
    if count == 0: s = 'route map with no places'
    else:
        s = f'route map of {count} place' + ('' if count == 1 else 's')
        if nl == 0: s += ', none with a position'
        else:
            s += f', {on} on the route' if route else ', route not drawn'
            if count > nl: s += f', {count - nl} without a position'
            a = across_round(across)
            s += ', all at one position' if a == 0 else f', about {num(a)} km across'
        if total > count: s += f', the first {count} of {total} shown'
    return {"count": count, "total": total, "located": nl, "on_route": on, "km": float(tenth(km)) if True else 0,
            "across_km": None if across is None else across_round(across), "summary": s, "list": lst}

def row(name, pos, kind=None):
    o = {"name": name, "pos": pos}
    if kind is not None: o["kind"] = kind
    return json.dumps(o, ensure_ascii=False)

cases = [
 ("a trip with one place that has no position", True, [row("Geneva airport", "46.2381,6.1089", "transport"), row("Chamonix", "45.9237,6.8694", "lodging"), row("Aiguille du Midi", "45.8786,6.8873", "activity"), row("Courmayeur", "45.7969,6.9689", "lodging"), row("To be decided", "unknown", "activity")]),
 ("the same trip with the route not drawn", False, [row("Geneva airport", "46.2381,6.1089", "transport"), row("Chamonix", "45.9237,6.8694", "lodging"), row("Aiguille du Midi", "45.8786,6.8873", "activity"), row("Courmayeur", "45.7969,6.9689", "lodging"), row("To be decided", "unknown", "activity")]),
 ("no points", True, []),
 ("one point", True, [row("Chamonix", "45.9237,6.8694", "lodging")]),
 ("the same point twice", True, [row("Chamonix", "45.9237,6.8694", "lodging"), row("Chamonix again", "45.9237,6.8694", "lodging")]),
 ("every position malformed", True, [row("a", "no comma"), row("b", "91,0"), row("c", "1,2,3"), row("d", "x,y"), row("e", " 1,2"), row("f", "1,181"), '{"name":"g"}', "not json", "[1,2]", "7"]),
 ("malformed positions among good ones", True, [row("a", "40.4168,-3.7038"), row("b", "oops"), row("c", "41.3874,2.1686"), row("d", ""), row("e", "37.3891,-5.9845")]),
 ("across the antimeridian is NOT handled: the box spans the world", True, [row("West of the line", "0,179"), row("East of the line", "0,-179")]),
 ("a long way: Madrid to Tokyo", True, [row("Madrid", "40.4168,-3.7038", "transport"), row("Tokyo", "35.6762,139.6503", "transport")]),
 ("a short hop, under 10 km across", True, [row("A", "40.4168,-3.7038"), row("B", "40.4531,-3.6883", "activity")]),
 ("kinds: too long, with a space, with a dash, unknown", True, [row("a", "10,10", "thirteen-char"), row("b", "10,11", "has space"), row("c", "10,12", "day-trip"), row("d", "10,13", "mystery")]),
 ("markup in a name stays text", True, [row("</text><script>alert(1)</script>", "10,10", "<img onerror=x>"), row("<b>b</b>", "10,11")]),
 ("a name that is missing or not text", True, ['{"pos":"10,10"}', '{"name":5,"pos":"10,11"}', '{"name":"","pos":"10,12"}']),
 ("the poles and the edges", True, [row("N", "90,0"), row("S", "-90,0"), row("W", "0,-180"), row("E", "0,180")]),
 ("a unicode name", True, [row("Café Zürich ☕", "47.3769,8.5417", "lodging"), row("東京", "35.6762,139.6503")]),
]
rows250 = [row(f"P{i}", f"{40 + i * 0.01:.2f},3.00") for i in range(250)]
cases.append(("250 rows: the first 200 are drawn and listed", True, rows250))
exact200 = [row(f"P{i}", f"{40 + i * 0.01:.2f},3.00") for i in range(200)]
cases.append(("exactly 200 rows", True, exact200))

out = {"about": "What a RouteMap (docs/routes.md) says about a list of rows: the counts, the length of the route, the width of the diagram (the diagonal of the bounding box in the drawing's projection), the accessible list and the sentence that is the SVG's accessible name after its label. Expected values computed outside the program by scripts/gen-routemap-corpus.py (Python's math).",
       "cases": [{"why": w, "route": r, "rows": rows, "want": want(rows, r)} for (w, r, rows) in cases]}
path = os.path.join(os.path.dirname(__file__), '..', 'tests', 'routemap_corpus.json')
with open(path, 'w', encoding='utf-8') as f:
    json.dump(out, f, indent=1, ensure_ascii=False); f.write('\n')
print(len(cases), 'cases')
