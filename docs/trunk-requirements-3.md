**TRUNK**

A fast, open-source Git GUI client

*Product Requirements Document --- v2.0*

June 2026

  --------------------- -------------------------------------------------
  **Status**            Final --- implementation ready

  **Version**           2.0

  **Date**              June 2026

  **Stack**             Rust + Tauri 2.x (native cross-platform)

  **License**           Free & Open Source (MIT / Apache-2.0)

  **Target**            All developers --- individual, team, enterprise
  --------------------- -------------------------------------------------

**1. Executive Summary**

Trunk is a native, cross-platform Git GUI client built for raw speed and broad reach. It targets individual developers, small teams, and large engineering organisations alike --- anyone who wants a powerful visual interface to Git without sacrificing performance or paying for it.

The Git GUI landscape in 2026 is bifurcated: fast native clients lack cross-platform reach, while cross-platform clients are Electron-based and struggle with large repositories. Trunk closes that gap with native Rust performance across macOS, Windows, and Linux, free forever, with a plugin ecosystem for forge integrations.

Trunk operates in two distinct modes: Repository mode (a single .git directory) and Workspace mode (a .trunk file grouping multiple repositories). The mode is always visible to the user and determines the sidebar layout and available commands.

**2. Goals & Non-Goals**

**2.1 Goals for v1**

-   Render commit graphs and branch histories faster than any Electron-based client, including on repositories with 100,000+ commits.

-   Provide a complete daily Git workflow: viewing history, staging hunks, committing, branching, merging, rebasing, conflict resolution, and push/pull.

-   Support multiple repositories organised into named workspaces (.trunk files).

-   Integrate a full terminal emulator as an opt-in bottom drawer.

-   Ship with system-default theming (light/dark) and allow users to switch.

-   Establish a plugin API surface for forge integrations to be added post-v1.

**2.2 Non-Goals for v1**

-   AI-assisted commit messages, diff summarisation, or conflict guidance --- deferred to v2.

-   Forge-specific features (pull request creation, issue linking, code review) --- deferred to plugin packages.

-   Blame view --- deferred to v2; covered adequately by IDE integrations.

-   Mobile or web clients. SVN or Mercurial support.

-   GPG signing --- deferred to v2. SSH signing is supported.

**3. Target Users**

  ---------------------------------------- ----------------------------------------------------------- --------------------------------------------- ----------------------------------
  **Profile**                              **Typical use**                                             **Key need**                                  **Risk if unmet**

  **Solo developer / OSS contributor**     Daily commits, branch management, history inspection        Speed, low friction                           Switches to CLI

  **Small team (2--10)**                   Parallel branches, code review, merge conflict resolution   Visual clarity on multi-branch state          Mismerges, lost context

  **Mid-size engineering org (10--100)**   Large monorepos, multiple services, CI/CD integration       Performance at scale, multi-repo workspaces   Tool unusable on large histories
  ---------------------------------------- ----------------------------------------------------------- --------------------------------------------- ----------------------------------

**4. Layout & Interaction Model**

**4.1 Two-panel layout**

The default window layout is two panels: a left sidebar (fixed \~156px) for workspace/repo/branch navigation, and a full-width graph canvas for the commit graph. The graph canvas is the home screen and the largest panel at all times.

**4.2 Workspace sidebar**

-   Workspace switcher at the top: dropdown showing the current workspace name with a selector icon.

-   Repositories section (workspace mode only): lists repos with a + button for Add existing / Clone new.

-   Branches: lists local branches with colour dots. Active branch highlighted with left blue border.

-   Remotes: lists configured remotes with tracking branch sub-rows. Section header has + button.

-   Stashes: count and quick-access. Tags: list of all tags.

-   Footer: Preferences link.

-   In repository mode: no Repositories section. A non-interactive pin label shows the repo name at the top.

**4.3 Commit detail overlay**

Clicking a commit row opens a detail overlay sliding in from the right, rendering over the graph canvas without shrinking it. Contains: commit metadata, changed files list with +/− counts, inline unified diff for selected file, and action buttons (cherry-pick, revert, branch here, copy SHA). Escape or clicking outside dismisses it.

**4.4 Staging view**

The staging view replaces the graph canvas when explicitly triggered via the toolbar \"Stage changes\" button or ⌘⇧S. Three columns: file list (left, 196px), hunk-level diff (centre), commit panel (right, 214px). Returns to graph via \"← history\" button or Escape. If focus is in a text field, first Escape defocuses; second Escape exits staging.

**4.5 Terminal drawer**

A terminal emulator embedded as a bottom drawer, hidden by default, toggled with ⌘\` (configurable). Overlays the bottom of the graph canvas --- does not push content up. Opens in the active repository\'s working directory. Height is user-resizable and persisted. Multiple tabs supported.

**4.6 Merge conflict resolution**

When a merge/rebase/cherry-pick produces conflicts, the conflict resolver replaces the graph canvas. An amber banner shows the operation and conflict count. The resolver is a three-panel editor (ours / base / theirs) with a pinned merged result panel below. See §9 for full spec.

**5. Button Colour System**

All interactive buttons across the Trunk UI follow a strict semantic colour system. Colour communicates the nature and consequence of an action.

  ---------------------- ---------------- ------------ --------------------------------------------------- -----------------------------------------
  **Colour**             **Background**   **Border**   **Meaning**                                         **Examples**

  **Blue --- solid**     #185FA5          #0C447C      Primary / most important constructive action        Commit, Begin rebase, Continue merge

  **Blue --- outline**   #E6F1FB          #378ADD      Secondary constructive, related to primary          Amend last commit

  **Green --- solid**    #3B6D11          #27500A      Additive / staging action                           Stage all, Stage hunk, Create branch

  **Amber**              #FAEEDA          #EF9F27      Reversible caution --- undoable state change        Stash, Unstage hunk, Rename branch

  **Red**                #FCEBEB          #E24B4A      Destructive / irreversible --- data loss possible   Discard all, Drop commit, Delete branch

  **Neutral**            #F5F4F0          #C8C6BE      Navigation, settings, secondary actions             Cancel, Back, Settings, Copy SHA
  ---------------------- ---------------- ------------ --------------------------------------------------- -----------------------------------------

-   Dynamic colour flipping: \"Stage hunk\" renders green. Once staged, the same button reads \"Unstage hunk\" in amber. The colour always describes what will happen on click.

-   Disabled state: always gray (#ECEAE3 bg, #C8C6BE border, #888780 text) regardless of semantic colour.

-   In the dark theme, button colours use the dark palette equivalents. Theme authors must maintain the blue/green/amber/red distinction.

**6. Keyboard & Mouse Configuration**

Trunk ships with two built-in interaction profiles selectable from preferences: Keyboard-first (default) and Mouse-first. All shortcuts are user-configurable via the preferences keyboard shortcuts editor.

  -------------------------- ----------------------- ---------------------------
  **Capability**             **Keyboard profile**    **Mouse profile**

  **Navigate commits**       ↑↓ arrow keys           Click row

  **Open commit detail**     Enter / Space           Click row

  **Dismiss overlay**        Escape                  Click outside

  **Stage hunk**             S key on hunk           Click stage hunk button

  **Command palette**        ⌘K (always on)          ⌘K (always on)

  **Toggle terminal**        ⌘\` (configurable)      Toolbar button

  **Switch staging/graph**   ⌘⇧S                     Toolbar button

  **Create branch**          ⌘⇧B                     Sidebar + button → Create

  **Switch branch**          ⌘B                      Sidebar branch click

  **Push**                   ⌘P                      Toolbar push button

  **Fetch**                  ⌘F                      Toolbar fetch button

  **Pull**                   ⌘⇧P                     Toolbar pull button

  **Interactive rebase**     ⌘⇧R                     Toolbar rebase button
  -------------------------- ----------------------- ---------------------------

**7. Commit Graph**

**7.1 Visual design**

-   Each commit is a row. Rows are fixed height (28px default, configurable 22--36px).

-   Branch lanes are rendered as vertical coloured lines left of commit messages. Each branch gets a consistent colour derived from its name hash. Seven lane colours: \--lane-1 through \--lane-7.

-   Merge commits show branching/joining connectors between lanes.

-   Branch pills shown inline on the tip commit row: local branches, remote tracking branches, and tags are visually distinct with colour-coded backgrounds.

-   HEAD commit has a filled circle node; others are outlines.

**7.2 Performance targets**

-   Initial render of 10,000 commits: \< 500ms on reference hardware (M-series Mac, mid-range Windows laptop).

-   Initial render of 100,000 commits: \< 2 seconds.

-   Scrolling at any depth: 60fps with no jank. Graph is virtualised --- only visible rows rendered.

**7.3 Filtering & search**

-   Filter bar supports: author, branch, message text, file path, date range, SHA prefix. Filters are composable.

-   Filtered results highlight matching commits; non-matching commits are dimmed rather than hidden, preserving graph topology.

**8. Staging & Committing**

-   The staging view is triggered manually via toolbar or ⌘⇧S. It replaces the graph canvas. It is never shown automatically.

-   Three-column layout: file list (left, 196px), hunk-level diff (centre), commit panel (right, 214px).

-   File list: each row shows a checkbox (unchecked / partial / checked), filename in monospace, +/− stats, and a type badge (M/A/D). \"Stage all\" green button in section header.

-   Staging granularity: whole file (checkbox), individual hunk (\"stage hunk\" green button), individual line (click line to toggle). Partial-hunk staging supported.

-   Staged lines show a filled ● in the gutter; unstaged show ○. Unstaged lines are shown at 38% opacity.

-   Hunk controls: green \"stage hunk\" flips to amber \"unstage hunk\" when staged.

-   Commit panel: message textarea (no character limit), staged summary stats, options (amend, SSH sign, push after commit), author display, branch display.

-   Primary button: solid blue \"commit to main\". Secondary: blue-outline \"amend last commit\".

-   No character limit or counter on the commit message subject line.

-   Amend mode: pre-fills message with previous commit\'s message; targets same SHA.

-   SSH commit signing supported and configurable. GPG signing deferred to v2.

-   Exit: \"← history\" button in titlebar (with Esc badge), or Escape. First Escape defocuses active input; second Escape exits staging. A transient toast confirms return to graph.

**9. Conflict Resolver**

When a merge, rebase, or cherry-pick produces conflicts, Trunk enters conflict mode automatically. The graph canvas is replaced by the conflict resolver --- a full-screen view that remains active until all conflicts are resolved or the operation is aborted.

**9.1 Entry and exit**

-   Entry: automatic on any operation that produces conflicts.

-   Exit --- abort: Escape or Abort button cancels the operation and restores the working tree.

-   Exit --- continue: once all files are resolved, the Continue button (blue) executes git continue and returns to graph.

**9.2 Layout**

-   Conflict banner (amber): operation in progress, total conflicting file count, progress indicator (N of M resolved).

-   File tabs: one per conflicting file. Red dot = unresolved, green dot = resolved.

-   Three-panel editor (centre): ours (left, green tint), base / common ancestor (centre, gray), theirs (right, blue tint). All three panels scroll in sync.

-   Merged result panel (bottom, pinned): shows live merged output. Toggles between preview and edit mode.

**9.3 Three-panel editor**

-   Per-hunk action controls live on the \"theirs\" panel: Accept ours (green), Accept theirs (blue), Accept both (amber), Edit manually (neutral).

-   The \"ours\" panel also carries a shortcut \"accept ours\" button. Base panel is reference only --- no action buttons.

-   Once a hunk is resolved, all three panels show a green \"✓ accepted \[choice\]\" bar. An undo button restores the hunk to unresolved state.

**9.4 Merged result panel**

-   Preview mode (default): read-only. Unresolved hunks shown as red \<\<\<\<\<\<\< / \>\>\>\>\>\>\> markers. Updates in real time as hunks are resolved.

-   Edit mode: a plain editable textarea with line numbers. An amber banner marks edit mode as active. \"Done editing\" (amber) returns to preview and re-parses content.

-   If no conflict markers remain when \"Done editing\" is clicked, the file is considered resolved.

-   Undoing a hunk-level resolution after manual editing resets the textarea to auto-generated content, discarding manual edits.

**9.5 Resolution states and Continue button**

-   A file is resolved when: all hunks accepted via three-panel controls, OR manual edit with no markers remaining, OR \"Mark resolved\" clicked.

-   Continue button (blue) is disabled until every conflicting file is resolved.

  -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *The conflict resolver is the highest-stakes view in Trunk. Every resolution is undoable until Continue is clicked. Manual edit mode is available for power users --- entering it for a file and then undoing a hunk-level resolution discards the manual edits.*

  -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**10. Command Palette**

The command palette is a global fuzzy-search overlay triggered by ⌘K / Ctrl+K from anywhere in the application. It is the primary discovery and navigation surface for keyboard-driven users.

**10.1 Triggering and dismissal**

-   Open: ⌘K / Ctrl+K from any view. Opens instantly over the current view; app behind is dimmed but spatially visible.

-   Close: Escape, ⌘K again, or clicking outside. Focus returns to the previously active element.

**10.2 Search scope**

-   A single input field searches across: commands (all application actions), branches (by name, floated to top), files (recently changed), commits (SHA prefix and message), authors.

-   Five scope filter tabs (All, Git, Navigate, View, Repos) narrow results without changing the query. Tab key cycles through scope tabs.

**10.3 Result rows**

Each result row shows a colour-coded icon chip (following the §5 button colour system), a label with the matching substring highlighted in blue, a description line, and any keyboard shortcut right-aligned. The palette doubles as a shortcut reference card.

**10.4 Keyboard navigation**

-   ↑↓ --- move selection. Enter --- run action and close. ⌘Enter --- run action and keep palette open. Tab --- cycle scope tabs. Escape --- close.

**10.5 Performance requirement**

-   Palette must open in under 50ms. Results must return in under 16ms for any query. Queries an in-memory index --- never touches the filesystem or git on each keystroke.

**10.6 Command inventory**

-   Git: create/switch/merge/delete branch, push, fetch, pull, interactive rebase, stash, pop stash, create tag, cherry-pick, revert.

-   Navigate: open staging view, go to history, toggle terminal, switch repository, switch workspace.

-   View: switch theme, increase/decrease/reset UI scale.

-   Repos: add repository, clone, repository settings, application preferences.

**11. Stash Manager**

The stash manager is a dedicated full-screen view (same-window replacement of the graph canvas) for browsing, inspecting, and acting on all stashes in the active repository.

**11.1 Layout**

-   Left panel (240px): stash list, ordered newest first (stash@{0} at top). Each row shows: stash index badge, message, source branch with colour dot, relative timestamp, +/− stats, file count.

-   Right panel: stash detail --- header (message, metadata), action bar, changed files list (180px), diff view.

-   Titlebar: \"← history\" back button with Esc badge, \"New stash\" green button, \"Push all local\" blue button.

**11.2 Actions**

-   Pop (green): applies stash and removes it. Conflict resolver opens if conflicts arise.

-   Apply (blue): applies stash but keeps it in the list.

-   Branch from stash (neutral): prompts for a new branch name, creates the branch from the commit the stash was made on, then pops the stash onto it.

-   Drop (red): deletes stash permanently. Confirmation dialog shown before deletion.

**11.3 New stash dialog**

-   Message field (optional, defaults to \"WIP on \<branch\>: \<last commit\>\").

-   Checkbox: include untracked files (git stash -u). Checkbox: include ignored files (git stash \--all).

-   Create stash button (green) and Cancel (neutral).

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *Pop vs Apply is the most common point of confusion in stash UIs. Trunk labels both explicitly and keeps them visually distinct (green vs blue). The source branch dot on each stash row gives the context needed to decide whether applying is safe.*

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**12. Push, Fetch, and Pull Dialogs**

Each operation opens a small modal dialog over the current view. All three share the same visual structure: header with icon/title/subtitle; body with fields, previews, and options; footer with remote URL and action buttons.

**12.1 Push dialog**

-   Icon: blue arrow-up. Primary button: solid blue \"push N commits\" (N = commit count in summary).

-   From/To dropdowns: local branch (left) and remote/branch target (right).

-   Commit summary: list of all local commits not yet on the remote --- SHA, \"new\" green badge, message, author, timestamp.

-   \"Set as upstream\" checkbox, \"Force push\" checkbox (unchecked by default). When force push is checked: red warning box appears, and \"\--force-with-lease instead\" checkbox appears (checked by default).

-   Footer: remote URL left-aligned for confirmation.

**12.2 Fetch dialog**

-   Icon: gray arrow-down (neutral --- fetch is a safe read-only operation).

-   Remote dropdown (defaults to \"All remotes\"). Blue info box explains fetch does not modify the working tree.

-   \"Prune deleted remote branches\" checkbox (checked by default). \"Fetch tags\" and \"Fetch submodules\" checkboxes (unchecked by default).

-   Primary button: neutral \"fetch\". Fetch is neither constructive nor destructive.

**12.3 Pull dialog**

-   Icon: amber git-merge. Primary button: amber, label updates dynamically (\"pull and rebase\" / \"pull and merge\" / \"pull fast-forward only\").

-   Into/From dropdowns. Incoming commit summary with blue \"incoming\" badges.

-   Diverged branch warning: amber box when local has commits not on remote.

-   Integration strategy radio group: Rebase (default), Merge, Fast-forward only --- each with plain-English description.

-   If pull produces conflicts, conflict resolver (§9) opens automatically.

**12.4 Shared behaviours**

-   All three dismissed with Escape or Cancel. All show in-place progress after primary button is clicked. On network error, dialog stays open with red error box and Retry button. Footer always shows remote URL.

**13. Branch Dialogs**

Branch operations use four separate focused dialogs triggered from the sidebar context menu, the graph context menu, or the command palette.

**13.1 Create branch (⌘⇧B)**

-   Icon: green plus. Primary button: green \"Create branch\".

-   Branch name field: monospaced, validated in real time. Green border + ✓ hint when valid. Red border + specific error when invalid (conflict or illegal characters). Button disabled until name is valid.

-   Starting point dropdown: defaults to HEAD with SHA. Pre-filled with selected commit when opened from graph context menu (blue info box confirms this).

-   \"Checkout after creating\" checkbox (default on). \"Push to remote after creating\" checkbox (default off).

-   Footer: \"from \[SHA\]\" in monospaced gray.

**13.2 Switch branch (⌘B)**

-   Icon: blue git-branch. Primary button: blue \"Switch\". Search input focused on open; typing filters list; Enter switches.

-   Branch list: local branches (newest first) then remote-only below a separator. Each row: colour dot, name, current badge or timestamp, remote label.

-   Dirty working tree: amber warning + radio group --- \"Stash automatically, apply after switch\" (default) or \"Carry over (may fail on conflict)\".

-   Remote branch selected: button label changes to \"Checkout & track\"; blue info box explains local tracking branch creation.

-   \"Create new branch... ⌘⇧B\" text link at the bottom (dashed top border).

**13.3 Rename branch**

-   Icon: amber pencil. Button: amber \"Rename\". From sidebar context menu only.

-   Current name: read-only pre-filled. New name: validated in real time. Amber info box warns about remote tracking branch side effects.

**13.4 Delete branch**

-   Icon: red trash. Triggered from sidebar context menu only.

-   Merged branch (safe): green confirmation box \"fully merged into main. Safe to delete.\" Optional checkbox to also delete remote branch. Delete button immediately active.

-   Unmerged branch (destructive): red warning box stating how many commits would be lost. Mandatory acknowledgement checkbox \"I understand this work may be lost\" must be checked before Force delete button activates (otherwise disabled at 40% opacity).

**13.5 Shared behaviours**

-   All four dismissed with Escape or Cancel. All focus their primary input on open. All show inline success message (spinner → success → auto-close after 800ms) rather than closing immediately on submit.

  ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *The delete branch dialog for unmerged branches is one of the highest-stakes moments in the entire application. The mandatory acknowledgement checkbox is non-negotiable. There is no way to undo a branch deletion if the commits were not pushed to a remote.*

  ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**14. Tag Manager**

The tag manager is a dedicated full-screen view for browsing, inspecting, creating, pushing, and deleting tags in the active repository.

**14.1 Tag list**

-   Tags are displayed sorted by date (newest first) by default with no grouping.

-   \"Group by\" dropdown in the toolbar: None (default), Version series, Push status. The chosen mode is persisted per repository in the .trunk file.

-   Each row shows: tag name (monospace), type badge (annotated/lightweight), status badge (pushed/local only), target SHA and relative timestamp.

-   Search input filters tags by name in real time. Section headers hide when all their tags are filtered out.

**14.2 Version group pattern (configurable)**

-   A regex applied to each tag name. First capture group becomes the group label. Tags that do not match go in \"Other\".

-   Pattern editor: regex input field, live preview table showing every tag with its resolved group. Invalid regex handled gracefully.

-   Four built-in presets: semver \^(v\\d+)\\. (default), quarter-based, date-based, word-prefix.

-   Pattern saved per repository. Global default set in preferences (§19).

**14.3 Tag detail panel**

-   Target commit card (always shown): SHA, branch with colour dot, author/date, commit message.

-   Tag message (annotated tags only): full annotation in an italicised box. Hidden for lightweight tags.

-   Signature: green shield-check for GPG-verified tags; gray shield-off for unsigned.

**14.4 Actions**

-   Checkout commit (neutral): switches to the target commit in detached HEAD state. Shows warning before proceeding.

-   Show in graph (neutral): returns to graph view with the target commit selected.

-   Push tag (blue): pushes this specific tag to origin. Disabled at 40% opacity when already pushed.

-   \"Push all local\" (blue, titlebar): pushes all local-only tags. Shows confirmation listing which tags will be pushed.

-   Delete (red): opens delete tag confirmation dialog. Optional \"Also delete from remote\" checkbox with red warning.

**14.5 Create tag dialog**

-   Two modes toggled by radio: Lightweight (name only) or Annotated (name + message textarea + optional SSH signing checkbox).

-   Starting point: defaults to HEAD. Pre-filled from graph context menu with blue info box.

-   Name validated in real time --- red border if name already exists.

**15. Welcome Screen, Repository Mode & Workspace Mode**

Trunk operates in one of two distinct modes depending on what the user opens. The mode is set at open time and determines the sidebar layout, available commands, and navigation options.

**15.1 Welcome screen**

-   Open Repository (primary, blue): folder picker. Applies nested-repo detection rule (§15.2) when opened from welcome screen.

-   Clone Repository (primary, green): opens clone dialog in welcome-screen context (§15.5).

-   Recent elements list: mixed repos and workspaces sorted by last-opened date. Each entry shows a type icon (repository = blue, workspace = purple), name, full path, relative timestamp. Stale paths shown with amber warning icon, non-clickable. Users can remove stale entries via right-click.

-   \"Create empty workspace\" (secondary, text link): prompts for name + directory, creates empty .trunk file, opens in workspace mode.

-   Fast path: if a previously active repo/workspace exists on disk, Trunk skips the welcome screen and opens directly to the graph view.

**15.2 Nested-repo detection rule**

-   Folder has .git only → open as repository immediately, no dialog.

-   Folder has .git AND nested repos inside → show choice dialog: \"Open as repository\" or \"Open as workspace\" (two-step creation, §15.6).

-   This detection only applies from the welcome screen. The workspace + button always treats root .git as repository unconditionally.

**15.3 Repository mode**

-   No Repositories sidebar section. A non-interactive pin label at the top of the sidebar shows the repo name.

-   Commands \"Add existing repository\", \"Clone new repository\", and \"Switch repository\" are absent from the UI and command palette.

-   \"Promote to workspace...\" is available via command palette only. Creates a .trunk file with the current repo pre-added, then re-opens in workspace mode.

**15.4 Workspace mode**

**15.4.1 The .trunk workspace file**

A .trunk file is a TOML config file containing: workspace name, array of repository paths (absolute with relative-to-file fallback), last active repository path, per-workspace config overrides (e.g. version group pattern, author identity override).

**15.4.2 Sidebar in workspace mode**

Repositories section at the top with + button (Add existing / Clone new). Each repo row shows name, branch indicator, and optional in-progress badges.

**15.4.3 Active repository**

Exactly one repository is active at any time --- selected in the sidebar Repositories list. The active repo drives: graph canvas, sidebar branch/remote/stash/tag sections, all toolbar operations, terminal cwd, and titlebar name. Clicking a different repo switches instantly; user always lands on graph view.

**15.4.4 Switching repos --- destructive state handling**

-   Conflict resolver mid-resolution (choices made but Continue not yet clicked): dialog warns --- \"repo A has an unresolved merge conflict. Switch away and return to it later?\" Options: Switch anyway / Cancel. Conflict markers on disk are untouched.

-   Interactive rebase mid-execution (git holds a rebase lock): hard-blocked. Dialog explains; no \"switch anyway\" option.

-   All other views (staging, stash manager, tag manager, push/pull/fetch dialogs): switch freely. Staging index is git state on disk --- never lost by switching repos.

**15.5 Clone dialog --- two contexts**

**15.5.1 Welcome-screen context**

-   Step 1 --- Source: URL field with smart inference from GitHub/GitLab/Bitbucket URLs. HTTPS vs SSH inferred from URL format.

-   Step 2 --- Destination: \"Clone into\" path field + \"Also create a workspace\" checkbox (unchecked by default). When checked, a \"Workspace file\" path field appears immediately --- both paths are independently editable and reflect changes instantly. The user sees exactly where the repo and workspace file will land.

-   Step 3 --- Progress: inline terminal-style log (real git clone output, not a spinner). On success: if workspace checkbox was checked, opens in workspace mode with cloned repo listed. If unchecked, opens in repository mode. On failure: error shown inline with Retry button; URL and destination fields are restored.

**15.5.2 Workspace context (from + button)**

\"Also create a workspace\" option is absent --- a workspace is already open. On success, cloned repo is automatically added to the current workspace .trunk file and becomes the active repository.

**15.6 Nested-repo workspace creation flow**

-   Step 1 --- Workspace file location: path field pre-filled with parent folder + folder name + .trunk extension. Editable; folder picker provided.

-   Step 2 --- Select repositories: checklist of all detected git repos including the root repo (checked by default). Each row shows path relative to workspace file and a hint (remote URL or last commit message). \"Create workspace\" button is disabled when zero repos are checked (hint: \"Select at least one repository\").

-   Creating the workspace writes the .trunk file, then opens Trunk in workspace mode with the selected repos in the sidebar and the root repo as the active repository.

**15.7 Post-creation and post-open behaviour**

-   Non-empty workspace (repos already on disk): auto-selects root repository, shows its graph view immediately.

-   Empty workspace: shows empty state panel on main canvas. Panel: git-branch icon, \"No repositories yet\", \"Add existing repository\" (green) button, \"Clone repository\" (blue) button, divider, \"Or drag a folder here to add it\" hint. Sidebar shows Repositories section with + button and hint text.

-   Drag and drop onto empty canvas: adds the dragged folder to the workspace. Root .git always wins --- no nested-repo detection dialog in this context (same rule as + button).

-   Returning user: Trunk reads last active repo path from .trunk and opens its graph directly. If that path no longer exists: falls back to first repo in list and shows amber toast \"Could not find \[repo name\] --- opened \[other repo\] instead.\"

**16. Interactive Rebase**

Interactive rebase opens as a full-screen takeover within the main Trunk application window. The sidebar and graph canvas are replaced entirely by the rebase UI. No second OS window is spawned.

**16.1 Layout**

-   Two panels divided by a fixed vertical splitter: commit list (left, 58%) and result preview (right, 42%).

-   A slim header bar at the top carries the operation context (branch name, target, commit count) and an Esc-to-cancel hint.

-   A footer bar shows the live summary on the left and Cancel / Begin Rebase buttons on the right.

**16.2 Commit list**

-   Commits listed newest-first (top) to oldest (bottom), matching git rebase -i ordering for power-user familiarity.

-   Rows are draggable. Dropping a row reorders the commit and the preview updates immediately.

-   Action selectors: pick (blue), reword (green), squash (amber), fixup (amber), edit (purple), drop (red). Row background tinting makes groupings scannable without reading dropdowns.

-   Selecting reword turns the message field into an inline text input --- no dialog. Edited message flows into preview in real time.

**16.3 Result preview panel**

-   Shows only the resulting history --- no before column. Each resulting commit is a node on a vertical lane using Trunk graph visual language.

-   pick: filled blue dot. reword: filled green dot, green badge, new message in green italics. squash target: amber badge showing absorbed commit count, plus annotation listing their messages. All dropped: warning state.

**16.4 Execution**

-   Cancel (Escape): dismisses the takeover with no git operations run. Restores graph view.

-   Begin Rebase: executes the sequence. Window transitions to a progress view per step. On success it closes and the main graph refreshes. On conflict, stays open and enters conflict resolution flow.

  -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *The rebase takeover is the most complex view in Trunk v1. Its design prioritises safety --- explicit after-state preview, destructive action colour coding, no accidental dismissal --- over speed. The full-screen same-window approach avoids taskbar clutter and multi-monitor placement issues.*

  -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**17. Theme & Appearance**

-   Default theme at installation follows the OS light/dark mode setting.

-   Users can override to light, dark, or \"follow system\" in preferences. Theme applies immediately without restart.

-   Theme files are CSS custom-property files on disk. Location: \~/Library/Application Support/trunk/themes/ (macOS), %APPDATA%/trunk/themes/ (Windows), \~/.config/trunk/themes/ (Linux).

-   Trunk ships light.css and dark.css on first launch. Both are fully commented. Any valid .css file in the themes folder is auto-detected in preferences. Hot-reload within 200ms on save; transient toast confirms. Invalid CSS falls back to built-in default with warning.

-   UI zoom scale: 70--150% in 10% steps, default 100%. ⌘+/−/0 (configurable). Persisted per workspace. Transient badge appears briefly when zoom changes.

-   Font size and graph row height are user-configurable (see §19 Preferences).

-   Trunk does not ship a visual theme editor in v1 --- the CSS file is the editor.

**17.1 Official dark theme palette**

The dark theme shipped with Trunk (dark.css) uses the following design tokens. This is the canonical reference for all dark-mode UI work and for users building custom themes.

  ------------------------------ ---------------------------------------------------------------------
  **\--bg-base**                 #0D1117 --- Titlebar, window chrome, terminal background

  **\--bg-primary**              #161B22 --- Main content area, graph canvas, sidebar

  **\--bg-secondary**            #1C2128 --- Cards, setting groups, overlay panels

  **\--bg-tertiary**             #22272E --- Inputs, dropdowns, toolbar buttons

  **\--bg-raised**               #2D333B --- Hover states, toggle tracks, keyboard badge backgrounds

  **\--border-subtle**           #30363D --- Default dividers, card edges, section separators

  **\--border-default**          #444C56 --- Inputs, controls, interactive element borders

  **\--text-primary**            #CDD9E5 --- Main labels, commit messages, values

  **\--text-secondary**          #768390 --- Descriptions, metadata, timestamps

  **\--text-tertiary**           #444C56 --- Section labels, placeholder text, hints

  **\--text-dim**                #2D333B --- Disabled states, ghost text

  **\--blue / \--blue-bg**       #539BF5 / #1C2A3A --- Primary / constructive

  **\--green / \--green-bg**     #57AB5A / #1B2A1F --- Additive / stage

  **\--red / \--red-bg**         #E5534B / #2D1B1B --- Destructive

  **\--amber / \--amber-bg**     #C69026 / #2D2415 --- Caution / reversible

  **\--purple / \--purple-bg**   #986EE2 / #261E3A --- Tags / workspaces

  **\--lane-1 ... \--lane-7**    #539BF5 / #57AB5A / #C69026 / #986EE2 / #D4537E / #39C5CF / #D08444
  ------------------------------ ---------------------------------------------------------------------

**18. Remote Management**

Remote management does not have a dedicated screen. All remote operations are contextual to the Remotes section of the left sidebar.

**18.1 Sidebar Remotes section**

Lists all configured remotes for the active repository. Each remote row is expandable to show its tracking branches as sub-rows. Section header carries a + button for adding a new remote.

**18.2 Add remote (+ button)**

-   Small inline dialog: Name field (validated --- red border if name already exists), URL field (HTTPS or SSH). Green \"Add remote\" button. Cancel (neutral).

-   On confirm, remote is added via git remote add and appears in the sidebar immediately.

**18.3 Right-click context menu on a remote**

-   Fetch from this remote (neutral): runs git fetch \[remote\] immediately, no dialog. Inline spinner on the remote row confirms the operation.

-   Edit (amber): opens the edit dialog (§18.4).

-   Copy URL (neutral): copies the remote URL to clipboard. Transient \"Copied\" toast confirms.

-   Remove (red): confirmation dialog \"Remove remote \[name\]? This will not delete the remote repository.\" Red \"Remove remote\" button, neutral Cancel.

**18.4 Edit remote dialog**

-   Pre-filled with current name and URL. Both fields editable in a single dialog.

-   Amber \"Save changes\" button. Cancel (neutral).

-   SSL verification checkbox: \"Disable SSL verification for this remote\" (unchecked by default). When checked: red warning box appears. Trunk writes http.\[url\].sslVerify = false to repository git config, scoped to this remote\'s URL only --- not a global setting.

  -----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *There is no standalone Remote Manager screen. All remote operations live in the sidebar. Remotes are simple two-field objects (name + URL) and do not warrant a dedicated view.*

  -----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**19. Preferences Screen**

The preferences screen is a full-screen view replacing the main application window. Two-panel layout: left navigation column (176px, ten categories), right content panel showing settings for the active category. Opened via ⌘, / Ctrl+,, toolbar settings button, or sidebar preferences item.

**19.1 Layout**

-   Left navigation: flat single-level list, no sub-navigation. Active category highlighted with left 2px blue border accent.

-   Right content: settings organised into named card groups with section labels. Related settings grouped together.

-   Footer: Cancel (neutral) and Save (blue). Theme and zoom apply live for visual feedback. All other changes committed on Save.

**19.2 Categories and settings**

**19.2.1 Appearance**

-   Colour theme: dropdown --- Follow system / Dark / Light.

-   Custom theme file: path input + Browse + Clear. Hot-reloads on save.

-   UI zoom scale: slider 70--150% in 10% steps, default 100%.

-   Graph row height: slider 22--36px in 2px steps, default 28px.

-   Font size: dropdown --- 11px / 12px (default) / 13px / 14px.

**19.2.2 Identity & signing**

-   Author name (git config user.name) and author email (git config user.email): text inputs.

-   Info box: \"Per-workspace overrides in the .trunk file take precedence.\"

-   Sign commits with SSH: toggle (off by default). When on: signing key path field, signature program override field, and amber info box \"GPG signing is configured via \~/.gitconfig. Trunk does not manage GPG keys directly.\"

**19.2.3 Git behaviour**

-   Default pull strategy: dropdown --- Rebase (default) / Merge / Fast-forward only.

-   Default push behaviour: dropdown --- simple (default) / current / upstream.

-   Prune on fetch: toggle (on by default).

-   Auto-fetch interval: dropdown --- Off / 5 min / 10 min (default) / 30 min / 1 hour.

**19.2.4 Keyboard & input**

-   Default interaction mode: dropdown --- Keyboard-first (default) / Mouse-first / Custom.

-   Keyboard shortcuts: searchable table of all actions with current shortcut. \"Edit shortcuts...\" button opens full remapping editor.

**19.2.5 Terminal**

-   Default shell: dropdown --- System default / bash / zsh / fish / Custom path....

-   Terminal font: font name input + size dropdown. Terminal drawer default height: slider 80--400px, default 200px.

**19.2.6 Diff & staging**

-   Default diff view: dropdown --- Unified (default) / Split.

-   Syntax highlighting: toggle (on by default). Whitespace handling: dropdown --- Show all (default) / Ignore trailing / Ignore all.

**19.2.7 Tag manager**

-   Default grouping: dropdown --- None / Version series / Push status (global default; per-repo overrides in .trunk file).

-   Default version group pattern: monospaced text input. Default: \^(v\\d+)\\. with example hint.

**19.2.8 Workspace**

-   Re-open last session on launch: toggle (off by default).

-   Default workspace file location: path input + Browse button.

**19.2.9 Network & credentials**

-   SSH key path: path input + Browse. Credential helper: dropdown --- OS keychain (default) / Custom. HTTP proxy: text input (blank = system proxy).

-   Note: SSL verification is configured per-remote in the Edit Remote dialog (§18.4), not here.

**19.2.10 Updates**

-   Check for updates automatically: toggle (on by default). Update channel: dropdown --- Stable (default) / Pre-release.

-   Current version row with \"Check now\" blue button.

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *Every configurable option documented throughout this PRD maps to exactly one setting in §19. The Save / Cancel footer ensures no accidental changes --- only theme and zoom apply live as visual feedback.*

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**20. Multi-Repo & Workspaces**

Workspace architecture is fully documented in §15. This section captures the remaining multi-repo configuration details.

-   A workspace is a .trunk TOML file grouping repository paths with optional per-workspace config overrides.

-   Repos can appear in multiple workspaces. Workspaces can be renamed and deleted without affecting repositories.

-   Switching active repo: click in sidebar. Graph reloads instantly. User always lands on graph view of the new repo.

-   Recently opened repositories and workspaces are shown on the welcome screen (§15.1) and are accessible without opening a workspace.

**21. Plugin Architecture**

Trunk\'s plugin system allows forge-specific functionality to be added as separate open-source packages. This is the mechanism by which GitHub, GitLab, Bitbucket, and other hosting integrations will be delivered post-v1.

**21.1 Plugin scope (post-v1)**

-   Pull/Merge Request creation and review. Issue and project board linking.

-   CI/CD status per commit (build pass/fail badges in the graph). Hosted repository browsing and cloning.

-   Notifications and activity feeds.

**21.2 Plugin API principles**

-   Plugins are written in Rust or via a stable Wasm ABI. No arbitrary code execution in the main process without sandboxing.

-   Plugins declare required permissions at install time.

-   A community plugin registry will be established alongside the v1 release.

-   All first-party plugins (GitHub, GitLab, Bitbucket) will be open-source under the same licence as Trunk core.

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  *Plugin API design is out of scope for v1. The v1 deliverable is the core plugin host --- a stable, versioned API surface that plugins can target. First plugins will follow in a subsequent release.*

  --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

**22. Technical Constraints**

  ---------------------- --------------------------------------------------------------------------
  **Runtime**            Rust + Tauri 2.x

  **Platforms**          macOS 13+, Windows 10+, Linux (Debian/Ubuntu/Fedora)

  **Git engine**         libgit2 via git2-rs (primary); shell git for unsupported operations

  **UI rendering**       WebView (Tauri) for all UI; Rust backend for all Git operations

  **Terminal**           Native PTY via portable-pty crate

  **Credential store**   OS-native (Keychain on macOS, DPAPI on Windows, Secret Service on Linux)

  **Config format**      TOML --- stored in OS-standard config directory

  **Workspace files**    .trunk TOML files --- human-readable, committable, shareable

  **Licence**            MIT or Apache-2.0 (dual-licensed)
  ---------------------- --------------------------------------------------------------------------

**23. v1 Feature Checklist**

  --------------------------------------- --------------- --------------- -----------------------------------------
  **Feature**                             **Priority**    **Section**     **Notes**

  **Commit graph with branch lanes**      P0              §7              Virtualised, 60fps, 7 lane colours

  **Commit detail overlay**               P0              §4.3            Right overlay, no graph shrink

  **Hunk-level staging**                  P0              §8              Line-level granularity, ●/○ gutter

  **Commit (with amend)**                 P0              §8              SSH signing, no char limit

  **Branch create / checkout / delete**   P0              §13             Four separate dialogs

  **Merge**                               P0              §4.6            Fast-forward + merge commit

  **Inline conflict resolution**          P0              §9              3-panel + editable merged output

  **Push / fetch / pull**                 P0              §12             With remote auth, diverged warnings

  **Multi-repo workspaces**               P0              §15             Named .trunk files

  **Built-in terminal drawer**            P0              §4.5            Bottom overlay, ⌘\`

  **Light + dark theme**                  P0              §17             System default at install, CSS tokens

  **Welcome screen**                      P0              §15.1           Repo / workspace / clone / recent

  **Repository mode**                     P0              §15.3           No repos section, pin label

  **Workspace mode**                      P0              §15.4           .trunk file, + button, repo switching

  **Clone dialog**                        P0              §15.5           Two contexts, workspace option

  **Command palette**                     P1              §10             ⌘K, 50ms open, in-memory index

  **Interactive rebase**                  P1              §16             Full-screen takeover, drag to reorder

  **Stash management**                    P1              §11             Create, pop, apply, branch, drop

  **Tag management**                      P1              §14             Annotated + lightweight, grouping, push

  **Remote management**                   P1              §18             Sidebar contextual, no dedicated screen

  **Preferences screen**                  P1              §19             10 categories, CSS theme file support

  **Rename branch**                       P1              §13.3           Amber dialog, remote warning

  **Auto-fetch**                          P1              §19.2.3         Background, configurable interval

  **SSH key management**                  P1              §19.2.2         Configure in preferences

  **Git LFS support**                     P2              ---             Track, fetch, push

  **Submodule support**                   P2              ---             Init, update, status

  **Plugin host (API surface)**           P2              §21             No plugins ship in v1

  **Blame view**                          ---             ---             Deferred to v2
  --------------------------------------- --------------- --------------- -----------------------------------------

**24. Out of Scope for v1**

-   AI features of any kind --- deferred to v2.

-   Forge integrations (GitHub PRs, GitLab MRs, Bitbucket, etc.) --- deferred to plugin packages.

-   GPG commit signing --- deferred to v2. SSH signing is supported.

-   Blame view --- deferred to v2.

-   Custom theme editor or third-party theme packages beyond CSS file editing.

-   Mobile or web clients. SVN or Mercurial.

-   Auto-update (v1 ships manual updates; auto-update in v1.1).

**25. Open Questions**

-   Plugin ABI: Rust dylib, Wasm component model, or IPC-based? Requires a design spike before v1.1.

-   Credential storage for HTTPS remotes on Linux where Secret Service availability varies.

-   Trunk naming: \"Trunk\" overlaps with trunk.io (CI/CD tool). Trademark search required before public launch.

-   Interactive rebase paused-for-edit step: should Trunk surface this in the main window or open an editor?

*Trunk PRD v2.0 --- June 2026*
