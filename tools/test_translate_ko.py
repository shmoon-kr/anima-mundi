"""translate_ko's term matching: case-sensitive proper nouns, terms scoped to zones.

  /home/shmoon/workspaces/anima/.venv/bin/python -m pytest tools/test_translate_ko.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import translate_ko as t  # noqa: E402


def en(found):
    return [x["en"] for x in found]


def test_a_case_sensitive_term_only_when_capitalised():
    terms = [{"en": "Faith", "ko": "신앙", "case_sensitive": True}, {"en": "balm", "ko": "연고"}]
    assert en(t.terms_in("A statue of Faith stands here.", terms)) == ["Faith"]
    assert en(t.terms_in("You have faith in a balm.", terms)) == ["balm"], "the word, not the name"
    assert en(t.terms_in("FAITH", terms)) == [], "exactly as written"


def test_a_zone_term_only_in_its_zones():
    terms = [{"en": "square", "ko": "칸", "zones": [36]}, {"en": "Midgaard", "ko": "미드가르드"},
             {"en": "Temple", "ko": "신전", "zones": ["30", "31", "32"]}]
    assert en(t.for_zone(terms, 36)) == ["square", "Midgaard"]
    assert en(t.for_zone(terms, "31")) == ["Midgaard", "Temple"], "a list: one area over several zones"
    assert "square" not in en(t.for_zone(terms, 30)), "the chessboard's 칸 is not the town's square"


def test_with_still_gates_on_the_context():
    terms = [{"en": "crown", "ko": "우듬지", "with": ["tree", "trees"]}]
    assert en(t.terms_in("The crown of the old tree.", terms)) == ["crown"]
    assert en(t.terms_in("A golden crown.", terms)) == []
