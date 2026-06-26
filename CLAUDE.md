# Trunk

## What this is
Trunk is a native cross-platform Git GUI client. Rust backend + Tauri 2.x frontend.
Free and open-source. Target: individual devs, teams, and engineering orgs.
Primary differentiator: raw performance on large repositories.

## Stack
- Backend: Rust, Tauri 2.x, libgit2 (via git2-rs), portable-pty
- Frontend: WebView (HTML/CSS/JS inside Tauri), no framework — vanilla JS + CSS custom properties
- Config: TOML (.trunk workspace files)
- Platforms: macOS 13+, Windows 10+, Linux (Debian/Ubuntu/Fedora)
- Credential storage: OS-native (Keychain / DPAPI / Secret Service)

## Key docs (read when relevant, not upfront)
- Full feature spec: trunk-requirements-3.docx (PRD v2.0, "Final" — 25 sections, §1–§25).
  trunk-requirements.docx / -2.docx are an earlier v1.0 draft with §10–16 and §19–20
  unwritten — do not use them.
- Component library & dark theme palette: trunk-claude-design-briefing.docx, plus the
  single-page visual reference in "Trunk Component Library.pdf"
- UI screens: see screenshot_*.html files in docs/

## Dark theme — always use these tokens
Never use arbitrary hex colours. Always use CSS custom properties from dark.css:
  --bg-base: #0D1117  --bg-primary: #161B22  --bg-secondary: #1C2128
  --bg-tertiary: #22272E  --bg-raised: #2D333B
  --border-subtle: #30363D  --border-default: #444C56
  --text-primary: #CDD9E5  --text-secondary: #768390
  --text-tertiary: #444C56  --text-dim: #2D333B
  --blue: #539BF5  --blue-bg: #1C2A3A
  --green: #57AB5A  --green-bg: #1B2A1F
  --red: #E5534B  --red-bg: #2D1B1B
  --amber: #C69026  --amber-bg: #2D2415
  --purple: #986EE2  --purple-bg: #261E3A
  --lane-1:#539BF5  --lane-2:#57AB5A  --lane-3:#C69026
  --lane-4:#986EE2  --lane-5:#D4537E  --lane-6:#39C5CF  --lane-7:#D08444

## Button colour rule (mandatory)
Blue (#185FA5 solid) = primary constructive. Green (#3B6D11 solid) = additive/stage.
Red (--red-bg tinted) = destructive. Amber (--amber-bg tinted) = caution/reversible.
Neutral (--bg-tertiary) = navigation. Disabled = always gray regardless of semantic colour.

## Layout
Two-panel: 156px fixed sidebar + full-width graph canvas.
Other views (staging, conflict resolver, rebase, stash, tags, prefs) replace the canvas.
Commit detail is a right-side overlay (264px), does not shrink the graph.
Terminal is a bottom drawer toggled with Cmd+` — overlays from below.

## PRD section index (for quick reference)
§4 Layout  §5 Button colours  §6 Keyboard  §7 Commit graph  §8 Staging
§9 Conflict resolver  §10 Command palette  §11 Stash manager
§12 Push/Fetch/Pull  §13 Branch dialogs  §14 Tag manager
§15 Welcome/Workspace/Clone  §16 Interactive rebase  §17 Theme & palette
§18 Remote management  §19 Preferences  §20–§25 Technical/Plugins/Checklist

## Build commands
cargo build --manifest-path src-tauri/Cargo.toml
cargo tauri dev
cargo test --manifest-path src-tauri/Cargo.toml