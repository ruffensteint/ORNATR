# ORNATR

ORNATR (formerly ScrollWorks) is a desktop carving design tool for Windows, written in Rust. Generate acanthus scrollwork from editable backbones, compose chip-carving patterns, place library leaves and buds, and export designs as SVG at physical millimetre sizes. All generation runs locally.

## Features

- **Scroll workspace:** grown sweeps and acanthus leaves on editable backbones, with Select (V), Pen (P) and Move/Transform (T) tools.
  - **Construction picker:** single scroll, parent and child, S-scroll, mirrored pair, running border, point of origin and corner, each with seeded variations.
  - **Backbones grown from other backbones:** they join like a leaf root and move with their parent. The fork can be dressed with a collar: an axil leaf lying over the crotch, or a split sheath opening along both stems.
  - **Library:** measured leaf types, terminal buds (husk, trefoil, berry cluster) and saved presets.
  - **Wrapping leaves** laid into a scroll's curl, either generated or taken from the library.
  - **Follow stem**, which bends a placed leaf along the stem it grows from.
  - **Leaf fans:** two or three leaves from one node, sized 100 / 66 / 33.
  - **Scroll vines:** a backbone can grow as a vine instead: curls seeded along it, each the largest scroll that fits, branching and never touching each other or the other backbones, clad with acanthus and fitted to a **carving surface** (whole page, plaque, oval or rectangle). Each curl can be moved, turned, resized, mirrored or deleted like a library leaf.
  - **Layers and carving guides:** over/under layering and carving guides.
  - **Root joins** (Canvas settings): *Exact* rounds the crotch where a leaf or branch grows from its stem with a true fillet (adjustable radius) and tidies tangled outlines; *Smooth* and *Classic* are the earlier styles.
- **Chip workspace:** a chip-carving pattern generator with its own preset library.
  - **Faceted engine** (the default for new patterns): each chip is drawn with the facet lines you carve to, down to its deepest point or deep line. Pick a centre (faceted star, compass petals, fan and rings, or swirl) and its count, and a square border (zigzag ribbon, arcade of fans or almond chain), or let *Generate variation* step through the combinations.
  - **Lit preview:** shows the cut wood under raking light, to judge how a pattern will read before carving.
  - **Classic engine:** the earlier generator, kept unchanged; existing chip layouts open with it.
- **Themes:** Graphite (default), Midnight, Slate, Studio, Paper and Sage.

Layouts save as `.ornatr`, the same format as the earlier browser edition; `.scrollworks` and `.json` layouts still open. Old stamped motifs are converted to grown leaves.

## Build

1. Install Rust from https://rustup.rs.
2. Run `Build ORNATR.cmd`. It writes `build-log.txt` and `ORNATR.exe`. The first build downloads libraries and takes several minutes.
3. Run `Test geometry.cmd` for the geometry tests; results go to `test-log.txt`.

On other platforms, use `cargo build --release -p ornatr` and `cargo test --release -p scroll_core`.

## Layout

- `core/` (`scroll_core`; one dependency, i_overlay, for exact polygon booleans): backbones, grown sweeps, acanthus and library leaves, buds, skeletons, wrapping leaves, joins, layering, carving guides, transforms, chip patterns and SVG export. `tests/` includes golden comparisons against the browser edition's output.
- `app/` (`ornatr`): the egui desktop program, with menus, tools, canvas, panels, themes, the chip workspace and presets.

Settings and presets are stored in `%APPDATA%\ORNATR` (copied from `%APPDATA%\ScrollWorks` on first run).

## Browser edition

The earlier TypeScript/React browser edition is retired. Its source remains in this repository's history, up to commit `e7715db`.

## License and attribution

Project-authored software is licensed under GPL-3.0-only. See LICENSE, COPYING.md, THIRD_PARTY_NOTICES.md and BRANDING.md. THIRD_PARTY_LICENSES.md lists every crate compiled into ORNATR.exe with its license text, and ships with any distributed executable. Modified distributions must meet the applicable GPL source and notice requirements. Ruffensteint branding does not imply endorsement of forks.

## Limitations

Leaf anatomy and collision handling are still being worked on. Review joins, overlaps and print scaling before carving. Scroll exports are drawing outlines, not machining toolpaths. Passing tests does not certify visual quality.
