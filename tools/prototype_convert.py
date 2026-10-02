"""Prototype: one tbaMUD zone -> the Anima Mundi content format (PHASE-1-PLAN S2).

Not the converter: it made the example in third_party/tbamud/examples/ so the format could be reviewed.
The Rust converter (mundi-convert, D15) must produce the same content (compared value by value).
Usage: python tools/prototype_convert.py <zone> <out dir>   (reads ~/tbamud/lib/world, needs PyYAML)
"""
import re
import sys
from pathlib import Path

import yaml

WORLD = Path.home() / "tbamud" / "lib" / "world"
ZONE = int(sys.argv[1])
OUT = Path(sys.argv[2])

DIRS = ["north", "east", "south", "west", "up", "down"]
ROOM_BITS = ["dark", "death", "no_mob", "indoors", "peaceful", "soundproof", "no_track", "no_magic", "tunnel",
             "private", "godroom", "house", "house_crash", "atrium", "olc", "bfs_mark", "worldmap"]
SECTORS = ["inside", "city", "field", "forest", "hills", "mountains", "water_swim", "water_noswim", "flying",
           "underwater"]
MOB_BITS = ["spec", "sentinel", "scavenger", "isnpc", "aware", "aggressive", "stay_zone", "wimpy", "aggr_evil",
            "aggr_good", "aggr_neutral", "memory", "helper", "no_charm", "no_summon", "no_sleep", "no_bash",
            "no_blind", "no_kill", "dead"]
AFF_BITS = [None, "blind", "invisible", "detect_align", "detect_invis", "detect_magic", "sense_life", "waterwalk",
            "sanctuary", "group", "curse", "infravision", "poison", "protect_evil", "protect_good", "sleep",
            "no_track", "fly", "scuba", "sneak", "hide", "unused", "charm"]
ZONE_BITS = ["closed", "no_immortal", "quest", "grid", "no_build", "no_astral", "worldmap"]
POSITIONS = ["dead", "mortally_wounded", "incapacitated", "stunned", "sleeping", "resting", "sitting",
             "fighting", "standing"]
SEXES = ["neutral", "male", "female"]
ITEM_TYPES = ["undefined", "light", "scroll", "wand", "staff", "weapon", "furniture", "free", "treasure", "armor",
              "potion", "worn", "other", "trash", "free2", "container", "note", "drinkcon", "key", "food", "money",
              "pen", "boat", "fountain"]
WEAR_BITS = ["take", "finger", "neck", "body", "head", "legs", "feet", "hands", "arms", "shield", "about", "waist",
             "wrist", "wield", "hold"]
EXTRA_BITS = ["glow", "hum", "no_rent", "no_donate", "no_invis", "invisible", "magic", "no_drop", "bless",
              "anti_good", "anti_evil", "anti_neutral", "anti_mage", "anti_cleric", "anti_thief", "anti_warrior",
              "no_sell", "quest_item"]
APPLY = {1: "str", 2: "dex", 3: "int", 4: "wis", 5: "con", 6: "cha", 12: "mana", 13: "hit", 14: "move",
         17: "ac", 18: "hitroll", 19: "damroll", 20: "save_para", 21: "save_rod", 22: "save_petri",
         23: "save_breath", 24: "save_spell"}
ATTACKS = ["hit", "sting", "whip", "slash", "bite", "bludgeon", "crush", "pound", "claw", "maul", "thrash",
           "pierce", "blast", "punch", "stab"]
LIQUIDS = ["water", "beer", "wine", "ale", "dark_ale", "whisky", "lemonade", "firebreather", "local_speciality",
           "slime_mold_juice", "milk", "tea", "coffee", "blood", "salt_water", "clear_water"]
EQUIP_POS = ["light", "finger_right", "finger_left", "neck_1", "neck_2", "body", "head", "legs", "feet", "hands",
             "arms", "shield", "about", "waist", "wrist_right", "wrist_left", "wield", "hold"]
DOOR_STATE = ["open", "closed", "locked"]
RESET_MODE = ["never", "when_empty", "always"]


def lines(p):
    return p.read_text(encoding="latin-1").replace("\r", "").split("\n")


def tilde(ls, i):
    buf = []
    while i < len(ls):
        s = ls[i]
        i += 1
        if s.endswith("~"):
            buf.append(s[:-1])
            return "\n".join(buf), i
        buf.append(s)
    return "\n".join(buf), i


def flags(tok, names):
    v = 0
    if tok.lstrip("-").isdigit():
        v = int(tok)
    else:
        for ch in tok:
            v |= 1 << (ord(ch) - 97 if ch.islower() else 26 + ord(ch) - 65)
    return [n for b, n in enumerate(names) if n and v & (1 << b) and n != "isnpc"]


class Prose(str):
    """Paragraphs: written folded (>) in the file, loaded unwrapped; the renderer wraps."""


class Pre(str):
    """Verbatim text (maps, ASCII art): written literal (|)."""


def prose(text):
    """Hard-wrapped tbaMUD text -> paragraphs (the renderer wraps per language). Art stays verbatim."""
    raw = text.rstrip("\n")
    body = [l for l in raw.split("\n")]
    art = any(re.search(r"\S {4,}\S", l) or re.search(r"[|\\/_=+#*]{3,}", l) for l in body)
    if art:
        return Pre(raw + "\n")
    paras, cur = [], []
    for l in body:
        if (l.startswith("   ") or not l.strip()) and cur:
            paras.append(" ".join(cur))
            cur = []
        if l.strip():
            cur.append(l.strip())
    if cur:
        paras.append(" ".join(cur))
    return Prose("\n".join(paras))      # one line per paragraph: one blank line in the file


def dice(text):
    """'NdS+B' -> an int when nothing is rolled (0d0+20 is 20, 1d1+50 is 51), else the dice text."""
    m = re.fullmatch(r"(\d+)d(\d+)([+-]\d+)?", text)
    if not m:
        return text
    n, sides, bonus = int(m.group(1)), int(m.group(2)), int(m.group(3) or 0)
    if n == 0 or sides == 0:
        return bonus
    if sides == 1:
        return n + bonus
    return text if bonus else f"{n}d{sides}"


# --- zones (for ids of rooms in other zones)
zones = {}
for f in sorted((WORLD / "zon").glob("*.zon")):
    ls = lines(f)
    m = re.fullmatch(r"#(\d+)", ls[0].strip())
    if not m:
        continue
    builders, i = tilde(ls, 1)
    name, i = tilde(ls, i)
    head = ls[i].split()
    zones[int(m.group(1))] = dict(builders=builders.strip(), name=name.strip(), head=head, lines=ls[i + 1:])


def zone_of(vnum):
    for z, d in zones.items():
        if int(d["head"][0]) <= vnum <= int(d["head"][1]):
            return z
    return vnum // 100


def ref(vnum, kind="room"):
    """tba:<zone>:<kind>:<vnum> — tbaMUD numbers rooms, mobs and objects separately (D16)."""
    return f"tba:{zone_of(vnum)}:{kind}:{vnum}"


# --- rooms
rooms = {}
ls = lines(WORLD / "wld" / f"{ZONE}.wld")
i = 0
while i < len(ls):
    m = re.fullmatch(r"#(\d+)", ls[i].strip())
    if not m:
        i += 1
        continue
    v = int(m.group(1))
    name, i = tilde(ls, i + 1)
    desc, i = tilde(ls, i)
    head = ls[i].split()
    i += 1
    room = {"name": name.strip(), "description": prose(desc), "sector": SECTORS[int(head[-1])]}
    fl = flags(head[1], ROOM_BITS)
    if fl:
        room["flags"] = fl
    exits, extras = {}, []
    while i < len(ls) and ls[i].strip() != "S":
        t = ls[i].strip()
        if t.startswith("D") and t[1:].isdigit():
            look, i = tilde(ls, i + 1)
            kw, i = tilde(ls, i)
            info, key, to = (int(x) for x in ls[i].split()[:3])
            i += 1
            ex = {"to": ref(to)}
            if look.strip():
                ex["look"] = prose(look)
            if info:
                door = {"keywords": kw.split() or ["door"], "kind": "pickproof" if info == 2 else "door"}
                if key > 0:
                    door["key"] = ref(key, "obj")
                ex["door"] = door
            exits[DIRS[int(t[1:])]] = ex
        elif t == "E":
            kw, i = tilde(ls, i + 1)
            text, i = tilde(ls, i)
            extras.append({"keywords": kw.split(), "text": prose(text)})
        else:
            i += 1
    if exits:
        room["exits"] = {d: exits[d] for d in DIRS if d in exits}
    if extras:
        room["extras"] = extras
    rooms[ref(v)] = room

# --- mobs
mobs = {}
ls = lines(WORLD / "mob" / f"{ZONE}.mob")
i = 0
while i < len(ls):
    m = re.fullmatch(r"#(\d+)", ls[i].strip())
    if not m:
        i += 1
        continue
    v = int(m.group(1))
    kw, i = tilde(ls, i + 1)
    short, i = tilde(ls, i)
    long_, i = tilde(ls, i)
    desc, i = tilde(ls, i)
    f = ls[i].split()
    lvl, thac0, ac, hp, dam = ls[i + 1].split()
    gold, exp = ls[i + 2].split()
    pos = ls[i + 3].split()
    i += 4
    extra = {}
    if f[-1] == "E":
        while i < len(ls) and ls[i].strip() != "E":
            k, _, val = ls[i].partition(":")
            extra[re.sub(r"(?<!^)(?=[A-Z])", "_", k.strip()).lower()] = int(val) if val.strip().lstrip("-").isdigit() else val.strip()
            i += 1
    mob = {"keywords": kw.split(), "short": short.strip(), "long": long_.strip(), "description": prose(desc),
           "level": int(lvl), "sex": SEXES[int(pos[2])], "alignment": int(f[-2])}
    fl = flags(f[0], MOB_BITS)
    if fl:
        mob["flags"] = fl
    af = flags(f[4], AFF_BITS) if len(f) >= 10 else []
    if af:
        mob["affects"] = af
    mob["combat"] = {"thac0": int(thac0), "armor": int(ac), "hit_points": dice(hp), "damage": dice(dam)}
    if extra:
        mob["combat"].update(extra)
    mob["gold"], mob["exp"] = int(gold), int(exp)
    mob["position"] = {"load": POSITIONS[int(pos[0])], "default": POSITIONS[int(pos[1])]}
    mobs[ref(v, "mob")] = mob

# --- objects


def named_values(t, v):
    if t == "light":
        return {"hours": v[2]}
    if t == "weapon":
        return {"damage": f"{v[1]}d{v[2]}", "attack": ATTACKS[v[3]] if 0 <= v[3] < len(ATTACKS) else v[3]}
    if t == "armor":
        return {"armor": v[0]}
    if t in ("drinkcon", "fountain"):
        return {"capacity": v[0], "contains": v[1], "liquid": LIQUIDS[v[2]] if 0 <= v[2] < len(LIQUIDS) else v[2],
                "poisoned": bool(v[3])}
    if t == "food":
        return {"hours": v[0], "poisoned": bool(v[3])}
    if t == "money":
        return {"coins": v[0]}
    if t == "container":
        return {"capacity": v[0], "lock_flags": v[1], "key": ref(v[2], "obj") if v[2] > 0 else None, "corpse": bool(v[3])}
    return {"raw": v}


objects = {}
ls = lines(WORLD / "obj" / f"{ZONE}.obj")
i = 0
while i < len(ls):
    m = re.fullmatch(r"#(\d+)", ls[i].strip())
    if not m:
        i += 1
        continue
    v = int(m.group(1))
    kw, i = tilde(ls, i + 1)
    short, i = tilde(ls, i)
    long_, i = tilde(ls, i)
    action, i = tilde(ls, i)
    head = ls[i].split()
    vals = [int(x) for x in ls[i + 1].split()[:4]]
    num = [int(x) for x in ls[i + 2].split()[:5]] + [0, 0]
    i += 3
    t = ITEM_TYPES[int(head[0])]
    obj = {"type": t, "keywords": kw.split(), "short": short.strip(), "long": long_.strip()}
    if action.strip():
        obj["action"] = action.strip()
    fl = flags(head[1], EXTRA_BITS)
    if fl:
        obj["flags"] = fl
    obj["wear"] = flags(head[5], WEAR_BITS)
    obj["values"] = {k: x for k, x in named_values(t, vals).items() if x is not None}
    obj["weight"], obj["cost"], obj["rent"], obj["level"] = num[0], num[1], num[2], num[3]
    affects, extras = [], []
    while i < len(ls) and not re.fullmatch(r"#\d+", ls[i].strip()) and not ls[i].startswith("$"):
        if ls[i].startswith("A"):
            loc, mod = (int(x) for x in ls[i + 1].split())
            affects.append({"apply": APPLY.get(loc, loc), "modifier": mod})
            i += 2
        elif ls[i].startswith("E"):
            k, i = tilde(ls, i + 1)
            text, i = tilde(ls, i)
            extras.append({"keywords": k.split(), "text": prose(text)})
        else:
            i += 1
    if affects:
        obj["affects"] = affects
    if extras:
        obj["extras"] = extras
    objects[ref(v, "obj")] = obj

# --- zone and resets
z = zones[ZONE]
h = z["head"]
zone = {"id": f"tba:{ZONE}", "name": z["name"], "builders": z["builders"],
        "levels": {"min": int(h[8]), "max": int(h[9])} if len(h) >= 10 and int(h[8]) > 0 else None,
        "reset": {"every_minutes": int(h[2]), "when": RESET_MODE[int(h[3])]},
        "flags": flags(h[4], ZONE_BITS) if len(h) >= 10 else [],
        "source": {"vnums": [int(h[0]), int(h[1])]}}
zone = {k: x for k, x in zone.items() if x not in (None, [])}

spawns, doors, removes = [], [], []
last_mob = last_obj = None
for line in z["lines"]:
    f = line.split()
    if not f or f[0] not in "MOGEPDR":
        continue
    cmd, args = f[0], [int(x) for x in f[1:6] if x.lstrip("-").isdigit()]
    cond = args[0] == 1
    if cmd == "M":
        last_mob = {"mob": ref(args[1], "mob"), "room": ref(args[3]), "limit": args[2]}
        spawns.append(last_mob)
        last_obj = None
    elif cmd == "O":
        last_obj = {"object": ref(args[1], "obj"), "room": ref(args[3]), "limit": args[2]}
        spawns.append(last_obj)
    elif cmd == "E" and last_mob is not None:
        item = {"object": ref(args[1], "obj"), "limit": args[2]}
        if not cond:
            item["even_if_mob_not_loaded"] = True
        last_mob.setdefault("equip", {})[EQUIP_POS[args[3]]] = item
    elif cmd == "G" and last_mob is not None:
        item = {"object": ref(args[1], "obj"), "limit": args[2]}
        if not cond:
            item["even_if_mob_not_loaded"] = True
        last_mob.setdefault("carry", []).append(item)
    elif cmd == "P" and last_obj is not None:
        last_obj.setdefault("contents", []).append({"object": ref(args[1], "obj"), "limit": args[2]})
    elif cmd == "D":
        doors.append({"room": ref(args[1]), "exit": DIRS[args[2]], "state": DOOR_STATE[args[3]]})
    elif cmd == "R":
        removes.append({"room": ref(args[1]), "object": ref(args[2], "obj")})
left = []
for d in doors:                     # a door's reset state belongs to the door (rooms.yaml)
    ex = rooms.get(d["room"], {}).get("exits", {}).get(d["exit"])
    if ex is not None and "door" in ex:
        ex["door"]["reset"] = d["state"]
    else:
        left.append(d)
resets = {"spawns": spawns}
if left:
    resets["doors"] = left
if removes:
    resets["removes"] = removes


class Dumper(yaml.SafeDumper):
    pass


def text_repr(d, s):
    return d.represent_scalar("tag:yaml.org,2002:str", s)


def prose_repr(d, s):
    if len(s) <= 90 and "\n" not in s:
        return d.represent_scalar("tag:yaml.org,2002:str", str(s))
    return d.represent_scalar("tag:yaml.org,2002:str", str(s), style=">")


def pre_repr(d, s):
    return d.represent_scalar("tag:yaml.org,2002:str", str(s), style="|")


def list_repr(d, xs):
    flow = all(isinstance(x, (str, int, float, bool)) and not isinstance(x, (Prose, Pre)) for x in xs) and len(xs) <= 12
    return d.represent_sequence("tag:yaml.org,2002:seq", xs, flow_style=flow)


Dumper.add_representer(str, text_repr)
Dumper.add_representer(Prose, prose_repr)
Dumper.add_representer(Pre, pre_repr)
Dumper.add_representer(list, list_repr)


def dump(path, data, header):
    path.parent.mkdir(parents=True, exist_ok=True)
    text = yaml.dump(data, Dumper=Dumper, sort_keys=False, allow_unicode=True, width=100, default_flow_style=False)
    text = re.sub(r"\n(tba:\d+:[a-z]+:\d+:)", r"\n\n\1", text)        # a blank line between entries
    path.write_text(header + text, encoding="utf-8")


H = "# tbaMUD-derived (third_party/tbamud/NOTICE.md). Converted from tbaMUD zone {z}.\n"
LEGEND = {
 "zone.yaml": "# reset.every_minutes: minutes between resets; reset.when: never | when_empty (no players in the zone) | always\n",
 "rooms.yaml": ("# Entries by ID, in vnum order. Prose is folded (>): wrapped here, one paragraph per blank line, the\n"
                "# renderer wraps it per language. exits.<dir>: to (room ID), look (what `look <dir>` shows), door\n"
                "# (keywords, kind door|pickproof, key, reset: the state the zone reset puts it in).\n"),
 "mobs.yaml": ("# keywords: words to name it in commands; short: in sentences (\"the pit beast\"); long: its line in a room.\n"
               "# combat.armor is tbaMUD's armor class / 10 (lower is better); hit_points and damage: a number, or dice NdS+B when rolled.\n"),
 "objects.yaml": ("# values are named per type (armor: armor; weapon: damage, attack; drinkcon: capacity, contains, liquid...).\n"
                  "# level: minimum level to use it. affects: apply + modifier while worn.\n"),
 "resets.yaml": ("# What the zone reset puts back. A spawn is a mob in a room (with what it wears and carries) or an object\n"
                 "# in a room (with its contents). limit: at most this many of it may exist in the whole world; the item\n"
                 "# is not loaded if there are already that many. Door states are on the doors (rooms.yaml).\n"),
}
for name, data in (("zone.yaml", zone), ("rooms.yaml", rooms), ("mobs.yaml", mobs), ("objects.yaml", objects),
                   ("resets.yaml", resets)):
    dump(OUT / name, data, H.format(z=ZONE) + LEGEND[name])
print(len(rooms), "rooms", len(mobs), "mobs", len(objects), "objects", len(spawns), "spawns", len(doors), "doors")
