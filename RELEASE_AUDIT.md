# Release review — 2026-09-16

GPL-3.0-only selected for project-authored software. The user confirmed ownership or redistribution permission for the original leaf sheets and three chip-carving SVG sheets on 2026-09-16, and explicitly requested that reference images not be uploaded. This is a recorded user confirmation, not an independent legal title investigation.

Reference-derived geometry remains in the software. All reference images, supplied reference SVG collections, personal designs, screenshots and existing Git history are excluded from the public snapshot. Dependency licenses retain their own terms. AI assistance was used in development; the license grants only rights held by contributors.

The allowlisted release-source directory is intended for a NEW repository with fresh history. Do not push the existing private hosting repository or alter its remote. Suggested repository name: ruffensteint-carving-studio.

Before upload: inspect the snapshot, run pnpm install --frozen-lockfile, pnpm test and pnpm build in the clean copy, and authenticate GitHub. GitHub authentication was verified with network access before publication; the earlier authentication failure was caused by restricted network access. The release copy passed 72 tests, TypeScript checking and a production build using the existing workspace dependencies. A fresh dependency installation was not performed.

THIRD_PARTY_NOTICES.md inventories the installed dependencies and their bundled root license files. Platform-specific packages are included when installed; regenerate for a changed lockfile or platform. This inventory is not a comprehensive legal certification.


# Release review: native edition, 2026-09-23

The browser edition's source files were replaced with the native Rust edition (`core/`, `app/`, the Cargo workspace and the Windows build scripts). The license, copying, branding and attribution terms are unchanged. The browser edition stays in the Git history of this public repository. No private hosting history was added.

Excluded: reference images, reference SVG collections, personal layouts and designs, screenshots, build output (`target/`, `ORNATR.exe`), build and test logs, and local settings (`%APPDATA%\ORNATR`). The golden test data in `core/tests/golden/` is generator output from synthetic test layouts and chip settings, not reference artwork. `core/src/profiles.rs` contains the same reference-derived leaf geometry that was already published in the browser edition, under the permission recorded above.

The geometry tests pass on Linux (`cargo test --release -p scroll_core`). The desktop program is built and run on Windows with `Build ORNATR.cmd`. THIRD_PARTY_NOTICES.md lists the direct Rust dependencies. A complete crate inventory should be generated before any binary is distributed.


# Release review: exact root joins, 2026-09-25

The geometry crate now depends on i_overlay 9 (MIT OR Apache-2.0) for exact polygon booleans. THIRD_PARTY_NOTICES.md lists the direct Rust dependencies; THIRD_PARTY_LICENSES.md, generated from `Cargo.lock` by `scripts/third-party-licenses.ps1`, lists every crate compiled into the Windows executable with its license text and must ship with any distributed `ORNATR.exe`.

Excluded as before: reference images and SVG collections, personal layouts and designs, screenshots, build output, logs and local settings. Unpublished leaf-model study code and development notes are also excluded.
