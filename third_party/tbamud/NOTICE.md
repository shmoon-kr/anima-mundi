# tbaMUD-derived material

Everything in this directory comes from, or was converted from, **tbaMUD** (https://github.com/tbamud/tbamud),
which is based on CircleMUD by Jeremy Elson and DikuMUD by Hans Henrik Staerfeldt, Katja Nyboe, Tom Madsen,
Michael Seifert and Sebastian Hammer.

**This directory is NOT covered by Anima Mundi's licence (AGPL-3.0).** It is distributed under the CircleMUD
licence and the DikuMUD licence it incorporates (copies: `LICENSE-CircleMUD.md`, `license-tbamud.txt`, taken
from the tbaMUD distribution). In short, those licences forbid using the material to make money or be
compensated in any way, and require crediting the authors. Read the full texts before any use.

What is here (docs/PHASE-1-PLAN.md):
- `content/<zone>/`: the converted world (IDs `tba:<zone>:<kind>:<vnum>`, D16) and `CONVERSION-REPORT.md`
- `tables/`: tbaMUD's number tables as values only (ability bonuses, THAC0, levels, saving throws, liquids,
  terrain costs, regeneration curves, spell syllables, a few settings), from `mundi-convert tables` (D21)
- `messages/combat.yaml`: the combat message file `lib/misc/messages`, from `mundi-convert messages`
- `locales/en/`: tbaMUD's English game messages as Fluent templates; `locales/ko/`: the Korean translations
  and their glossary (D19)
- `examples/`: the reviewed format example (the Newbie Zone)

The engine code does not copy tbaMUD code, comments or structure (D9). Tables and texts enter it only as
data from this directory. This notice is not legal advice; get proper legal review before any commercial use.
