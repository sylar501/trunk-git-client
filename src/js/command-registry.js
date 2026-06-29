// Builds the command palette's inventory for a given context (PRD §10.6, SPEC.md item 9).
// Knows nothing about the palette's UI (`command-palette.js`) — just turns whatever a host
// page already has lying around (repo path, app state, its own add/clone/navigate handlers)
// into a flat list of `{ id, label, description, scope, iconVariant, icon, shortcutLabel,
// disabled, run }` rows plus the dynamic branch/commit groups from §10.2. Author and file
// search are both deferred (see SPEC.md's "Future ideas #1" and "#2") — neither has anywhere
// useful to land yet: no contributor view for an author result, and file results could only
// ever cover *uncommitted* files (`getWorkingTreeStatus` has no other file listing to query),
// which is too narrow to call "file search" without saying so loudly in the UI somewhere.
//
// Several §10.6 rows have no backend/dialog yet (merge branch, rebase, stash, tag, terminal,
// theme, zoom) — shipped here as visible-but-disabled so the palette still doubles as the
// shortcut reference card §10.3 describes; flip `disabled` off in place once each lands.
//
// Intentionally NOT included (see SPEC.md item 9's revision note): rename/delete branch
// (sidebar context menu only per item 8's override of this section's literal §10.6 wording),
// cherry-pick/revert (need a specific target commit a text search has no way to supply —
// stay on the commit-detail overlay), switch repository (Workspace mode's per-repo list would
// just duplicate the sidebar's own one-click rows with more noise, unlike branches/files/
// commits where the search shortcut earns its keep).

import { openCommandPalette } from "../components/command-palette.js";
import { listBranches, getWorkingTreeStatus, getGraphRows, checkoutBranch, promoteToWorkspace } from "./app.js";
import { openCreateBranchDialog } from "./create-branch-dialog.js";
import { openSwitchBranchDialog } from "./switch-branch-dialog.js";
import { openPushDialog } from "./push-dialog.js";
import { openFetchDialog } from "./fetch-dialog.js";
import { openPullDialog } from "./pull-dialog.js";
import { confirmDirtyTreeStrategy } from "./branch-dialog-shared.js";
import { promptWorkspaceDetails } from "./workspace-prompt.js";
import { showToast } from "../components/toast.js";

const SHA_PREFIX_RE = /^[0-9a-f]{4,40}$/i;
// `get_graph_rows` returns rows for a *position* window with a `matches` flag set per-row
// (PRD §7.3 — it dims non-matches in the graph rather than dropping them, since the graph
// needs to preserve topology). The command palette has no topology to preserve, so it scans
// a window this much deeper than what it displays, filters to actual matches, then caps the
// list — searching by position alone (the graph's own behaviour) would mostly return
// coincidental position hits rather than real text matches.
const COMMIT_SEARCH_SCAN_DEPTH = 2000;
const COMMIT_SEARCH_MAX_RESULTS = 20;
const COMMIT_SEARCH_MIN_QUERY_LENGTH = 3;

const RELATIVE_TIME_UNITS = [
  ["year", 31536000],
  ["month", 2592000],
  ["week", 604800],
  ["day", 86400],
  ["hour", 3600],
  ["minute", 60],
];

/** "3 hours ago"/"1 week ago" style — long-form, unlike `commit-row.js`'s/`welcome.js`'s own
 * abbreviated ("3h"/"1d ago") relative-time helpers, which read fine in a tight row but too
 * clipped for a result description here. */
function relativeTimeLong(epochSeconds) {
  const deltaSeconds = Math.max(0, Math.floor(Date.now() / 1000) - epochSeconds);
  for (const [unit, seconds] of RELATIVE_TIME_UNITS) {
    const value = Math.floor(deltaSeconds / seconds);
    if (value >= 1) return `${value} ${unit}${value === 1 ? "" : "s"} ago`;
  }
  return "just now";
}

function comingLater(label, description = "Coming in a later release.") {
  return { disabled: true, label, description };
}

/** Mirrors `sidebar.js`'s direct branch-row checkout — dirty-tree confirm, then checkout. */
async function switchToBranch(repoPath, name, onMutated) {
  let dirty = false;
  try {
    dirty = (await getWorkingTreeStatus(repoPath)).files.length > 0;
  } catch {
    dirty = false;
  }
  let dirtyStrategy;
  if (dirty) {
    dirtyStrategy = await confirmDirtyTreeStrategy("Switch");
    if (!dirtyStrategy) return;
  }
  try {
    await checkoutBranch(repoPath, name, { dirtyStrategy });
    await onMutated?.();
    showToast({ variant: "success", message: `Switched to ${name}.` });
  } catch (err) {
    showToast({ variant: "danger", message: String(err) });
  }
}

/**
 * @param {{
 *   repoPath: string,
 *   appState: object,
 *   onMutated: () => Promise<void> | void,
 *   onAddExisting: () => Promise<void> | void,
 *   onCloneNew: () => Promise<void> | void,
 *   goToStaging: () => void,
 *   goToHistory: () => void,
 * }} ctx
 * @returns {Array<object>} static commands (all scoped "commands" — see `command-palette.js`'s
 *   header comment for why scope is by result type, not §10.6's command category), grouped
 *   by §10.6's original Git/Navigate/View/Repos categories purely for readability below
 */
function staticCommands(ctx) {
  const { repoPath, appState, onMutated, onAddExisting, onCloneNew, goToStaging, goToHistory } = ctx;
  const inWorkspace = appState?.mode === "workspace";
  const onGraph = location.pathname.endsWith("index.html");
  const onStaging = location.pathname.endsWith("staging.html");

  return [
    // --- Git ---
    {
      id: "create-branch",
      scope: "commands",
      iconVariant: "green",
      icon: "+",
      label: "Create branch…",
      description: "Create a new branch",
      shortcutLabel: "⌘⇧B",
      run: () =>
        openCreateBranchDialog({ repoPath, onMutated })
          .then((result) => result?.created && onMutated?.())
          .catch((err) => showToast({ variant: "danger", message: String(err) })),
    },
    {
      id: "switch-branch",
      scope: "commands",
      iconVariant: "blue",
      icon: "⌥",
      label: "Switch branch…",
      description: "Switch to a different branch",
      shortcutLabel: "⌘B",
      run: () =>
        openSwitchBranchDialog({ repoPath, onMutated }).then((result) => {
          if (!result?.switched) return;
          showToast({ variant: "success", message: `Switched to ${result.name}.` });
        }),
    },
    {
      id: "merge-branch",
      scope: "commands",
      iconVariant: "neutral",
      icon: "⇄",
      ...comingLater("Merge branch…"),
    },
    {
      id: "push",
      scope: "commands",
      iconVariant: "blue",
      icon: "↑",
      label: "Push",
      description: "Push commits to the remote",
      shortcutLabel: "⌘P",
      run: () => openPushDialog({ repoPath, onMutated }),
    },
    {
      id: "fetch",
      scope: "commands",
      iconVariant: "neutral",
      icon: "↓",
      label: "Fetch",
      description: "Fetch from the remote",
      shortcutLabel: "⌘F",
      run: () => openFetchDialog({ repoPath, onMutated }),
    },
    {
      id: "pull",
      scope: "commands",
      iconVariant: "amber",
      icon: "⇄",
      label: "Pull",
      description: "Pull from the remote",
      shortcutLabel: "⌘⇧P",
      run: () => openPullDialog({ repoPath, onMutated }),
    },
    { id: "rebase", scope: "commands", iconVariant: "purple", icon: "↕", ...comingLater("Interactive rebase…") },
    { id: "stash", scope: "commands", iconVariant: "amber", icon: "▤", ...comingLater("Stash changes…") },
    { id: "pop-stash", scope: "commands", iconVariant: "green", icon: "▤", ...comingLater("Pop stash") },
    { id: "create-tag", scope: "commands", iconVariant: "purple", icon: "◆", ...comingLater("Create tag…") },

    // --- Navigate ---
    {
      id: "open-staging",
      scope: "commands",
      iconVariant: "green",
      icon: "▦",
      label: "Open staging view",
      description: "Stage changes and commit",
      shortcutLabel: "⌘⇧S",
      disabled: !!appState?.conflict_resolution_in_progress || onStaging,
      run: () => goToStaging?.(),
    },
    {
      id: "go-to-history",
      scope: "commands",
      iconVariant: "blue",
      icon: "≡",
      label: "Go to history",
      description: "Back to the commit graph",
      disabled: onGraph,
      run: () => goToHistory?.(),
    },
    { id: "toggle-terminal", scope: "commands", iconVariant: "neutral", icon: "▢", ...comingLater("Toggle terminal", "Coming with the terminal drawer."), shortcutLabel: "⌘`" },
    {
      id: "switch-workspace",
      scope: "commands",
      iconVariant: "purple",
      icon: "⌂",
      label: "Switch workspace…",
      description: "Back to the workspace/repository picker",
      run: () => {
        // Same flag `sidebar.js`'s "Open another repository or workspace…" sets — without it
        // welcome.js's fast path (PRD §15.1) immediately reopens this same repo/workspace and
        // the welcome screen never actually shows.
        sessionStorage.setItem("trunk-skip-fast-path", "1");
        window.location.href = "welcome.html";
      },
    },

    // --- View ---
    { id: "switch-theme", scope: "commands", iconVariant: "neutral", icon: "◐", ...comingLater("Switch theme") },
    { id: "zoom-in", scope: "commands", iconVariant: "neutral", icon: "+", ...comingLater("Increase UI zoom"), shortcutLabel: "⌘+" },
    { id: "zoom-out", scope: "commands", iconVariant: "neutral", icon: "−", ...comingLater("Decrease UI zoom"), shortcutLabel: "⌘−" },
    { id: "zoom-reset", scope: "commands", iconVariant: "neutral", icon: "↺", ...comingLater("Reset UI zoom"), shortcutLabel: "⌘0" },

    // --- Repos ---
    {
      id: "add-repository",
      scope: "commands",
      iconVariant: "green",
      icon: "+",
      label: "Add repository…",
      description: "Add an existing repository to this workspace",
      disabled: !inWorkspace,
      run: () => onAddExisting?.(),
    },
    {
      id: "clone-repository",
      scope: "commands",
      iconVariant: "blue",
      icon: "⇣",
      label: "Clone repository…",
      description: "Clone a remote repository",
      disabled: !inWorkspace,
      run: () => onCloneNew?.(),
    },
    {
      id: "promote-to-workspace",
      scope: "commands",
      iconVariant: "purple",
      icon: "⌂",
      label: "Promote to workspace…",
      description: "Turn this repository into a workspace",
      disabled: inWorkspace,
      run: () =>
        promptWorkspaceDetails({
          title: "Promote to workspace",
          submit: (name, directory) => promoteToWorkspace(name, directory),
        }).then((result) => {
          if (!result) return;
          showToast({ variant: "success", message: `Promoted to workspace "${result.workspace.name}".` });
          onMutated?.();
        }),
    },
    {
      id: "repository-settings",
      scope: "commands",
      iconVariant: "neutral",
      icon: "⚙",
      label: "Repository settings…",
      description: "Per-repository preferences",
      run: () => {
        window.location.href = "preferences.html";
      },
    },
    {
      id: "preferences",
      scope: "commands",
      iconVariant: "neutral",
      icon: "⚙",
      label: "Preferences…",
      description: "Application preferences",
      shortcutLabel: "⌘,",
      run: () => {
        window.location.href = "preferences.html";
      },
    },
  ];
}

/** Branches floated to the top of search results (§10.2) — one `listBranches` call, reused
 * for every keystroke rather than re-fetched, satisfying §10.5's "never touches git per
 * keystroke" for this group. */
async function branchCommands(ctx) {
  const branches = await listBranches(ctx.repoPath).catch(() => []);
  return branches.map((b) => ({
    id: `branch:${b.name}`,
    scope: "branches",
    iconVariant: b.is_head ? "blue" : "neutral",
    icon: "⎇",
    label: b.name,
    description: b.is_head ? "Current branch" : "Switch to this branch",
    disabled: b.is_head,
    run: () => switchToBranch(ctx.repoPath, b.name, ctx.onMutated),
  }));
}

/**
 * Commit search (§10.2) — unlike branches/files above, this one *does* touch git per query
 * (via the same `getGraphRows` call the graph's own filter bar already uses, SHA-prefix-vs-
 * message heuristic included). Debounced as a deliberate, documented exception to §10.5's
 * index-only requirement: no commit index exists anywhere in this codebase yet, and building
 * one is out of scope for this session — see SPEC.md item 9's note.
 *
 * §10.2 also calls for author search, but there's nowhere for an author result to go yet — no
 * dedicated user/contributor view exists (see SPEC.md's "Future ideas #1"). Surfacing author
 * rows that just dump you back on the graph isn't worth the confusion, so this only returns
 * commits for now; add authors back once that view lands.
 */
function commitSearchCommands(ctx, query) {
  if (query.length < COMMIT_SEARCH_MIN_QUERY_LENGTH) return Promise.resolve([]);
  const filter = SHA_PREFIX_RE.test(query) ? { sha_prefix: query } : { message: query };
  return getGraphRows(ctx.repoPath, 0, COMMIT_SEARCH_SCAN_DEPTH, filter)
    .then((allRows) => {
      const rows = allRows.filter((r) => r.matches).slice(0, COMMIT_SEARCH_MAX_RESULTS);
      return rows.map((r) => ({
        id: `commit:${r.sha}`,
        scope: "commits",
        iconVariant: "neutral",
        icon: "●",
        label: `${r.short_sha}  ${r.summary}`,
        description: `${r.author_name}, ${relativeTimeLong(r.time)} — view in graph`,
        // `ctx.goToCommit` is only present on index.html (the graph that can actually honour
        // a jump is already mounted there — see `index-page.js`). On staging/resolve, where
        // there's no graph to jump within, stash the target and navigate; `index-page.js`
        // consumes the same key right after its graph mounts.
        run: () => {
          if (ctx.goToCommit) {
            ctx.goToCommit(r.sha);
          } else {
            sessionStorage.setItem("trunk-goto-commit", r.sha);
            ctx.goToHistory?.();
          }
        },
      }));
    })
    .catch(() => []);
}

let openPalette = null;

/**
 * Wires ⌘K (open/close toggle) and the §6 shortcut→command-id map into the host page.
 * `getCtx()` is called fresh on every shortcut press / palette open so the inventory always
 * reflects the latest active repo/mode (§10's "rebuilt on repo/workspace switch") without
 * needing a re-mount on every `refresh()`.
 * @param {() => object} getCtx
 * @param {{ signal?: AbortSignal }} [opts]
 */
export function mountCommandPalette(getCtx, { signal } = {}) {
  const SHORTCUT_IDS = { p: "push", f: "fetch" };
  const SHIFT_SHORTCUT_IDS = { p: "pull", r: "rebase" };

  // Branches are cheap, fully in-memory once fetched, and not query-dependent — built up
  // front (§10.5 "index-only"). Commits are query-dependent and go through
  // `command-palette.js`'s own debounced `fetchDynamic` hook instead (see that module's
  // header comment for why this one group is a deliberate exception).
  async function commandsFor() {
    const ctx = getCtx();
    if (!ctx.repoPath) return staticCommands(ctx);
    const branches = await branchCommands(ctx);
    return [...staticCommands(ctx), ...branches];
  }

  function toggle() {
    if (openPalette) {
      openPalette.close();
      return;
    }
    commandsFor().then((commands) => {
      openPalette = openCommandPalette(commands, {
        onClose: () => (openPalette = null),
        fetchDynamic: (query) => commitSearchCommands(getCtx(), query),
        dynamicMinQueryLength: COMMIT_SEARCH_MIN_QUERY_LENGTH,
        dynamicHint: `Type at least ${COMMIT_SEARCH_MIN_QUERY_LENGTH} characters to start searching.`,
      });
    });
  }

  document.addEventListener(
    "keydown",
    (e) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      const key = e.key.toLowerCase();
      if (key === "k") {
        e.preventDefault();
        toggle();
        return;
      }
      const id = e.shiftKey ? SHIFT_SHORTCUT_IDS[key] : SHORTCUT_IDS[key];
      if (!id) return;
      e.preventDefault();
      commandsFor().then((commands) => {
        const command = commands.find((c) => c.id === id);
        if (!command) return;
        if (command.disabled) {
          showToast({ variant: "info", message: `${command.label} isn't available yet.` });
          return;
        }
        command.run();
      });
    },
    { signal }
  );
}
