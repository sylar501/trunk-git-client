**TRUNK**

Claude Code Handoff Document

*Files to upload · CLAUDE.md · Starting prompt \| June 2026*

**1. Files to Feed to Claude Code**

Upload these files when starting your Claude Code session. Required files (green) must be present. Recommended files (gray) significantly improve output quality.

  -------- --------------------------------------- ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- ---------
  **\#**   **File**                                **What Claude Code uses it for**                                                                                                                                              **Req**

  **1**    **CLAUDE.md**                           Project constitution --- read by Claude Code at the start of every session. Contains the WHY/WHAT/HOW of Trunk and pointers to all other docs. See §2 for the full content.   **✓**

  **2**    **trunk-requirements.docx**             Full PRD --- 25 sections covering every feature, screen, interaction model, and design decision. The authoritative spec for all implementation work.                          **✓**

  **3**    **trunk-claude-design-briefing.docx**   Component library briefing --- dark theme palette (all hex values), button colour semantics, typography, spacing conventions, and the full dark.css token reference.          **✓**

  **4**    **\[component-library\].pdf or .png**   The component library output from Claude Design --- shows every UI element in all states. Claude Code references this for exact visual implementation.                        **✓**

  **5**    **screenshot-1-main-graph.png**         Main graph view with commit detail overlay --- shows sidebar, graph canvas, branch pills, diff lines, terminal drawer, overlay panel.                                         **---**

  **6**    **screenshot-2-staging.png**            Staging view --- shows all button colours in context, file list, hunk staging, commit panel.                                                                                  **---**

  **7**    **screenshot-3-preferences.png**        Preferences screen --- shows two-panel layout, all form control types, info boxes, footer.                                                                                    **---**

  **8**    **screenshot-4-palette-dialogs.png**    Command palette + branch dialogs --- shows dialog chrome, input states, warning boxes.                                                                                        **---**
  -------- --------------------------------------- ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- ---------

Note: the four screenshots (files 5--8) are the ones rendered at the end of our design session. Take screenshots of those four mockups and include them. They are not strictly required but give Claude Code the clearest visual reference for the dark theme implementation.

**2. CLAUDE.md --- Project Constitution**

Create this file at the root of the Trunk repository as CLAUDE.md. Keep it lean --- it is loaded into every session. It points to the detailed docs rather than inlining everything.

+------------------------------------------------------------------------------------------------------+
| \# Trunk                                                                                             |
|                                                                                                      |
| \## What this is                                                                                     |
|                                                                                                      |
| Trunk is a native cross-platform Git GUI client. Rust backend + Tauri 2.x frontend.                  |
|                                                                                                      |
| Free and open-source. Target: individual devs, teams, and engineering orgs.                          |
|                                                                                                      |
| Primary differentiator: raw performance on large repositories.                                       |
|                                                                                                      |
| \## Stack                                                                                            |
|                                                                                                      |
| \- Backend: Rust, Tauri 2.x, libgit2 (via git2-rs), portable-pty                                     |
|                                                                                                      |
| \- Frontend: WebView (HTML/CSS/JS inside Tauri), no framework --- vanilla JS + CSS custom properties |
|                                                                                                      |
| \- Config: TOML (.trunk workspace files)                                                             |
|                                                                                                      |
| \- Platforms: macOS 13+, Windows 10+, Linux (Debian/Ubuntu/Fedora)                                   |
|                                                                                                      |
| \- Credential storage: OS-native (Keychain / DPAPI / Secret Service)                                 |
|                                                                                                      |
| \## Key docs (read when relevant, not upfront)                                                       |
|                                                                                                      |
| \- Full feature spec: trunk-requirements.docx (25 sections, §1--§25)                                 |
|                                                                                                      |
| \- Component library & dark theme palette: trunk-claude-design-briefing.docx                         |
|                                                                                                      |
| \- UI screens: see screenshot-\*.png files                                                           |
|                                                                                                      |
| \## Dark theme --- always use these tokens                                                           |
|                                                                                                      |
| Never use arbitrary hex colours. Always use CSS custom properties from dark.css:                     |
|                                                                                                      |
| \--bg-base: #0D1117 \--bg-primary: #161B22 \--bg-secondary: #1C2128                                  |
|                                                                                                      |
| \--bg-tertiary: #22272E \--bg-raised: #2D333B                                                        |
|                                                                                                      |
| \--border-subtle: #30363D \--border-default: #444C56                                                 |
|                                                                                                      |
| \--text-primary: #CDD9E5 \--text-secondary: #768390                                                  |
|                                                                                                      |
| \--text-tertiary: #444C56 \--text-dim: #2D333B                                                       |
|                                                                                                      |
| \--blue: #539BF5 \--blue-bg: #1C2A3A                                                                 |
|                                                                                                      |
| \--green: #57AB5A \--green-bg: #1B2A1F                                                               |
|                                                                                                      |
| \--red: #E5534B \--red-bg: #2D1B1B                                                                   |
|                                                                                                      |
| \--amber: #C69026 \--amber-bg: #2D2415                                                               |
|                                                                                                      |
| \--purple: #986EE2 \--purple-bg: #261E3A                                                             |
|                                                                                                      |
| \--lane-1:#539BF5 \--lane-2:#57AB5A \--lane-3:#C69026                                                |
|                                                                                                      |
| \--lane-4:#986EE2 \--lane-5:#D4537E \--lane-6:#39C5CF \--lane-7:#D08444                              |
|                                                                                                      |
| \## Button colour rule (mandatory)                                                                   |
|                                                                                                      |
| Blue (#185FA5 solid) = primary constructive. Green (#3B6D11 solid) = additive/stage.                 |
|                                                                                                      |
| Red (\--red-bg tinted) = destructive. Amber (\--amber-bg tinted) = caution/reversible.               |
|                                                                                                      |
| Neutral (\--bg-tertiary) = navigation. Disabled = always gray regardless of semantic colour.         |
|                                                                                                      |
| \## Layout                                                                                           |
|                                                                                                      |
| Two-panel: 156px fixed sidebar + full-width graph canvas.                                            |
|                                                                                                      |
| Other views (staging, conflict resolver, rebase, stash, tags, prefs) replace the canvas.             |
|                                                                                                      |
| Commit detail is a right-side overlay (264px), does not shrink the graph.                            |
|                                                                                                      |
| Terminal is a bottom drawer toggled with Cmd+\` --- overlays from below.                             |
|                                                                                                      |
| \## PRD section index (for quick reference)                                                          |
|                                                                                                      |
| §4 Layout §5 Button colours §6 Keyboard §7 Commit graph §8 Staging                                   |
|                                                                                                      |
| §9 Conflict resolver §10 Command palette §11 Stash manager                                           |
|                                                                                                      |
| §12 Push/Fetch/Pull §13 Branch dialogs §14 Tag manager                                               |
|                                                                                                      |
| §15 Welcome/Workspace/Clone §16 Interactive rebase §17 Theme & palette                               |
|                                                                                                      |
| §18 Remote management §19 Preferences §20--§25 Technical/Plugins/Checklist                           |
|                                                                                                      |
| \## Build commands (fill in after project scaffold)                                                  |
|                                                                                                      |
| \# cargo build                                                                                       |
|                                                                                                      |
| \# cargo tauri dev                                                                                   |
|                                                                                                      |
| \# cargo test                                                                                        |
+------------------------------------------------------------------------------------------------------+

**3. Starting Prompt for Claude Code**

Use this as your first message in Claude Code. It instructs Claude Code to read all the context before writing a single line of code, then produce a project scaffold and SPEC.md.

Paste it verbatim after uploading all the files listed in §1.

+-----------------------------------------------------------------------------------+
| Read CLAUDE.md first, then read trunk-requirements.docx in full, then read        |
|                                                                                   |
| trunk-claude-design-briefing.docx. Also examine all screenshot-\*.png files and   |
|                                                                                   |
| the component library file. Do not write any code yet.                            |
|                                                                                   |
| Once you have read everything, do the following in order:                         |
|                                                                                   |
| 1\. CONFIRM UNDERSTANDING                                                         |
|                                                                                   |
| Summarise in 10 bullet points what Trunk is, its stack, its two operating modes   |
|                                                                                   |
| (Repository vs Workspace), and the five most important UI design constraints.     |
|                                                                                   |
| 2\. SCAFFOLD THE PROJECT                                                          |
|                                                                                   |
| Create a Rust + Tauri 2.x project structure for Trunk. The structure should       |
|                                                                                   |
| separate Rust backend (src-tauri/) from the frontend (src/). Include:             |
|                                                                                   |
| \- src-tauri/src/: git operations (libgit2), workspace management, terminal (pty) |
|                                                                                   |
| \- src/: HTML/CSS/JS frontend --- one file per screen/view                        |
|                                                                                   |
| \- src/styles/: dark.css (with all tokens from CLAUDE.md), components.css         |
|                                                                                   |
| \- src/components/: shared UI components matching the component library           |
|                                                                                   |
| \- Cargo.toml with correct dependencies (tauri 2.x, git2, portable-pty, serde)    |
|                                                                                   |
| \- tauri.conf.json with correct window config                                     |
|                                                                                   |
| \- .trunk/ for workspace files                                                    |
|                                                                                   |
| 3\. WRITE SPEC.md                                                                 |
|                                                                                   |
| Before implementing any feature, write SPEC.md at the project root.               |
|                                                                                   |
| This is the implementation checklist. For each screen/feature in the PRD          |
|                                                                                   |
| (§7 through §19), write:                                                          |
|                                                                                   |
| \- Feature name and PRD section reference                                         |
|                                                                                   |
| \- Frontend files involved                                                        |
|                                                                                   |
| \- Backend Tauri commands required                                                |
|                                                                                   |
| \- Dependencies between features (e.g. staging depends on graph view existing)    |
|                                                                                   |
| \- Acceptance criteria (what \"done\" looks like)                                 |
|                                                                                   |
| Order features by dependency --- scaffold and global styles first, welcome screen |
|                                                                                   |
| second, main graph view third (it is the anchor), then all other screens.         |
|                                                                                   |
| 4\. IMPLEMENT: dark.css                                                           |
|                                                                                   |
| Create src/styles/dark.css with all tokens from CLAUDE.md §Dark theme.            |
|                                                                                   |
| This file must exist before any other frontend work begins.                       |
|                                                                                   |
| 5\. IMPLEMENT: CLAUDE.md build commands                                           |
|                                                                                   |
| Fill in the Build commands section of CLAUDE.md with the actual commands          |
|                                                                                   |
| from the scaffolded project (cargo build, cargo tauri dev, cargo test, etc.).     |
|                                                                                   |
| After steps 1--5 are complete, stop and show me:                                  |
|                                                                                   |
| \- The full project directory tree                                                |
|                                                                                   |
| \- SPEC.md                                                                        |
|                                                                                   |
| \- src/styles/dark.css                                                            |
|                                                                                   |
| Do not proceed to implement any screens until I confirm the scaffold and SPEC.    |
|                                                                                   |
| We will implement one screen per session from that point forward.                 |
+-----------------------------------------------------------------------------------+

**4. Recommended Session Workflow**

Claude Code works best one feature per session. Follow this pattern for each screen after the initial scaffold:

**Session structure**

-   Start a fresh Claude Code session for each screen or major feature.

-   Always begin with: \"Read CLAUDE.md and SPEC.md. We are implementing \[screen name\] (PRD §N). Read that section of trunk-requirements.docx before writing any code.\"

-   Ask Claude Code to write a plan first (use Plan Mode: Shift+Tab twice). Review it before approving.

-   After implementation, ask Claude Code to update SPEC.md to mark the feature as done.

-   Commit after each working screen. Never work on multiple screens in one session.

**Suggested implementation order**

Follow SPEC.md, but this is the recommended dependency order:

-   Session 1: Scaffold + dark.css + SPEC.md (covered by starting prompt above)

-   Session 2: Welcome screen --- all three entry points (Open / Clone / Create workspace)

-   Session 3: Main graph view --- sidebar + graph canvas + commit rows (no overlay yet)

-   Session 4: Commit detail overlay + diff view

-   Session 5: Staging view (depends on diff view being done)

-   Session 6: Branch dialogs (create, switch, rename, delete)

-   Session 7: Push / Fetch / Pull dialogs

-   Session 8: Command palette

-   Session 9: Conflict resolver

-   Session 10: Interactive rebase takeover

-   Session 11: Stash manager

-   Session 12: Tag manager

-   Session 13: Remote management (sidebar contextual --- no separate screen)

-   Session 14: Preferences screen

-   Session 15: Empty workspace state + workspace mode plumbing

-   Session 16: Clone dialog + nested-repo detection

-   Session 17--N: Rust backend --- git operations, credential store, auto-fetch, terminal pty

**Context management tips**

-   Run /compact if context is filling up mid-session. Include: \"Keep all dark theme token names, the current screen spec, and file paths.\"

-   Keep CLAUDE.md under 150 lines. If you add to it, remove something else.

-   Point Claude Code at PRD sections by number rather than pasting content: \"Read §13 of trunk-requirements.docx.\"

-   If Claude Code goes off track, use /rewind immediately. Do not try to fix a bad path.

-   Screenshots (files 5--8) are your fastest way to course-correct visual implementation --- show Claude Code a screenshot and ask \"does your output match this?\"

**5. Key Constraints Claude Code Must Never Violate**

These are the highest-priority rules from the PRD. Add them to CLAUDE.md if you find Claude Code forgetting them.

-   Never use arbitrary hex colours. Every colour must be a CSS custom property from dark.css.

-   Button colour follows the semantic system (§5 of PRD): blue = primary, green = additive, red = destructive, amber = caution, neutral = navigation.

-   The graph canvas is never shrunk --- the commit detail overlay slides over it, not beside it.

-   Staging, conflict resolver, interactive rebase, stash manager, and tag manager replace the graph canvas --- they are full-screen takeovers.

-   Escape always exits the current view and returns to graph. If focus is in a text field, first Escape defocuses the field; second Escape exits the view.

-   The terminal drawer overlays from the bottom --- it never pushes content up.

-   The delete branch confirmation for unmerged branches requires an explicit \"I understand this work may be lost\" checkbox before the Force delete button activates.

-   Repository mode: no Repositories sidebar section, no + button, no clone/add commands.

-   Workspace mode: Repositories section always present with + button.

-   All dialogs are dismissed with Escape or Cancel. No dialog traps.

*Trunk Claude Code Handoff --- June 2026*
