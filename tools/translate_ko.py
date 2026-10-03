"""Korean translation batch for converted content (D19): local LLM, glossary first, re-translate only what changed.

  python tools/translate_ko.py translate 186 30     # translate zones (resumable, skips what is up to date)
  python tools/translate_ko.py check 186 30         # list fields with no translation (missing-translation report)
  python tools/translate_ko.py translate combat     # the combat message file (messages/combat.yaml -> ko/combat.yaml)

- Source: third_party/tbamud/content/<zone>/{rooms,mobs,objects}.yaml (English, the base)
- Output: third_party/tbamud/locales/ko/<zone>.yaml (same IDs and shape, only translated strings; D11, D17: no keywords)
- State: third_party/tbamud/locales/ko/<zone>.state.json — per entry the hash of its English source and the hashes of
  the glossary entries the translation used. An entry is translated again only when its source changed or one of those
  glossary entries changed (e.g. 큰길 -> 중앙로), not when unrelated terms change.
- Model: the LM Studio host's dense model (quality over speed; overnight batch). Thinking off (empty <think> prefill).
- Checks on every answer, else retry: same IDs and fields, same paragraph count, no Han or kana (Qwen leaks Chinese;
  Latin letters, digits, punctuation and glossary terms kept in English are fine), glossary terms used.
- Markup (D20): `{yellow}...{/yellow}` must survive translation — the same tags, in the same order, paired; else retry.
- Verbatim texts ({preformatted}: maps, pictures) are left in English in phase 1.
- Combat messages: one entry per attack variant, its lines keyed `die.attacker` .. `god.room`. act() codes:
  $n (the attacker; 당신 when it is the reader) and $N (the victim) stay, a particle after one is a pair in braces
  (`$N{을/를}`, D23), $p (the weapon) stays, and the pronoun codes ($e $m $s, $E $M $S) become the name again.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
CONTENT = ROOT / "third_party/tbamud/content"
LOCALE = ROOT / "third_party/tbamud/locales/ko"
GLOSSARY = LOCALE / "glossary.yaml"
URL = "http://192.168.1.191:1234/v1/completions"
MODEL = "qwen/qwen3.8-27b"
BATCH = 4
TRIES = 3
FORBIDDEN = re.compile("[぀-ヿㇰ-ㇿｦ-ﾟ"          # kana
                       "㐀-䶿一-鿿豈-﫿\U00020000-\U0002fa1f]")   # Han


# ---------------------------------------------------------------- source fields

def paragraphs(text):
    """Content Text -> list of paragraphs, or None for verbatim (left untranslated)."""
    if isinstance(text, dict):
        return None
    if isinstance(text, list):
        return [p.rstrip("\n") for p in text]
    return [str(text).rstrip("\n")]


def fields(kind, entry):
    """The translatable strings of an entry: {field path: str | [paragraphs]}."""
    out = {}
    if kind == "rooms":
        out["name"] = entry["name"]
        if (p := paragraphs(entry["description"])) is not None:
            out["description"] = p
        for d, ex in (entry.get("exits") or {}).items():
            if ex.get("look") is not None and (p := paragraphs(ex["look"])) is not None:
                out[f"exits.{d}.look"] = p
    else:
        out["short"] = entry["short"]
        out["long"] = entry["long"]
        if kind == "mobs" and (p := paragraphs(entry.get("description", ""))) and any(p):
            out["description"] = p
        if entry.get("action"):
            out["action"] = entry["action"]
    for i, ex in enumerate(entry.get("extras") or []):
        if (p := paragraphs(ex["text"])) is not None:
            out[f"extras.{i}.text"] = p
    return out


COMBAT = ROOT / "third_party/tbamud/messages/combat.yaml"
OUTCOMES = ("die", "miss", "hit", "god")
ROLES = ("attacker", "victim", "room")


def load_combat():
    """{"attack:<number>:<variant>": ("combat", {"die.attacker": line, ...})}, null lines left out."""
    out = {}
    for a in yaml.safe_load(COMBAT.read_text(encoding="utf-8"))["attacks"]:
        for i, v in enumerate(a["variants"], 1):
            f = {f"{o}.{r}": v[o][r] for o in OUTCOMES for r in ROLES if (v.get(o) or {}).get(r)}
            out[f"attack:{a['number']}:{i}"] = ("combat", f)
    return out


def load_zone(zone):
    if zone == "combat":
        return load_combat()
    out = {}
    for kind in ("rooms", "mobs", "objects"):
        f = CONTENT / str(zone) / f"{kind}.yaml"
        data = yaml.safe_load(f.read_text(encoding="utf-8")) if f.exists() else {}
        for id_, entry in (data or {}).items():
            out[id_] = (kind, fields(kind, entry))
    return out


TAG = re.compile(r"\{\{|\}\}|\{(/?[a-z0-9_:]+)\}")


def tags(text):
    """Markup tags of a text in order (D20); ValueError if unbalanced."""
    out, stack = [], []
    for m in TAG.finditer(text):
        if m.group(1) is None:
            continue
        t = m.group(1)
        if t.startswith("/"):
            if not stack or stack.pop() != t[1:]:
                raise ValueError(f"unbalanced {{{t}}}")
        else:
            stack.append(t)
        out.append(t)
    if stack:
        raise ValueError(f"{{{stack[-1]}}} not closed")
    return out


def sha(obj):
    return hashlib.sha256(json.dumps(obj, ensure_ascii=False, sort_keys=True).encode()).hexdigest()[:16]


# ---------------------------------------------------------------- glossary

def load_glossary():
    g = yaml.safe_load(GLOSSARY.read_text(encoding="utf-8"))
    return g["conventions"], g["terms"]


def text_of(f):
    return " ".join(v if isinstance(v, str) else " ".join(v) for v in f.values())


def terms_in(text, terms):
    """Glossary entries that occur in an English text (whole words, case-insensitive)."""
    low = text.lower()
    return [t for t in terms if re.search(r"(?<![a-z])" + re.escape(t["en"].lower()) + r"(?![a-z])", low)]


def term_hash(t):
    return sha([t["en"], t["ko"], t.get("note", "")])


# ---------------------------------------------------------------- the model

SYSTEM = """You translate the text of a fantasy text MUD (tbaMUD) from English into natural Korean.
Rules:
{conventions}
- Translate every string. Keep the JSON structure exactly: the same IDs, the same keys, and for a list the same
  number of paragraphs in the same order.
- Write Korean. Do not write Chinese characters (漢字) or Japanese kana. Latin letters only where the glossary says.
- Keep every markup tag like {{yellow}} and {{/yellow}} exactly as written, around the Korean words that translate the
  words it surrounds, in the same order. Do not add, drop or translate tags.
- Use these glossary terms exactly when the English term appears:
{glossary}
Reply with the JSON object only."""

COMBAT_RULES = """
These are combat messages. In each line $n is the attacker and $N the victim (a name, or 당신 for the reader); $p is
the weapon. Keep $n, $N and $p as written. Write a Korean particle after a code as a pair in braces, never attached:
$n{이/가}, $N{을/를}, $n{은/는}, $N{과/와}, $p{으로/로}, $N{아/야} (only these six pairs). Particles that never
change (의, 에게, 에, 도, 만, 에서, 한테) are written attached: $N에게, $n의. Do not use $e $m $s $E $M $S (he, him, his): write
$n or $N again, or leave the person out when Korean reads better without it. Keys ending in .attacker are seen by the
attacker (English "you" = the attacker: 당신), .victim by the victim ("you" = the victim: 당신), .room by others.
Short vivid 해라체 narration ("~다", "~했다!"). Keep exclamation marks and jokes."""


def prompt(batch, conventions, terms):
    used = {t["en"]: t for e in batch.values() for t in terms_in(text_of(e), terms)}
    gl = "\n".join(f"  {t['en']} = {t['ko']}" + (f"  ({t['note']})" if t.get("note") else "") for t in used.values()) or "  (none)"
    conv = "\n".join(f"- {k}: {v}" for k, v in conventions.items())
    system = SYSTEM.format(conventions=conv, glossary=gl)
    if any(id_.startswith("attack:") for id_ in batch):
        system += COMBAT_RULES
    user = json.dumps(batch, ensure_ascii=False, indent=1)
    return (f"<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n"
            f"<|im_start|>assistant\n<think>\n\n</think>\n\n"), used


def ask(text, size, attempt=1):
    """`size`: characters of the batch's source JSON. The answer gets about as many tokens (Korean takes
    more tokens per character than English), so a model stuck repeating itself stops early. A retry
    is hotter and penalises repeats: the model sometimes loops on one clause until the cap."""
    limit = min(6000, max(800, size))
    body = {"model": MODEL, "prompt": text, "max_tokens": limit, "stop": ["<|im_end|>"],
            "temperature": 0.2 + 0.25 * (attempt - 1), "repeat_penalty": 1.0 + 0.1 * (attempt - 1),
            "frequency_penalty": 0.3 * (attempt - 1)}
    rq = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(rq, timeout=1800) as r:
        out = json.loads(r.read())
    return out["choices"][0]["text"], out.get("usage", {})


def parse(text):
    text = re.sub(r"(?s)^.*</think>", "", text).strip()
    text = re.sub(r"^```(json)?|```$", "", text.strip()).strip()
    # A pronoun code left in: the name it stands for ($M -> $N, $s -> $n의).
    text = re.sub(r"\$([emsEMS])(의?)", lambda m: "$" + ("n" if m.group(1).islower() else "N") + ("의" if m.group(1) in "sS" and not m.group(2) else "") + m.group(2), text)
    # A single particle in braces ($N{에게}, 당신{의}) is harmless: unbrace it; a stray half after a pair goes.
    text = re.sub(r"\{([가-힣]+)\}", r"\1", text)
    text = re.sub(r"(\{(?:이/가|은/는|을/를|과/와|으로/로|아/야)\})/[가-힣]", r"\1", text)
    # A pair after a word, not a code (당신{은/는}): the word is fixed, so is its particle.
    text = re.sub(r"당신\{(이/가|은/는|을/를|과/와|으로/로|아/야)\}", lambda m: "당신" + m.group(1).split("/")[0], text)
    # A varying particle attached ($N은) is the right particle in the wrong notation: make it the pair.
    text = re.sub(r"(\$[nNp])(으로|이|가|은|는|을|를|과|와|로|아|야)(?![가-힣])", lambda m: m.group(1) + "{" + PAIR_OF[m.group(2)] + "}", text)
    return json.loads(text[text.find("{"):text.rfind("}") + 1])


def problems(src, out, used):
    """Why a translated batch is not acceptable (empty: fine)."""
    bad = []
    if set(out) != set(src):
        return [f"IDs differ: {sorted(set(src) ^ set(out))[:3]}"]
    for id_, f in src.items():
        t = out[id_]
        if not isinstance(t, dict) or set(t) != set(f):
            bad.append(f"{id_}: fields differ")
            continue
        for k, v in f.items():
            tv = t[k]
            if isinstance(v, list):
                if not isinstance(tv, list) or len(tv) != len(v):
                    bad.append(f"{id_}.{k}: paragraph count {len(tv) if isinstance(tv, list) else '?'} != {len(v)}")
                    continue
                strings = tv
            else:
                if not isinstance(tv, str):
                    bad.append(f"{id_}.{k}: not a string")
                    continue
                strings = [tv]
            src_strings = v if isinstance(v, list) else [v]
            for s_src, s in zip(src_strings, strings):
                try:
                    if tags(s) != tags(s_src):
                        bad.append(f"{id_}.{k}: markup {tags(s)} != {tags(s_src)}")
                except ValueError as e:
                    bad.append(f"{id_}.{k}: markup {e}")
            for s in strings:
                if not s.strip():
                    bad.append(f"{id_}.{k}: empty")
                if FORBIDDEN.search(s):
                    bad.append(f"{id_}.{k}: Han/kana {FORBIDDEN.findall(s)[:3]}")
        if id_.startswith("attack:"):
            for k, v in f.items():
                if isinstance(t.get(k), str):
                    bad += [f"{id_}.{k}: {p}" for p in act_problems(v, t[k], k.split(".")[1])]
        tr = text_of(t)
        for term in terms_in(text_of(f), used):
            # strict: false marks everyday words that are game terms only sometimes ("the water
            # level", "experience in warfare"): in the prompt as guidance, not enforced.
            if term.get("strict", True) and term["ko"] not in tr:
                bad.append(f"{id_}: glossary {term['en']} -> {term['ko']} not used")
    return bad


PAIRS = {"이/가", "은/는", "을/를", "과/와", "으로/로", "아/야"}
PAIR_OF = {w: p for p in PAIRS for w in p.split("/")}
CODE = re.compile(r"\$(.)")
# A particle that changes with the final consonant must be a pair; 의, 에게, 도 .. attach as they are.
VARYING = re.compile(r"(이|가|은|는|을|를|과|와|으로|로|아|야)(?![가-힣])")


def act_problems(en, ko, role="room"):
    """A combat line's codes: the names it speaks of kept, no pronoun codes, particles as pairs.
    The reader's own code may stand for English "you" (it renders as 당신): $n in an attacker's line,
    $N in a victim's."""
    bad = []
    src = {c for c in CODE.findall(en)}
    out = {c for c in CODE.findall(ko)}
    if out & set("emsEMS"):
        bad.append(f"pronoun codes {sorted(out & set('emsEMS'))}")
    if out - set("nNp$emsEMS"):
        bad.append(f"unknown codes {sorted(out - set('nNp$emsEMS'))}")
    for c in "nNp":
        if c in src and c not in out:
            bad.append(f"${c} dropped")
    if src & set("nems") or role == "attacker":
        src.add("n")
    if src & set("NEMS") or role == "victim":
        src.add("N")
    if "n" in out and "n" not in src or "N" in out and "N" not in src:
        bad.append("a name the English does not speak of")
    for m in re.finditer(r"\$[nNp](\{[^}]*\}|[가-힣])?", ko):
        part = m.group(1)
        if part and part.startswith("{") and part[1:-1] not in PAIRS:
            bad.append(f"particle {part}")
        elif part and not part.startswith("{") and VARYING.match(ko[m.end() - 1:]):
            bad.append(f"particle attached: {ko[m.start():m.end() + 2]}")
    if re.search(r"(?<!\$[nNp])\{(" + "|".join(PAIRS) + r")\}", ko):
        bad.append("a particle pair after a word, not a code")
    return bad


def write_combat(order, translations):
    """ko/combat.yaml: the base file's shape (attacks, variants, outcomes, roles), only translated lines."""
    attacks = {}
    for a in yaml.safe_load(COMBAT.read_text(encoding="utf-8"))["attacks"]:
        variants = []
        for i in range(1, len(a["variants"]) + 1):
            t = translations.get(f"attack:{a['number']}:{i}", {})
            v = {}
            for o in OUTCOMES:
                lines = {r: t[f"{o}.{r}"] for r in ROLES if f"{o}.{r}" in t}
                if lines:
                    v[o] = lines
            variants.append(v)
        if any(variants):
            attacks[a["number"]] = {"number": a["number"], "name": a["name"], "variants": [
                {o: v.get(o, {}) for o in OUTCOMES} for v in variants]}
    head = ("# tbaMUD-derived (third_party/tbamud/NOTICE.md). Korean overlay for messages/combat.yaml, made by\n"
            "# tools/translate_ko.py. act() codes: $n $N names, $N{을/를} a particle (D23), $p the weapon.\n"
            "# A missing line falls back to English.\n")
    text = yaml.safe_dump({"attacks": list(attacks.values())}, allow_unicode=True, sort_keys=False, width=120)
    (LOCALE / "combat.yaml").write_text(head + text, encoding="utf-8")


def read_combat():
    path = LOCALE / "combat.yaml"
    out = {}
    for a in (read_yaml(path, {}) or {}).get("attacks", []):
        for i, v in enumerate(a["variants"], 1):
            out[f"attack:{a['number']}:{i}"] = {f"{o}.{r}": x for o in OUTCOMES for r, x in (v.get(o) or {}).items()}
    return out


# ---------------------------------------------------------------- files

def read_yaml(path, default):
    return yaml.safe_load(path.read_text(encoding="utf-8")) if path.exists() else default


class Folded(str):
    pass


def _str(d, s):
    style = ">" if len(s) > 90 else None
    return d.represent_scalar("tag:yaml.org,2002:str", str(s), style=style)


yaml.SafeDumper.add_representer(Folded, _str)


def nest(flat):
    """{"exits.north.look": [...], "extras.0.text": ...} -> the base files' shape."""
    out = {}
    for k, v in flat.items():
        v = [Folded(p) for p in v] if isinstance(v, list) else Folded(v)
        if isinstance(v, list) and len(v) == 1:
            v = v[0]
        parts = k.split(".")
        if parts[0] == "extras":
            ex = out.setdefault("extras", {})
            ex.setdefault(int(parts[1]), {})[parts[2]] = v
            continue
        cur = out
        for p in parts[:-1]:
            cur = cur.setdefault(p, {})
        cur[parts[-1]] = v
    if "extras" in out:
        n = max(out["extras"]) + 1
        out["extras"] = [out["extras"].get(i, {}) for i in range(n)]
    return out


def write_locale(zone, order, translations):
    path = LOCALE / f"{zone}.yaml"
    head = (f"# tbaMUD-derived (third_party/tbamud/NOTICE.md). Korean overlay for zone tba:{zone}, made by\n"
            f"# tools/translate_ko.py (local LLM, glossary.yaml). Same IDs and shape as the base files, only translated\n"
            f"# strings; commands and keywords stay English (D17). Missing entries fall back to English.\n")
    body = {id_: nest(translations[id_]) for id_ in order if id_ in translations}
    text = yaml.safe_dump(body, allow_unicode=True, sort_keys=False, width=100, default_flow_style=False)
    text = re.sub(r"\n(tba:)", r"\n\n\1", text)
    path.write_text(head + text, encoding="utf-8")


def flatten(nested):
    out = {}

    def walk(prefix, v):
        if isinstance(v, dict):
            for k, x in v.items():
                walk(f"{prefix}.{k}" if prefix else k, x)
        elif isinstance(v, list) and prefix == "extras":
            for i, x in enumerate(v):
                walk(f"extras.{i}", x)
        else:
            out[prefix] = [p.rstrip("\n") for p in v] if isinstance(v, list) else str(v).rstrip("\n")
    walk("", nested)
    return out


# ---------------------------------------------------------------- commands

def translate(zone, conventions, terms, log):
    src = load_zone(zone)
    state_path = LOCALE / f"{zone}.state.json"
    state = read_yaml(state_path, {}) if state_path.exists() else {}
    existing = read_combat() if zone == "combat" else {
        id_: flatten(v) for id_, v in (read_yaml(LOCALE / f"{zone}.yaml", {}) or {}).items()}
    save = write_combat if zone == "combat" else (lambda order, tr: write_locale(zone, order, tr))
    by_en = {t["en"]: t for t in terms}

    def up_to_date(id_, f):
        s = state.get(id_)
        if not s or s.get("source") != sha(f) or id_ not in existing:
            return False
        return all(en in by_en and term_hash(by_en[en]) == h for en, h in s.get("glossary", {}).items())

    todo = [id_ for id_, (_, f) in src.items() if f and not up_to_date(id_, f)]
    log(f"zone {zone}: {len(src)} entries, {len(todo)} to translate")
    t0, words = time.time(), 0
    def attempt_batch(ids):
        batch = {id_: src[id_][1] for id_ in ids}
        for attempt in range(1, TRIES + 1):
            text, used = prompt(batch, conventions, terms)
            try:
                raw, usage = ask(text, len(json.dumps(batch, ensure_ascii=False)), attempt)
                out = parse(raw)
                bad = problems(batch, out, list(used.values()))
            except (ValueError, KeyError, OSError) as e:
                bad, out = [f"{type(e).__name__}: {e}"], None
            if not bad:
                return batch, out
            log(f"  {ids[0]}.. attempt {attempt}: {bad[:3]}")
        log(f"  gave up on {ids}: {bad[:3]}")
        return batch, None

    def keep(batch, out):
        nonlocal words
        for id_ in batch:
            existing[id_] = out[id_]
            used_here = terms_in(text_of(batch[id_]), terms)
            state[id_] = {"source": sha(batch[id_]), "glossary": {t["en"]: term_hash(t) for t in used_here}}
        words += sum(len(text_of(f).split()) for f in batch.values())
        save(list(src), existing)
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=1, sort_keys=True), encoding="utf-8")

    size = 2 if zone == "combat" else BATCH  # a variant is a dozen lines; one bad line costs the batch
    for i in range(0, len(todo), size):
        ids = todo[i:i + size]
        batch, out = attempt_batch(ids)
        if out is not None:
            keep(batch, out)
        elif len(ids) > 1:
            # One bad entry should not cost the others: try each alone.
            for id_ in ids:
                one, out = attempt_batch([id_])
                if out is not None:
                    keep(one, out)
        el = time.time() - t0
        log(f"  {min(i + size, len(todo))}/{len(todo)}  {words} words in {el / 60:.1f} min ({words / max(el, 1) * 60:.0f}/min)")
    save(list(src), existing)


def check(zone):
    """Missing-translation report: fields of the base that the overlay does not translate."""
    src = load_zone(zone)
    tr = read_combat() if zone == "combat" else {
        id_: flatten(v) for id_, v in (read_yaml(LOCALE / f"{zone}.yaml", {}) or {}).items()}
    missing = []
    for id_, (_, f) in src.items():
        have = tr.get(id_, {})
        for k in f:
            if k not in have:
                missing.append(f"{id_} {k}")
    print(f"zone {zone}: {len(src)} entries, {len(missing)} fields without a Korean translation")
    for m in missing[:50]:
        print("  " + m)
    return missing


def main():
    cmd, zones = sys.argv[1], sys.argv[2:]
    if cmd == "check":
        sys.exit(1 if any([check(z) for z in zones]) else 0)
    conventions, terms = load_glossary()

    def log(msg):
        print(time.strftime("%H:%M:%S"), msg, flush=True)
    for z in zones:
        translate(z, conventions, terms, log)
        check(z)


if __name__ == "__main__":
    main()
