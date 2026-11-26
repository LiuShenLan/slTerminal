# slTerminal

A Windows-native terminal emulator tuned for AI coding CLIs.

slTerminal is a single-process GPUI application: one window, one instance, dark mode only, GPU-accelerated rendering. It runs AI CLIs directly on Windows (no WSL) and treats every major AI CLI as a first-class citizen.

## Features

- **Native terminal core** — VT parsing, ConPTY, bounded scrollback with read-only search, selection, copy/paste guards, shell integration, completion and a bundled ConPTY runtime for Windows 10.
- **Projects and workspace** — one window and one instance; project switching preserves running terminals; panel groups, splitters, layout and per-view state restore by stable `ViewKey`.
- **Shared documents** — one live Document per real file across projects; Editor, Markdown and Diff share text, undo, dirty state and external-change conflicts; autosave and crash recovery with explicit reauthorization.
- **Sidebar** — activity bar, file explorer with planned file operations and recycle-bin delete, Commit view and agent global file roots.
- **AI CLI integration** — hook installation and status signals for Claude Code, Codex, OpenCode, Kimi, Pi and OhMyPi; terminal status fallback and agent capability scopes.
- **Appearance and settings** — dark mode only; built-in and custom color-only themes (Linear default), window materials, fonts and a local background image; English and Chinese UI.
- **Runtime control plane** — loopback-only Runtime v2 for discovery, snapshots, project activation events and panel/terminal operations, with tokens held in Windows Credential Manager.
- **Security** — PathPolicy authorization, audited settings writes, encrypted local backup with a strict allow-list.
- **Packaging** — reproducible local release chain: Inno Setup installer, portable zip, SHA-256 checksums and a pinned size budget.

## Requirements

- Windows 10 1809 (build 17763) or newer, x64.
- Rust toolchain pinned by `rust-toolchain.toml` (1.97.1).
- PowerShell 7 for release and packaging scripts.
- Inno Setup 6 to build the installer.

## Build

```powershell
cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook
```

The debug binary is `target/debug/slterm.exe`. Launching the GUI from an existing pwsh/cmd session blocks that shell; use the installed shortcut or `Start-Process slterm` for daily use.

For a release build:

```powershell
pwsh -File scripts/release.ps1 -Version 0.3.0
```

This produces the installer, the portable zip and `SHA256SUMS` under `dist/`. See `scripts/release-checklist.md` for the manual verification steps.

## Test

```powershell
cargo test --workspace -- --test-threads=1
cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
python scripts/check_architecture.py
python scripts/check_doc_policy.py full
python scripts/check_traceability.py
python scripts/check_brand_regress.py staged
```

Use the explicit `lib_tests` target for filtered tests; `--lib` and bare filters may bypass the intended test manifest and fail with a zero-output crash. See `.claude/docs/testing/strategy.md`.

## Documentation

- `.claude/CLAUDE.md` — project rules and development constraints (module details load from the nearest directory `CLAUDE.md`).
- `CONTEXT.md` — domain glossary.
- `.claude/docs/architecture/overview.md` — architecture, dependencies and ownership boundaries.
- `.claude/docs/testing/strategy.md` and `.claude/docs/testing/exemptions.md` — test strategy and registered exemptions.
- `.claude/docs/release/packaging.md` — release chain, size budget and compliance.
- `.claude/docs/operations/commands.md` — full command matrix.
- `release-notes/` — per-version release notes.
- `CHANGELOG.md` — bilingual changelog, bundled with the installer and portable zip.

## License

GPL-3.0. See `LICENSE` and `THIRD-PARTY-NOTICES`.
