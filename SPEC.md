# Trunk — Implementation Spec

Feature-by-feature checklist, ordered by dependency. Each entry cites the PRD section in
`docs/trunk-requirements-3.md` (v2.0 — the authoritative spec;). One screen/feature per Claude Code session, per the recommended workflow in `docs/trunk-claudecode-handoff.md`.

Mark an entry `[x]` only when its acceptance criteria are fully met and the working tree
for that session has been committed.

---

## 0. [x] Scaffold + dark.css + SPEC.md — Session 1

- **Frontend**: `src/styles/dark.css`, `src/styles/components.css`, `src/index.html`,
  `src/welcome.html`, `src/staging.html`, `src/preferences.html`, `src/js/app.js`, `src/components/*.js`
- **Backend**: `src-tauri/` crate scaffold, `commands.rs` stub surface, `git/`, `workspace/`, `terminal/` module stubs
- **Dependencies**: none
- **Acceptance criteria**: `cargo build` succeeds; `dark.css` contains every token in §17.1
  plus the button-specific tokens in §5; `SPEC.md` exists; CLAUDE.md build commands filled in.

---

## 1. [x] Welcome screen — §15.1

- **Frontend**: `src/welcome.html`, `src/components/dialog.js` (clone dialog, §15.5.1),
  `src/components/toast.js`, `src/components/context-menu.js`
- **Backend**: `open_repository`, `open_workspace`, `list_recent`, `remove_recent`,
  `create_workspace` (generalized — also used for "Open as workspace" below),
  `detect_nested_repos` (§15.2), `clone_repository` (§15.5.1, real `git2` clone with
  streamed progress events — not deferred to item 15)
- **Dependencies**: scaffold (0)
- **Acceptance criteria**:
  - Open Repository (blue) and Clone Repository (green) primary buttons.
  - Clone Repository is a fully working 3-step dialog (Source / Destination / Progress,
    §15.5.1) with real streamed clone progress, not a stub — this pulls the welcome-screen-context
    half of what was originally scoped to item 15 forward into this session, so the button is
    never shipped non-functional. The workspace-context variant (§15.5.2, the future `+` button)
    is explicitly NOT built here — see item 2.
  - Recent list: mixed repos (blue icon) and workspaces (purple icon), sorted by last-opened;
    stale paths shown with amber warning icon, non-clickable, removable via right-click.
  - "Create empty workspace" secondary text link prompts name + directory, writes an empty
    `.trunk` file, opens in Workspace mode.
  - Fast path: if a previously active repo/workspace exists on disk, skip welcome and open
    directly to the graph view. **Implemented but disabled** (`FAST_PATH_ENABLED = false` in
    `src/js/welcome.js`) until the Main graph view (3) exists — `index.html` is currently an
    empty placeholder with no way back to Welcome, so skipping straight to it is a dead end
    during development. Re-enable that flag once item 3 lands.
  - Nested-repo detection rule (§15.2) applies only from this screen: plain `.git` opens
    immediately as a repository; `.git` + nested repos triggers the "Open as repository" /
    "Open as workspace" choice dialog. "Open as workspace" here auto-includes the root + all
    detected nested repos with no per-repo picker (the interactive checklist UI is explicitly
    NOT built here — see item 2, §15.6).

## 2. [x] Repository mode / Workspace mode plumbing — §15.3, §15.4, §15.5.2, §15.6 — Session 2

- **Frontend**: sidebar shell shared by `index.html` (Repositories section toggle), workspace-context
  clone dialog variant (§15.5.2, reuses `dialog.js`), nested-repo workspace-creation flow
  (§15.6, separate 2-step dialog: workspace-file path step, then per-repo checklist with
  relative path + remote/last-commit hint, "Create workspace" disabled at zero selections)
- **Backend**: mode resolution in `workspace::open_repository` / `load_workspace`,
  `.trunk` read/write (name, repo paths with relative-to-file fallback, last-active-repo,
  per-workspace overrides), active-repo switching, `clone_repository` reused for the
  workspace-context variant (no "create a workspace" checkbox; success auto-adds the clone
  to the current `.trunk` and makes it active)
- **Dependencies**: Welcome screen (1)
- **Acceptance criteria**:
  - Repository mode: no Repositories sidebar section; non-interactive pin label with repo
    name; "Add existing repository" / "Clone new repository" / "Switch repository" absent
    from UI and command palette; "Promote to workspace…" available via palette only.
    — "Promote to workspace…" itself (the command's actual implementation) is deferred to
    item 9 (command palette) since there is no palette to host it in yet — this item only
    guarantees Repository-mode UI doesn't show the Add/Clone/Switch commands a palette would
    otherwise need to hide. See item 9 when it lands.
  - Workspace mode: Repositories section with `+` button (Add existing / Clone new); exactly
    one active repo at a time; switching is instant and always lands on the graph view.
  - "Clone new" from the `+` button opens the workspace-context clone dialog (§15.5.2): same
    3-step flow as item 1's clone dialog minus the "create a workspace" checkbox; on success the
    clone is auto-added to the current `.trunk` and becomes active.
  - "Add existing" from the `+` button, when the selected folder has nested repos, opens the
    interactive nested-repo picker (§15.6) — NOT the auto-include-all shortcut used by item 1's
    welcome-screen choice dialog. This picker, and item 1's simpler auto-include path, are two
    distinct entry points into the same underlying `create_workspace` backend command.
    — Simplified to a single-step checklist (no separate "workspace file location" step):
    the only entry point reachable from this session's UI is "add to the already-open
    workspace" (the sidebar `+` button only exists in Workspace mode), so there is no
    "create a new workspace" case to handle here. Loops `add_repository_to_workspace` per
    checked row instead of calling `create_workspace`.
  - Switching repos mid-conflict-resolution: warns "switch anyway / cancel", conflict markers
    on disk untouched. Mid-rebase: hard-blocked, no override.
  - Empty workspace (§15.7): centred empty-state card (icon, "No repositories yet", green Add
    repository / blue Clone repository buttons, drag-folder hint).

## 3. [x] Main graph view (anchor) — §7 — Session 3

- **Frontend**: `src/index.html`, `src/components/commit-row.js`, `src/components/sidebar-item.js`,
  `src/components/branch-pill.js`
- **Backend**: commit graph walk (virtualised), branch-lane assignment (hash-derived colour,
  persists for branch lifetime), filter/search index (author, branch, message, path, date, SHA)
- **Dependencies**: Repository/Workspace mode (2)
- **Acceptance criteria**:
  - Two-panel layout: 156px fixed sidebar + full-width graph canvas, canvas never shrunk by
    any overlay/takeover.
  - Fixed-height rows (28px default, configurable 22–36px), 7 lane colours, HEAD = filled
    circle, others outline, merge connectors between lanes.
  - Branch pills inline on tip-commit rows (local/remote/tag visually distinct).
  - Performance: <500ms initial render at 10k commits, <2s at 100k commits, 60fps scroll, DOM
    virtualised to visible rows only.
  - Filter bar: composable filters, non-matching commits dimmed (not hidden) to preserve topology.
  - Re-enable the Welcome screen's fast path (`FAST_PATH_ENABLED` in `src/js/welcome.js`,
    disabled in item 1) now that this view exists and gives the user somewhere real to land.

## 4. [x] Commit detail overlay — §4.3 — Session 4

- **Frontend**: overlay component (right-side, 264px), reuses `diff-line.js`
- **Backend**: commit metadata + diff fetch for a single commit, cherry-pick/revert/branch-here commands
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**: slides in from right over the (unshrunk) graph canvas; metadata,
  changed-files list with +/− counts, inline unified diff per selected file, action buttons
  (cherry-pick, revert, branch here, copy SHA); Escape or click-outside dismisses.

## 5. Staging & committing — §4.4, §8

- **Frontend**: `src/staging.html`, `src/components/diff-line.js`
- **Backend**: working-tree diff, whole-file/hunk/line staging, commit (+ amend), SSH commit signing
- **Dependencies**: Commit detail overlay (4) for shared diff rendering
- **Acceptance criteria**:
  - Manual entry only (toolbar button or ⌘⇧S) — never auto-shown. Three columns: file list
    (196px) / hunk diff (centre) / commit panel (214px).
  - File list rows: tri-state checkbox, monospaced filename, +/− stats, M/A/D badge; green
    "Stage all" in section header.
  - Hunk/line staging: green "stage hunk" flips to amber "unstage hunk" once staged (dynamic
    colour flipping, §5.1); staged lines filled ● in gutter, unstaged ○ at 38% opacity.
  - Commit panel: message textarea (no char limit), amend toggle (pre-fills previous message,
    same target SHA), SSH sign toggle, push-after-commit option. Primary: solid blue "commit to
    main". Secondary: blue-outline "amend last commit". **No GPG signing in v1.**
  - Exit: "← history" button (Esc badge) or Escape; first Escape defocuses an active input,
    second exits to graph; transient toast confirms return.

## 6. Merge + Conflict resolver — §4.6, §9

- **Frontend**: full-screen conflict resolver view, file tabs, three-panel editor, merged-result panel
- **Backend**: merge/rebase/cherry-pick conflict detection, per-hunk accept-ours/theirs/both,
  manual edit mode parsing, `git continue` / abort
- **Dependencies**: Staging (5)
- **Acceptance criteria**:
  - Auto-entry on any conflicting operation; replaces graph canvas entirely.
  - Amber banner: operation + conflict count + progress (N of M resolved). File tabs: red dot
    unresolved, green dot resolved.
  - Three-panel editor: ours (green tint) / base (gray, reference-only) / theirs (blue tint),
    synced scroll. Per-hunk controls on theirs panel: accept ours (green), accept theirs
    (blue), accept both (amber), edit manually (neutral). Resolved hunks show a green
    "✓ accepted" bar with an undo button.
  - Merged result panel (pinned, bottom): preview mode read-only with live-updating
    `<<<<<<<`/`>>>>>>>` markers for unresolved hunks; edit mode is an editable textarea with
    an amber "edit mode active" banner; "Done editing" re-parses — file resolves if no markers
    remain. Undoing a hunk after manual edits discards those edits.
  - Continue (blue) disabled until every file resolved. Escape/Abort cancels with no git
    operations applied and restores the working tree.

## 7. Push / Fetch / Pull dialogs — §12

- **Frontend**: 3 small modal dialogs (shared header/body/footer chrome)
- **Backend**: push (incl. force/`--force-with-lease`), fetch (incl. prune/tags/submodules), pull
  (rebase/merge/ff-only strategies), remote auth
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**:
  - Push: blue arrow-up icon, primary "push N commits"; from/to dropdowns; commit summary list
    (SHA, green "new" badge, message, author, time); "Set as upstream" + "Force push" checkboxes
    — checking force reveals a red warning box and a default-checked "--force-with-lease
    instead" checkbox.
  - Fetch: gray arrow-down icon (neutral — read-only); remote dropdown defaults "All remotes";
    blue info box; prune checkbox default-checked, tags/submodules checkboxes default-unchecked;
    primary button neutral "fetch".
  - Pull: amber git-merge icon; primary button label updates dynamically ("pull and rebase" /
    "pull and merge" / "pull fast-forward only"); incoming commit summary with blue badges;
    amber diverged-branch warning when applicable; strategy radios (Rebase default / Merge /
    Fast-forward only). Conflicts open the resolver (6) automatically.
  - Shared: Escape/Cancel dismiss all three; in-place progress after primary click; network
    error keeps dialog open with red error box + Retry; footer always shows remote URL.

## 8. Branch dialogs — §13

- **Frontend**: 4 separate small dialogs (Create/Switch/Rename/Delete)
- **Backend**: branch create/checkout/rename/delete, merge-status check, dirty-tree
  stash-and-restore
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**:
  - Create (⌘⇧B, green plus icon): monospaced name field with real-time validation (green
    border + ✓ valid / red border + specific error); starting-point dropdown defaults HEAD,
    pre-filled with blue info box when opened from graph context menu; "Checkout after
    creating" default-on, "Push to remote after creating" default-off; button disabled until valid.
  - Switch (⌘B, blue git-branch icon): search-focused on open, Enter switches; local branches
    (newest first) then remote-only below a separator; dirty tree shows amber warning + radio
    (stash-and-reapply default, or carry-over); remote branch selection relabels button
    "Checkout & track" with explanatory info box; "Create new branch… ⌘⇧B" link at bottom.
  - Rename (amber pencil icon, sidebar context menu only): read-only current name, real-time
    validated new name, amber warning about remote-tracking side effects.
  - Delete (red trash icon, sidebar context menu only): merged branch shows green safe-to-delete
    box with optional "also delete remote" checkbox, delete active immediately; unmerged branch
    shows red warning with commit-loss count and a **mandatory** "I understand this work may be
    lost" checkbox — Force Delete stays disabled at 40% opacity until checked.
  - Shared: Escape/Cancel dismiss all four; primary input auto-focused; inline
    spinner→success→auto-close (800ms) instead of immediate close on submit.

## 9. Command palette — §10

- **Frontend**: `src/components/command-palette.js`, global overlay
- **Backend**: in-memory command/branch/file/commit/author index (rebuilt on repo/workspace switch)
- **Dependencies**: Main graph view (3), Branch dialogs (8) for command inventory entries
- **Acceptance criteria**:
  - ⌘K/Ctrl+K opens instantly from anywhere; app behind dimmed but visible; Escape/⌘K
    again/click-outside closes and restores prior focus.
  - Single search field spans commands, branches (floated top), recently changed files, commits
    (SHA/message), authors. 5 scope tabs (All/Git/Navigate/View/Repos), Tab cycles them.
  - Result rows: colour-coded icon chip (§5 system), highlighted matching substring, description
    line, right-aligned shortcut.
  - ↑↓ select, Enter run+close, ⌘Enter run+keep-open, Tab cycle tabs, Escape close.
  - Performance: open <50ms, results <16ms per keystroke — index only, never touches
    filesystem/git per keystroke.
  - Command inventory per §10.6 (branch/push/fetch/pull/rebase/stash/tag/cherry-pick/revert
    under Git; staging/history/terminal/repo/workspace switch under Navigate; theme/zoom under
    View; add/clone/settings under Repos) — wire in the new shortcuts from §6 (⌘⇧B, ⌘B, ⌘P,
    ⌘F, ⌘⇧P, ⌘⇧R).

## 10. Interactive rebase — §16

- **Frontend**: full-screen same-window takeover (no second OS window), two-panel split (58%/42%)
- **Backend**: rebase plan construction, drag-reorder, pick/reword/squash/fixup/edit/drop
  execution, step-by-step progress, conflict handoff to resolver (6)
- **Dependencies**: Conflict resolver (6)
- **Acceptance criteria**:
  - Header: branch/target/commit-count context + Esc-to-cancel hint. Footer: live plain-English
    summary (left), Cancel / Begin Rebase (right).
  - Commit list (left, 58%, newest-first): draggable rows, action selectors (pick blue, reword
    green, squash amber, fixup amber, edit purple, drop red) with row tinting; reword becomes an
    inline text input, no dialog.
  - Result preview (right, 42%, after-state only): pick = filled blue dot; reword = filled green
    dot + green badge + italic new message; squash target = amber badge with absorbed count +
    message list; all-dropped = warning state.
  - Cancel/Escape: no git operations run, restores graph. Begin Rebase: per-step progress view;
    success closes and refreshes graph; conflict keeps window open and enters resolver inline.

## 11. Stash manager — §11

- **Frontend**: full-screen view, left list (240px) + right detail
- **Backend**: stash create (incl. `-u`/`--all`), pop, apply, branch-from-stash, drop
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**:
  - List newest-first (stash@{0} top): index badge, message, source-branch colour dot,
    relative time, +/− stats, file count. Titlebar: "← history" (Esc badge), green "New
    stash", blue "Push all local".
  - Detail: header (message/metadata), action bar, changed-files list (180px), diff view.
  - Pop (green, applies+removes, conflicts open resolver), Apply (blue, applies+keeps),
    Branch from stash (neutral, prompts name, creates branch at source commit, pops onto it),
    Drop (red, confirmation required).
  - New stash dialog: optional message (default "WIP on <branch>: <last commit>"), "include
    untracked" + "include ignored" checkboxes, green Create / neutral Cancel.

## 12. Tag manager — §14

- **Frontend**: full-screen view, searchable list + detail panel
- **Backend**: tag list/create/push/delete, regex-based grouping (persisted per-repo in `.trunk`)
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**:
  - List sorted newest-first, no grouping by default. "Group by" dropdown: None / Version
    series / Push status — persisted per repo. Rows: monospace name, annotated/lightweight
    badge, pushed/local-only badge, target SHA, relative time. Search filters in real time;
    empty groups' headers hide.
  - Version grouping: regex with first capture group as label, non-matches → "Other"; 4 built-in
    presets (semver `^(v\d+)\.` default, quarter-based, date-based, word-prefix); live preview
    table in the pattern editor; invalid regex handled gracefully; global default set in
    Preferences (§19.2.7).
  - Detail: target-commit card (always shown), tag message in italic box (annotated only,
    hidden for lightweight), signature badge (green shield-check verified / gray shield-off unsigned).
  - Actions: Checkout commit (neutral, detached-HEAD warning), Show in graph (neutral), Push tag
    (blue, 40%-opacity-disabled if already pushed), "Push all local" (blue, titlebar, confirms
    list), Delete (red, confirm dialog with optional "also delete from remote" + red warning).
  - Create dialog: Lightweight/Annotated radio toggle (annotated adds message textarea + SSH
    sign checkbox); starting point defaults HEAD (pre-filled + info box from graph context
    menu); real-time name validation, red border if name exists.

## 13. Remote management — §18 (sidebar-contextual, no dedicated screen)

- **Frontend**: sidebar Remotes section (expandable tracking-branch sub-rows), inline add
  dialog, edit dialog, context menu
- **Backend**: `git remote add/set-url/remove`, per-remote `sslVerify` config write, fetch-from-remote
- **Dependencies**: Main graph view (3)
- **Acceptance criteria**:
  - No standalone screen. Remotes section header has `+` for Add remote (name + URL fields,
    name validated unique, green "Add remote" / neutral Cancel).
  - Right-click context menu: Fetch from this remote (neutral, immediate, inline spinner on
    row), Edit (amber), Copy URL (neutral, "Copied" toast), Remove (red, confirms "will not
    delete the remote repository").
  - Edit dialog: pre-filled name/URL, amber "Save changes"; SSL-verify-disable checkbox
    (default unchecked) shows a red warning when checked and writes `http.<url>.sslVerify =
    false` scoped to that remote's URL only — never a global setting.

## 14. Preferences — §19

- **Frontend**: `src/preferences.html`, left nav (176px, 10 categories, 2px blue active accent),
  right content panel
- **Backend**: settings read/write (per category below), live-apply for theme/zoom only,
  everything else commits on Save
- **Dependencies**: touches most prior features (theme tokens from scaffold (0), git
  behaviour defaults feed Branch/Push-Pull dialogs (7,8), etc.) — implement last among P0/P1 screens
- **Acceptance criteria** — all 10 categories present with every setting listed in §19.2:
  1. Appearance — theme dropdown, custom theme file path+Browse+Clear (hot-reload), UI zoom
     slider 70–150%/10%, graph row height slider 22–36px/2px, font size dropdown.
  2. Identity & signing — author name/email, per-workspace-override info box, SSH-sign toggle
     (+ key path, signature program override, GPG-not-managed info box when on). **No GPG.**
  3. Git behaviour — default pull strategy, default push behaviour, prune-on-fetch toggle,
     auto-fetch interval dropdown (Off/5/10/30/60 min).
  4. Keyboard & input — interaction-mode dropdown, searchable shortcuts table + "Edit
     shortcuts…" remapping editor.
  5. Terminal — default shell dropdown, font name+size, drawer default-height slider 80–400px.
  6. Diff & staging — default diff view (Unified default / **Split**), syntax-highlighting
     toggle, whitespace-handling dropdown.
  7. Tag manager — default grouping dropdown, default version-group-pattern input.
  8. Workspace — reopen-last-session toggle, default workspace file location.
  9. Network & credentials — SSH key path+Browse, credential-helper dropdown, HTTP proxy
     input. (SSL verify is per-remote, §18.4 — not here.)
  10. Updates — auto-check toggle, update channel dropdown, current version row + "Check now"
      blue button.
  - Footer: neutral Cancel / blue Save. Opened via ⌘, / Ctrl+, / toolbar button / sidebar item.
  - **Note (added alongside item 4's follow-up):** the sidebar width and commit-detail-overlay
    width are user-adjustable by drag (not through this screen — no slider/field for either) and
    are already persisted in `src-tauri/src/settings/mod.rs`'s `AppSettings` struct, written to
    `settings.json` in the app config dir. This struct is meant to be the *same* one this item's
    backend reads/writes for the 10 categories above — extend it, don't introduce a second
    settings file — and its read/write here must not clobber `sidebar_width`/
    `commit_overlay_width` in the process.

## 15. ~~Clone dialog (+ nested-repo detection) — §15.2, §15.5–15.6~~ — merged into items 1 & 2

Originally scoped here, but shipping a non-functional "Clone Repository" button (or a
nested-repo choice with no working "Open as workspace") was judged worse than slightly
reordering work. Its content was split: the welcome-screen-context clone dialog (§15.5.1) and
a simplified (auto-include-all, no picker) nested-repo "Open as workspace" path moved to item 1;
the workspace-context clone variant (§15.5.2) and the interactive per-repo checklist picker
(§15.6) moved to item 2, since both genuinely need item 2's `+`-button sidebar to exist first.
This entry is kept (not deleted) to preserve numbering for anything referencing it below.

## 16. Backend cross-cutting work (threaded through the above)

- Auto-fetch (§19.2.3): background task honoring the configured interval, off by default behavior per dropdown.
- SSH key management (§19.2.2): just the preferences path field — no separate generate/add/list flow.
- Terminal drawer (§4.5): PTY session per repo, cwd follows active repo, resizable + persisted
  height, multi-tab.
- Credential store: OS-native (Keychain/DPAPI/Secret Service) — needed once Push/Fetch/Pull (7)
  hits an authenticated remote.

---

## Deferred / out of scope for v1 (§24) — do not implement

AI features of any kind · forge integrations (GitHub/GitLab/Bitbucket PR/MR features — plugin
host only, §21, is P2 in-scope) · **GPG commit signing** (SSH only) · **blame view** · mobile/web
clients · SVN/Mercurial · auto-update (manual updates only in v1).
