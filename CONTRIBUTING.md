# Contributing

Changes and ports are welcome. Contributions to the software must be offered under GPL-3.0-only, with existing notices preserved. Contributors retain their rights; no copyright assignment is requested.

Explain the source and license of any copied code or artwork. Do not submit traced or adapted reference designs without documenting permission. Disclose AI assistance and review generated changes before submitting them.

Install Rust from https://rustup.rs. Run `cargo test --release -p scroll_core` (or `Test geometry.cmd`) and `cargo build --release -p ornatr`. For geometry changes, compare exported SVGs as well as running the tests. Passing tests does not establish visual quality or carving safety.

For a port, start in `core/src`:

- `geometry.rs`, `contour.rs` and `outline.rs`: curves, leaf contours and outline union.
- `spiral.rs`, `composed.rs` and `growth.rs`: grown scrolls.
- `shoots.rs` and `profiles.rs`: placed and library leaves.
- `model.rs`: layouts.
- `chip.rs`: chip patterns.

Keep seeded behaviour and physical SVG units, and use the tests as behavioural examples. Reference-based library shapes (`profiles.rs`) need the provenance review described in RELEASE_AUDIT.md.
