# Third-party dependency notices

ORNATR's geometry crate (`core/`, `scroll_core`) depends directly on:

| Crate | Version | Declared license |
|---|---|---|
| i_overlay | 9.0.0 | MIT OR Apache-2.0 |

The desktop program (`app/`) depends directly on:

| Crate | Version | Declared license |
|---|---|---|
| eframe (egui) | 0.29.1 | MIT OR Apache-2.0 |
| rfd | 0.15.4 | MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| web-time | 1.1.0 | MIT OR Apache-2.0 |

Cargo downloads these and their transitive dependencies (pinned in `Cargo.lock`) when you build. None of them are included in this repository. Each keeps its own license and notices, which ship with its source in the Cargo registry.

A compiled `ORNATR.exe` contains all of these crates. `THIRD_PARTY_LICENSES.md` lists every crate built into the Windows program with its license text, and must be shipped with the executable. Regenerate it with `scripts/third-party-licenses.ps1` whenever `Cargo.lock` changes. Neither file is a comprehensive legal certification.
