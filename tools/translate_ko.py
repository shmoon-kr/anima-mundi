"""Korean translation batch for converted content (D19): local LLM, glossary first, re-translate only what changed.

  python tools/translate_ko.py translate 186 30     # translate zones (resumable, skips what is up to date)
  python tools/translate_ko.py check 186 30         # list fields with no translation (missing-translation report)
  python tools/translate_ko.py translate combat     # the combat message file (messages/combat.yaml -> ko/combat.yaml)
  python tools/translate_ko.py translate --review CLAUDE 56 40    # and after each zone, a sample review (below)
  python tools/translate_ko.py translate --review CLAUDE --queue  # zones from locales/ko/queue.yaml, refilled when empty

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
- What a review found goes back in four ways (glossary.yaml): terms and conventions; `examples`, pairs of
  a bad and a good Korean line, in every prompt (a local model follows examples better than rules);
  `forbid` patterns and a term's `avoid` words, checked like Han leaks (a hit means a retry); and a
  translation memory: an English field already translated anywhere is reused, not sent again.
- Per zone (the unit a review changes the next translation by): the glossary is read again before each
  zone, so what the last review added is used; after it, with --review (a `claude` command), Claude
  reviews a random sample (REVIEW_N fields): ok / awkward / wrong. The review is kept in
  locales/ko/reviews/<zone>.yaml (the trend), its proposed terms and examples go into the glossary
  marked `added_by` (a person confirms them in the daily report: tools/translate_report.py), and a
  wrong rate above STOP_RATE stops the batch before the next zone, so bad work does not pile up overnight.
- Combat messages: one entry per attack variant, its lines keyed `die.attacker` .. `god.room`. act() codes:
  $n (the attacker; 당신 when it is the reader) and $N (the victim) stay, a particle after one is a pair in braces
  (`$N{을/를}`, D23), $p (the weapon) stays, and the pronoun codes ($e $m $s, $E $M $S) become the name again.
"""
from __future__ import annotations

import hashlib
import json
import random
import re
import subprocess
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


_RULES = None


def rules():
    """glossary.yaml `examples` (en, bad, good) and `forbid` (ko regex, why, optional en regex)."""
    global _RULES
    if _RULES is None:
        g = yaml.safe_load(GLOSSARY.read_text(encoding="utf-8"))
        _RULES = (g.get("examples", []), [dict(f, ko_re=re.compile(f["ko"]), en_re=re.compile(f["en"], re.I) if f.get("en") else None)
                                          for f in g.get("forbid", [])])
    return _RULES


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
{examples}Reply with the JSON object only."""

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
    ex = "".join(f"  {e['en']}\n    not: {e['bad']}\n    but: {e['good']}\n" for e in rules()[0])
    system = SYSTEM.format(conventions=conv, glossary=gl,
                           examples=f"- Examples of what to avoid and what to write instead:\n{ex}" if ex else "")
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
        for k, v in f.items():
            bad += [f"{id_}.{k}: {p}" for p in forbidden(v, t.get(k))]
        tr = text_of(t)
        for term in terms_in(text_of(f), used):
            for word in term.get("avoid", []):
                if word in tr:
                    bad.append(f"{id_}: {term['en']} is {term['ko']}, not {word}")
            # strict: false marks everyday words that are game terms only sometimes ("the water
            # level", "experience in warfare"): in the prompt as guidance, not enforced.
            if term.get("strict", True) and term["ko"] not in tr:
                bad.append(f"{id_}: glossary {term['en']} -> {term['ko']} not used")
    return bad


def forbidden(en, ko):
    """glossary.yaml `forbid` hits in a field (its paragraphs side by side with the English)."""
    if ko is None:
        return []
    ens = en if isinstance(en, list) else [en]
    kos = ko if isinstance(ko, list) else [ko]
    out = []
    for e, k in zip(ens, kos):
        if not isinstance(k, str):
            continue
        for f in rules()[1]:
            if f["ko_re"].search(k) and (f["en_re"] is None or f["en_re"].search(e)):
                out.append(f"{f['why']} ({f['ko_re'].search(k).group(0)})")
    return out


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

def missed_review_term(en, ko, terms):
    """A term a review added whose English is in the field and whose Korean is not: the field was
    translated before the review taught it (an update to make)."""
    if ko is None:
        return False
    e = " ".join(en) if isinstance(en, list) else en
    k = " ".join(ko) if isinstance(ko, list) else ko
    return any(t["ko"] not in k for t in terms_in(e, [t for t in terms if t.get("added_by")]))


def memory(terms=()):
    """English field → its Korean, from every zone already translated (the first one found; a field
    that a review would reject is left out)."""
    tm = {}
    for path in sorted(LOCALE.glob("*.yaml")):
        if not path.stem.isdigit():
            continue
        src = load_zone(path.stem)
        for id_, v in (read_yaml(path, {}) or {}).items():
            if id_ not in src:
                continue
            ko = flatten(v)
            for k, en in src[id_][1].items():
                if k in ko and not forbidden(en, ko[k]) and not missed_review_term(en, ko[k], terms):
                    tm.setdefault(json.dumps(en, ensure_ascii=False), ko[k])
    return tm


def translate(zone, conventions, terms, log):
    src = load_zone(zone)
    tm = memory(terms) if zone != "combat" else {}
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
        # a translation that a later review's rules reject is done again
        tr = existing[id_]
        if any(forbidden(v, tr.get(k)) for k, v in f.items()):
            return False
        if any(w in text_of(tr) for t in terms_in(text_of(f), terms) for w in t.get("avoid", [])):
            return False
        # a review taught a term after this was translated: once, with the term in view (its choice then stands)
        learned = s.get("glossary", {})
        new_terms = [t for t in terms if t.get("added_by") and t["en"] not in learned]
        if any(missed_review_term(v, tr.get(k), new_terms) for k, v in f.items()):
            return False
        return all(en in by_en and term_hash(by_en[en]) == h for en, h in s.get("glossary", {}).items())

    todo = [id_ for id_, (_, f) in src.items() if f and not up_to_date(id_, f)]
    log(f"zone {zone}: {len(src)} entries, {len(todo)} to translate, {len(tm)} fields in memory")

    def recalled(id_):
        """The entry's fields the memory has, and the rest."""
        f = src[id_][1]
        known = {k: tm[json.dumps(v, ensure_ascii=False)] for k, v in f.items() if json.dumps(v, ensure_ascii=False) in tm}
        return known, {k: v for k, v in f.items() if k not in known}
    t0, words = time.time(), 0
    def attempt_batch(ids):
        batch = {id_: recalled(id_)[1] for id_ in ids}
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
            full = src[id_][1]
            existing[id_] = {k: out[id_][k] if k in out[id_] else recalled(id_)[0][k] for k in full}
            for k, v in out[id_].items():
                tm.setdefault(json.dumps(full[k], ensure_ascii=False), v)
            used_here = terms_in(text_of(full), terms)
            state[id_] = {"source": sha(full), "glossary": {t["en"]: term_hash(t) for t in used_here}}
        words += sum(len(text_of(f).split()) for f in batch.values())
        save(list(src), existing)
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=1, sort_keys=True), encoding="utf-8")

    size = 2 if zone == "combat" else BATCH  # a variant is a dozen lines; one bad line costs the batch
    # Entries the memory covers whole need no model.
    whole = [id_ for id_ in todo if not recalled(id_)[1]]
    if whole:
        keep({id_: {} for id_ in whole}, {id_: {} for id_ in whole})
        log(f"  {len(whole)} entries from memory")
    todo = [id_ for id_ in todo if id_ not in whole]
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


REVIEW_N = 15
STOP_RATE = 0.15
REVIEWS = LOCALE / "reviews"

REVIEW_SYSTEM = """You review Korean translations of a fantasy text MUD (tbaMUD rooms, mobs and objects). The
narration is 해라체 (~다) and addresses the player as 당신. For each item, judge the Korean against the English:
- "wrong": the meaning is wrong, something is left out or added, or a word is mistranslated (e.g. arm armour
  "sleeves" as 팔찌, a waitress as 여종, "ever been in" as 들어본)
- "awkward": right but unnatural or stiff Korean (e.g. 당신은 큰길을 본다 for "You see the main street.")
- "ok"
Then propose what would stop the same mistake in the next zones: glossary terms (en, ko, words to avoid) for
mistranslated words, and examples (en, bad, good) for a style mistake. Only for mistakes you saw.
For each wrong or awkward item also give "fix": the whole corrected Korean, keeping every " / " paragraph
break and every {markup} tag exactly as in the given Korean.
Reply with JSON only: {"items": [{"n": 1, "verdict": "ok|awkward|wrong", "note": "...", "fix": "..."}],
"glossary": [{"en": "...", "ko": "...", "avoid": ["..."], "why": "..."}],
"examples": [{"en": "...", "bad": "...", "good": "..."}]}"""


def sample(zone, n=REVIEW_N):
    """Random fields of a translated zone, English beside Korean (the same draw for a zone on a day)."""
    src = load_zone(zone)
    ko = {id_: flatten(v) for id_, v in (read_yaml(LOCALE / f"{zone}.yaml", {}) or {}).items()}
    pairs = [(id_, k, v, ko[id_][k]) for id_, (_, f) in src.items() if id_ in ko for k, v in f.items() if k in ko[id_]]
    rng = random.Random(f"{zone}:{time.strftime('%Y-%m-%d')}")
    join = lambda v: " / ".join(v) if isinstance(v, list) else v
    return [{"id": i, "field": k, "en": join(en), "ko": join(kv)}
            for i, k, en, kv in rng.sample(pairs, min(n, len(pairs)))]


def review(zone, reviewer, log):
    """Claude's review of a sample of the zone, kept and applied. The wrong rate (None if no review)."""
    items = sample(zone)
    if not items:
        return None
    user = json.dumps([{"n": n, "en": it["en"], "ko": it["ko"]} for n, it in enumerate(items, 1)], ensure_ascii=False)
    argv = [*reviewer.split(), "-p", "--output-format", "text", "--tools", "", "--no-session-persistence",
            "--strict-mcp-config", "--system-prompt", REVIEW_SYSTEM]
    try:
        out = subprocess.run(argv, input=user, capture_output=True, text=True, timeout=600, check=True).stdout
        verdict = json.loads(out[out.find("{"):out.rfind("}") + 1])
    except (OSError, subprocess.SubprocessError, ValueError) as e:
        log(f"zone {zone}: review failed ({type(e).__name__}: {str(e)[:200]})")
        return None
    by_n = {v.get("n"): v for v in verdict.get("items", [])}
    for n, it in enumerate(items, 1):
        it["verdict"], it["note"] = by_n.get(n, {}).get("verdict", "?"), by_n.get(n, {}).get("note", "")
        if it["verdict"] in ("wrong", "awkward") and by_n.get(n, {}).get("fix"):
            it["fix"] = by_n[n]["fix"]
    fixed = apply_fixes(zone, items)
    wrong = sum(it["verdict"] == "wrong" for it in items) / len(items)
    awkward = sum(it["verdict"] == "awkward" for it in items) / len(items)
    added = apply_proposals(zone, verdict.get("glossary", []), verdict.get("examples", []))
    REVIEWS.mkdir(exist_ok=True)
    record = {"zone": str(zone), "date": time.strftime("%Y-%m-%d %H:%M"), "reviewer": "claude", "n": len(items),
              "wrong_rate": round(wrong, 3), "awkward_rate": round(awkward, 3), "stopped": wrong > STOP_RATE,
              "added_to_glossary": added, "fixed": fixed, "sample": items}
    (REVIEWS / f"{zone}.yaml").write_text(yaml.safe_dump(record, allow_unicode=True, sort_keys=False, width=120),
                                          encoding="utf-8")
    log(f"zone {zone}: review of {len(items)} fields: wrong {wrong:.0%}, awkward {awkward:.0%}, "
        f"{len(fixed)} fixed, {len(added)} added to the glossary")
    return wrong


def apply_fixes(zone, items):
    """The reviewer's corrected Korean for the sample's wrong and awkward fields, each only if it passes
    the checks every translation passes (same paragraphs and markup, no Han, nothing forbidden)."""
    src = load_zone(zone)
    path = LOCALE / f"{zone}.yaml"
    tr = {i: flatten(v) for i, v in (read_yaml(path, {}) or {}).items()}
    done = []
    for it in items:
        fix = it.get("fix")
        if not fix or it["id"] not in tr:
            continue
        en = src[it["id"]][1][it["field"]]
        new = [p.strip() for p in fix.split(" / ")] if isinstance(en, list) else fix.strip()
        if problems({it["id"]: {it["field"]: en}}, {it["id"]: {it["field"]: new}}, []):
            continue
        tr[it["id"]][it["field"]] = new
        done.append(f"{it['id']} {it['field']}")
    if done:
        write_locale(zone, list(src), tr)
    return done


def apply_proposals(zone, terms, examples):
    """A review's proposed terms and examples into the glossary, marked so a person confirms them. A term
    already there is left alone. Forbidden patterns are not taken: a bad one would block good work.
    Terms come in unenforced (strict false, avoid_proposed); confirming one makes it binding."""
    g = yaml.safe_load(GLOSSARY.read_text(encoding="utf-8"))
    have = {t["en"].lower() for t in g["terms"]}
    seen = {e["en"].lower() for e in g.get("examples", [])}
    mark = f"review {zone} {time.strftime('%Y-%m-%d')}"
    text = GLOSSARY.read_text(encoding="utf-8").rstrip("\n") + "\n"
    added = []
    for t in terms:
        if not t.get("en") or not t.get("ko") or t["en"].lower() in have:
            continue
        # guidance in the prompt until a person confirms it: not enforced (strict false), and its words
        # to avoid only proposed (a review's "trail -> 오솔길, not 산길" would reject a mountain trail)
        entry = {"en": t["en"], "ko": t["ko"], "kind": "review", "strict": False, "added_by": mark}
        if t.get("avoid"):
            entry["avoid_proposed"] = list(t["avoid"])
        if t.get("why"):
            entry["note"] = t["why"]
        text += "  - " + json.dumps(entry, ensure_ascii=False) + "\n"
        have.add(t["en"].lower())
        added.append(f"term {t['en']} -> {t['ko']}")
    fresh = [e for e in examples if e.get("en") and e.get("bad") and e.get("good") and e["en"].lower() not in seen]
    if fresh:
        block = "".join("  - " + json.dumps({"en": e["en"], "bad": e["bad"], "good": e["good"], "added_by": mark},
                                            ensure_ascii=False) + "\n" for e in fresh)
        i = text.find("\nforbid:")
        if i >= 0:
            text = text[:i].rstrip("\n") + "\n" + block + text[i:]
            added += [f"example {e['en']}" for e in fresh]
    yaml.safe_load(text)                      # still a glossary
    GLOSSARY.write_text(text, encoding="utf-8")
    return added


QUEUE = LOCALE / "queue.yaml"
REFILL = 5


def untranslated_zones():
    """Zones worth translating that are not done yet: content and a level range (not builders', gods' or
    examples'), lowest levels first; a zone with missing fields counts as not done."""
    out = []
    for d in sorted(CONTENT.iterdir(), key=lambda d: d.name):
        if not d.name.isdigit() or not (d / "zone.yaml").exists():
            continue
        lv = (yaml.safe_load((d / "zone.yaml").read_text(encoding="utf-8")) or {}).get("levels") or {}
        if not lv.get("max"):
            continue
        src = load_zone(d.name)
        if not any(f for _, f in src.values()):
            continue
        tr = {i: flatten(v) for i, v in (read_yaml(LOCALE / f"{d.name}.yaml", {}) or {}).items()}
        if all(k in tr.get(i, {}) for i, (_, f) in src.items() for k in f):
            continue
        out.append((lv.get("min", 0), lv.get("max", 0), int(d.name), d.name))
    return [z for *_, z in sorted(out)]


def next_from_queue(log):
    """The next zone of the queue (taken off it); an empty queue is filled first. None: nothing left."""
    q = (read_yaml(QUEUE, {}) or {}).get("zones", [])
    if not q:
        q = untranslated_zones()[:REFILL]
        if q:
            log(f"queue empty: filled with {q}")
    if not q:
        return None
    zone, rest = str(q[0]), q[1:]
    QUEUE.write_text("# Zones to translate next, first first (tools/translate_ko.py --queue takes one at a time and\n"
                     "# fills it when empty). Edit freely: it is read again before each zone.\n"
                     + yaml.safe_dump({"zones": rest}, allow_unicode=True), encoding="utf-8")
    return zone


def main():
    args = sys.argv[1:]
    cmd = args.pop(0)
    reviewer = None
    queue = "--queue" in args
    if queue:
        args.remove("--queue")
    if "--review" in args:
        i = args.index("--review")
        reviewer = args[i + 1]
        del args[i:i + 2]
    zones = args
    if cmd == "check":
        sys.exit(1 if any([check(z) for z in zones]) else 0)

    def log(msg):
        print(time.strftime("%H:%M:%S"), msg, flush=True)
    global _RULES

    def work():
        yield from zones
        while queue:
            z = next_from_queue(log)
            if z is None:
                log("nothing left to translate")
                return
            yield z
    for z in work():
        conventions, terms = load_glossary()       # each zone starts with what the last review added
        _RULES = None
        translate(z, conventions, terms, log)
        check(z)
        if reviewer and z != "combat":
            before = len(load_glossary()[1])
            wrong = review(z, reviewer, log)
            if wrong is not None and wrong > STOP_RATE:
                log(f"zone {z}: {wrong:.0%} of the sample is wrong (> {STOP_RATE:.0%}): stopping before the next zone")
                sys.exit(3)
            conventions, terms = load_glossary()
            _RULES = None
            if len(terms) > before:
                # the review taught terms: every zone translated so far gets the fields that missed them
                for done in sorted(p.stem for p in LOCALE.glob("*.yaml") if p.stem.isdigit()):
                    translate(done, conventions, terms, log)


if __name__ == "__main__":
    main()
