# slTerminal

A Windows-native terminal emulator tuned for AI coding CLIs.

slTerminal is a single-process GPUI application: one window, one instance, dark mode only, GPU-accelerated rendering. It runs AI CLIs directly on Windows (no WSL) and treats every major AI CLI as a first-class citizen.

## Features

- **Native terminal core** — VT parsing, ConPTY, scrollback, selection, copy/paste, shell integration and a bundled ConPTY fallback for Windows 10.
- **Tabs and split panes** — terminal tabs, grid splits, pane zoom, drag-to-dock, session restore for terminal tabs.
- **AI CLI integration** — screen detection rules for AI CLI agents, hook installation for Claude Code / Kimi / Codex / Pi / OpenCode, agent status projection, and Claude Code and Codex session history.
- **Files and editing** — file explorer, code editor, Markdown and image tabs, read-only dual-pane diff, git gutter, and Markdown math rendering.
- **Settings and themes** — settings center with application/appearance/profiles/interaction/advanced/localization/reset and an agent group; built-in dark themes (Linear default); English and Chinese UI.
- **System integration** — tray icon, Windows toast notifications, taskbar progress, launch at login, window materials and wallpaper, update checks and downloads.
- **Security** — credentials stay in Windows Credential Manager, audited settings writes, encrypted local backup with a strict allow-list.
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
