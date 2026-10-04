"""The daily translation report, and confirming what reviews proposed.

  python tools/translate_report.py [--days 1]      # the report (zones reviewed in the last N days)
  python tools/translate_report.py confirm trail   # make a review's term binding (strict, its avoid words enforced)
  python tools/translate_report.py drop trail      # take a review's term out of the glossary

The zone reviews are locales/ko/reviews/<zone>.yaml (tools/translate_ko.py --review). Their proposed
terms come into the glossary unenforced; a person confirms or drops them here.
"""
import sys
import time
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).parent))
import translate_ko as t  # noqa: E402


def reviews(days):
    out = []
    cutoff = time.strftime("%Y-%m-%d", time.localtime(time.time() - days * 86400))
    for p in sorted(t.REVIEWS.glob("*.yaml")):
        r = yaml.safe_load(p.read_text(encoding="utf-8"))
        out.append((r["date"] >= cutoff, r))
    return sorted(out, key=lambda x: x[1]["date"])


def report(days):
    lines = [f"# Translation, {time.strftime('%Y-%m-%d')}", ""]
    all_reviews = reviews(days)
    recent = [r for new, r in all_reviews if new]
    lines.append(f"## Zones reviewed in the last {days} day(s): {len(recent)}")
    for r in recent:
        flag = f"  ** over the stop line ({t.STOP_RATE:.0%}) **" if r.get("stopped") else ""
        kind = f"full review, {len(r.get('fixed', []))} fixed" if r.get("full") else "sample"
        lines.append(f"- zone {r['zone']} ({r['date']}, {kind}): wrong {r['wrong_rate']:.0%}, awkward {r['awkward_rate']:.0%}"
                     f" of {r['n']}{flag}")
    lines += ["", "## Trend (every review, oldest first): wrong / awkward"]
    lines.append("  " + ", ".join(f"{r['zone']}{' (full)' if r.get('full') else ''} {r['wrong_rate']:.0%}/{r['awkward_rate']:.0%}"
                                  for _, r in all_reviews) or "  (none)")
    lines += ["", "## For a person: what the reviews called wrong (sample reviews; full reviews fixed theirs)"]
    for r in recent:
        if r.get("full"):
            continue
        for it in r.get("sample", []):
            if it.get("verdict") == "wrong":
                lines.append(f"- {it['id']} {it['field']}: {it['note']}")
    g = yaml.safe_load(t.GLOSSARY.read_text(encoding="utf-8"))
    cutoff = time.strftime("%Y-%m-%d", time.localtime(time.time() - days * 86400))
    added_on = lambda x: (x.get("added_by") or "").rsplit(" ", 1)[-1]
    all_pending = [x for x in g["terms"] if x.get("kind") == "review"]
    pending = [x for x in all_pending if added_on(x) >= cutoff]
    examples = [x for x in g.get("examples", []) if x.get("added_by") and added_on(x) >= cutoff]
    lines += ["", f"## Proposed by reviews in the last {days} day(s), to confirm or drop ({len(pending)} terms, "
                  f"{len(examples)} examples; {len(all_pending) - len(pending)} older terms still unconfirmed)"]
    for x in pending:
        avoid = f", not {', '.join(x.get('avoid_proposed', []))}" if x.get("avoid_proposed") else ""
        lines.append(f"- term {x['en']} -> {x['ko']}{avoid} ({x['added_by']}): {x.get('note', '')}")
    for x in examples:
        lines.append(f"- example ({x['added_by']}): {x['en']} | not {x['bad']} | but {x['good']}")
    missing = []
    for p in sorted(t.LOCALE.glob("*.yaml")):
        if p.stem.isdigit():
            src = t.load_zone(p.stem)
            tr = {i: t.flatten(v) for i, v in (t.read_yaml(p, {}) or {}).items()}
            n = sum(1 for i, (_, f) in src.items() for k in f if k not in tr.get(i, {}))
            if n:
                missing.append(f"zone {p.stem}: {n}")
    lines += ["", "## Fields without a translation: " + (", ".join(missing) if missing else "none")]
    return "\n".join(lines)


def edit_term(en, confirm):
    """On the parsed glossary (it is block YAML since the 2026-10-04 cleanup, not one JSON line a term)."""
    g = yaml.safe_load(t.GLOSSARY.read_text(encoding="utf-8"))
    terms, found = [], False
    for entry in g["terms"]:
        if entry.get("kind") == "review" and entry.get("en", "").lower() == en.lower():
            found = True
            if not confirm:
                continue
            entry.pop("strict", None)
            entry["kind"] = "term"
            if entry.get("avoid_proposed"):
                entry["avoid"] = entry.pop("avoid_proposed")
            entry["confirmed"] = time.strftime("%Y-%m-%d")
        terms.append(entry)
    if not found:
        sys.exit(f"no review term {en!r}")
    g["terms"] = terms
    t.GLOSSARY.write_text(yaml.safe_dump(g, allow_unicode=True, sort_keys=False, width=120), encoding="utf-8")
    print(("confirmed " if confirm else "dropped ") + en)


def main():
    args = sys.argv[1:]
    if args[:1] in (["confirm"], ["drop"]) and len(args) == 2:
        return edit_term(args[1], args[0] == "confirm")
    days = int(args[args.index("--days") + 1]) if "--days" in args else 1
    print(report(days))


if __name__ == "__main__":
    main()
