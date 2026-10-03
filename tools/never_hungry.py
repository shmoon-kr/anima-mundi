"""Test characters that never get hungry or thirsty: tbaMUD's immortal condition value -1 ("no
change", limits.c:309; MECHANICS §6.1), set in a character's save. No rule changes.

For S6's anima runs on a local Mundi: a level-1 party without money for food starved, and hunger
quarters regeneration (§5.2). Turn it OFF for S7's comparison runs with tbaMUD, where hunger is part
of what is compared (camp time, growth). Where it is on: docs/PHASE-1-PLAN.md S6.

  python tools/never_hungry.py <mundi.db> on  Vallen Lil ...   # full, thirst, drunk = -1
  python tools/never_hungry.py <mundi.db> off Vallen Lil ...   # back to a new character's 24, 24, 0
  python tools/never_hungry.py <mundi.db> show

The server must be stopped: it keeps the saves while it runs and writes them back.
"""
import json
import sqlite3
import sys

NEVER = {"drunk": -1, "full": -1, "thirst": -1}
NEW = {"drunk": 0, "full": 24, "thirst": 24}       # class.c:1471-1473


def main(db, cmd, names):
    con = sqlite3.connect(db)
    rows = con.execute("select name, data from characters").fetchall()
    wanted = {n.lower() for n in names}
    for name, data in rows:
        save = json.loads(data) if data else None
        if save is None:
            continue
        if cmd in ("on", "off") and name in wanted:
            save["conditions"] = NEVER if cmd == "on" else NEW
            con.execute("update characters set data = ? where name = ?", (json.dumps(save), name))
        print(f"{name:10} {save.get('conditions')}")
    con.commit()
    missing = wanted - {n for n, _ in rows}
    if missing:
        sys.exit(f"no such characters: {sorted(missing)}")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[2] not in ("on", "off", "show"):
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2], sys.argv[3:])
