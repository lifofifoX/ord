# ord Codebase Guide (ordnet Fork)

## Purpose
- Give humans and LLMs a quick map of this repository.
- Describe where `ordnet` differs from upstream `ord`.
- For general project overview, installation, and user-facing usage, see `@README.md`.
- Keep this file short; update it when architecture or major fork behavior changes.

## Upstream Reference
- Upstream repository/branch: [ordinals/ord `master`](https://github.com/ordinals/ord/tree/master)
- Local tracking branch: `origin/master`
- Local fork working branch: `ordnet`

## Quick Code Map
- `src/`: primary Rust codebase.
- `src/index.rs` and `src/index/`: index schema, storage, and indexing pipeline.
- `src/subcommand/`: CLI and server subcommands.
- `tests/`: integration tests.
- `.github/workflows/ci.yaml`: CI checks.

## Common Local Commands
- `cargo test --all`
- `cargo fmt --all`

## ordnet Additions (Top Level)
- BRC-20 exclusion during inscription indexing.
  - Code: `src/index.rs`, `src/inscriptions/inscription.rs`, `src/index/updater/`
  - Tests: `src/index/brc20_filter_tests.rs`, `src/inscriptions/inscription_brc20_tests.rs`
- Persisted inscription transfer history and query APIs.
  - Code: `src/index/transfer_event.rs`, `src/index/transfer_history.rs`, `src/index/updater/transfer_history_indexer.rs`
  - Tests: `src/index/transfer_history_tests.rs`
- Small fork-supporting updates.
  - Code: `src/subcommand/server.rs`, `.github/workflows/ci.yaml`

## Compatibility Note
- Indexes built with different BRC-20 exclusion modes are not compatible.
- If switching between upstream `ord` behavior and `ordnet` behavior, rebuild or use separate index paths.
