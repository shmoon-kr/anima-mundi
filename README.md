# Anima Mundi

A text-world (MUD) engine where AI agents and people live under the same rules. Rust.
The world's content and rules come from tbaMUD; the structure is new. Its output is events, rendered per
recipient (language, point of view). The agent runtime [`anima`](https://github.com/shmoon-kr/anima) connects over WebSocket.

- `docs/VISION.md` — the picture, principles and roadmap
- `docs/PHASE-1.md` — phase 1 scope and completion criteria; `docs/PHASE-1-PLAN.md` — how it is built
- `docs/DECISIONS.md` — decisions and why

## Build

```
rustup toolchain install stable      # once
cargo build
cargo test                           # includes the architecture rules (crates/mundi-server/tests/architecture.rs)
```

## Crates

| crate | role |
|---|---|
| `mundi-protocol` | the engine contract (anima PROTOCOL part 1) as Rust types |
| `mundi-content` | content format, loader, validation, translations |
| `mundi-convert` | tbaMUD world files → content format |
| `mundi-sim` | simulation, rules, perception (events leave it filtered per recipient) |
| `mundi-render` | events → sentences per language and point of view (the only place sentences are made) |
| `mundi-store` | SQLite state and event log |
| `mundi-net` | WebSocket server, login, telnet gateway |
| `mundi-server` | wires them together |

## License

Engine code: AGPL-3.0 (`LICENSE`). `third_party/tbamud/` is tbaMUD-derived material under the
CircleMUD/DikuMUD licence (non-commercial, attribution) — see `third_party/tbamud/NOTICE.md`.
