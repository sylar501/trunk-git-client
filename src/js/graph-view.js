// Main graph view page controller (PRD §7, SPEC.md item 3) — mounted into `#graph-canvas`
// by `index-page.js`. Owns the toolbar/filter bar, the virtualised, recycled-row scroll
// container, and (SPEC.md item 4, PRD §4.3) the commit-detail overlay's open/update/dismiss
// wiring and its cherry-pick/revert/branch-from-here actions. Deliberately does NOT wire the
// "rebase" toolbar button (that's item 10) — still visual-only here.

import { openGraph, getGraphRows, getWorkingTreeStatus, getCommitIndex } from "./app.js";
import { createCommitRow, laneColumnWidth } from "../components/commit-row.js";
import { createCommitOverlay } from "../components/commit-overlay.js";
import { openCreateBranchDialog } from "./create-branch-dialog.js";
import { openSwitchBranchDialog } from "./switch-branch-dialog.js";
import { openConflictableActionDialog } from "./conflictable-action-dialog.js";
import { openPushDialog } from "./push-dialog.js";
import { openFetchDialog } from "./fetch-dialog.js";
import { openPullDialog } from "./pull-dialog.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const OVERLAY_MIN_WIDTH = 220;
const OVERLAY_MAX_WIDTH = 480;

const ROW_HEIGHT = 28; // px — mirrors --row-height; the 22-36px slider is item 14
const OVERSCAN = 10;
const DEBOUNCE_MS = 150;

function debounce(fn, ms) {
  let handle;
  return (...args) => {
    clearTimeout(handle);
    handle = setTimeout(() => fn(...args), ms);
  };
}

/**
 * @param {HTMLElement} canvas
 * @param {{
 *   onMutated?: () => Promise<void> | void,
 *   overlayWidth?: number - persisted commit-overlay width (px) from `getSettings()`,
 *     defaults to 264 if not supplied.
 *   onOverlayResize?: (width: number) => void - fired once per completed drag (not per
 *     mousemove) so the caller can persist it via `saveSettings()`.
 *   conflicted?: boolean - this repo has an unresolved conflict (PRD §4.6/§9, SPEC.md item 6),
 *     i.e. `appState.conflict_resolution_in_progress` for the active repo. Swaps the toolbar's
 *     "Stage changes" button for "Resolve conflicts" — staging doesn't make sense with unmerged
 *     paths in the index. There's no auto-redirect into the resolver anywhere, including right
 *     after the cherry-pick/revert action that produces a fresh conflict — see
 *     `conflictable-action-dialog.js`'s conflict-choice dialog, which lets the user pick
 *     resolve-now vs abort instead. This button is the persistent, user-invoked way back in.
 * }} opts - `onMutated` is called after a successful cherry-pick/revert/branch-from-here so the
 *   caller can refresh sidebar+graph together (see `index-page.js`) — one shared refresh path
 *   for all three, rather than this module guessing which mutation needs which slice of a
 *   refresh.
 */
export async function mountGraph(canvas, repoPath, { onMutated, overlayWidth: initialOverlayWidth, onOverlayResize, conflicted = false } = {}) {
  // `mountGraph` can be called again on the same `canvas` (every refresh remounts) — abort the
  // previous call's document-level listeners (Escape, outside-click) before attaching new ones,
  // since those aren't scoped to anything `canvas.innerHTML` below would otherwise clean up.
  canvas._cdoAbortController?.abort();
  const abortController = new AbortController();
  canvas._cdoAbortController = abortController;
  const { signal } = abortController;

  canvas.innerHTML = `
    <div class="graph-toolbar">
      <input class="search-input" id="g-search" placeholder="filter commits…" />
      <div class="fpill active" data-quick="all">all branches</div>
      <div class="fpill" data-quick="mine">mine</div>
      <div class="fpill" data-quick="week">this week</div>
      <div class="fpill" id="g-toggle-filters">filters</div>
      <div class="tb-spacer"></div>
      <div class="btn btn-blue" id="g-push">Push</div>
      <div class="btn btn-neutral" id="g-fetch">Fetch</div>
      <div class="btn btn-amber" id="g-pull">Pull</div>
      ${
        conflicted
          ? '<div class="btn btn-amber" id="g-stage">Resolve conflicts</div>'
          : '<div class="btn btn-green disabled" id="g-stage">Stage changes</div>'
      }
      <div class="btn btn-neutral disabled" id="g-rebase">rebase</div>
    </div>
    <div class="tb-filters" id="g-filters" hidden>
      <input class="inp" id="f-author" placeholder="author" />
      <input class="inp" id="f-branch" placeholder="branch" />
      <input class="inp" id="f-path" placeholder="path" />
    </div>
    <div class="graph-body" id="g-body" tabindex="0">
      <div class="graph-sizer" id="g-sizer"></div>
    </div>
  `;

  const body = canvas.querySelector("#g-body");
  const sizer = canvas.querySelector("#g-sizer");
  const searchInput = canvas.querySelector("#g-search");
  const filtersBar = canvas.querySelector("#g-filters");
  const authorInput = canvas.querySelector("#f-author");
  const branchInput = canvas.querySelector("#f-branch");
  const pathInput = canvas.querySelector("#f-path");

  // Commit detail overlay (PRD §4.3, SPEC.md item 4) — a sibling of `sizer`, not a child of it,
  // so it pins to `body`'s visible viewport instead of scrolling away with the recycled row
  // pool (see commit-overlay.js's own header comment for the full reasoning).
  //
  // Both actions open a confirm dialog (commit details + a "--no-commit" option) rather than
  // running immediately, and a `conflict` outcome opens a second "resolve now or abort" choice
  // instead of silently auto-navigating into the resolver — see
  // `conflictable-action-dialog.js`. Fire-and-forget, same convention as `onBranchFromHere`
  // below: the dialog only resolves once something actually happened, never on Cancel.
  async function onCherryPick({ sha }) {
    openConflictableActionDialog({ kind: "cherry-pick", repoPath, sha, onMutated });
  }

  async function onRevert({ sha }) {
    openConflictableActionDialog({ kind: "revert", repoPath, sha, onMutated });
  }

  // Deliberately not `await`ed below the dialog-open call: `openCreateBranchDialog` only
  // resolves on success and never on cancel (same convention as `openCloneDialog`), so awaiting
  // it here would leave the overlay's own buttons disabled forever if the user cancels — the
  // overlay should only stay disabled for the brief synchronous "open the dialog" step.
  async function onBranchFromHere({ sha, shortSha, summary }) {
    // The dialog itself already shows the create/checkout-failure toast (see
    // create-branch-dialog.js's `submit()`) — this just refreshes once it's done.
    openCreateBranchDialog({ sha, shortSha, summary, repoPath, onMutated })
      .then(async (result) => {
        if (!result?.created) return;
        await onMutated?.();
      })
      .catch((err) => {
        showToast({ variant: "danger", message: String(err) });
      });
  }

  const overlay = createCommitOverlay({ onCherryPick, onRevert, onBranchFromHere });
  let overlayWidth = initialOverlayWidth || 264;
  overlay.el.style.width = `${overlayWidth}px`;
  body.append(overlay.el);

  // `.cdo` is `position:fixed` (see components.css) — its on-screen rect is computed here from
  // `body`'s own measured box, rather than via CSS `top/right/bottom` against `body` as the
  // containing block, which measurably tracked scroll position instead of staying pinned on the
  // Linux/WebKitGTK build. Re-synced on every resize of `body` (covers window resize and the
  // filters bar toggling, which changes `body`'s height/top within the toolbar's flex column).
  function syncOverlayPosition() {
    const rect = body.getBoundingClientRect();
    overlay.el.style.top = `${rect.top}px`;
    overlay.el.style.height = `${rect.height}px`;
    overlay.el.style.left = `${rect.right - overlayWidth}px`;
  }
  syncOverlayPosition();
  const positionObserver = new ResizeObserver(syncOverlayPosition);
  positionObserver.observe(body);
  signal.addEventListener("abort", () => positionObserver.disconnect());

  // Handle is on the overlay's *left* edge, so dragging left (not right) grows it — `invert`.
  attachResizeHandle(overlay.el.querySelector(".cdo-resize-handle"), {
    getWidth: () => overlayWidth,
    setWidth: (w) => {
      overlayWidth = w;
      overlay.el.style.width = `${w}px`;
      syncOverlayPosition();
    },
    min: OVERLAY_MIN_WIDTH,
    max: OVERLAY_MAX_WIDTH,
    invert: true,
    onResizeEnd: (finalWidth) => onOverlayResize?.(finalWidth),
    signal,
  });

  // Centralised here (not self-attached inside commit-overlay.js) because closing on *any*
  // outside click would race with clicking a different commit row: mousedown fires before
  // click, so a naive overlay-owned listener would close it before this module's own `.crow`
  // click handler (below) gets a chance to just update the overlay's contents in place.
  // Capture-phase to match `dialog.js`/`context-menu.js`'s existing convention.
  document.addEventListener(
    "mousedown",
    (e) => {
      if (!overlay.isOpen()) return;
      if (overlay.el.contains(e.target)) return;
      if (e.target.closest(".crow")) return;
      overlay.close();
    },
    { capture: true, signal }
  );
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "Escape" || !overlay.isOpen()) return;
      overlay.close();
    },
    { capture: true, signal }
  );

  // ⌘⇧S / Ctrl+Shift+S → staging view (PRD §4.4, §6, SPEC.md item 5) — the first real
  // keybinding in this codebase beyond per-view Escape, so there's no existing Mac/non-Mac
  // normalization convention to copy; just accept either modifier key. While conflicted, this
  // shortcut is a no-op — the button it would otherwise activate is "Resolve conflicts", not
  // "Stage changes", and staging doesn't make sense with unmerged paths in the index anyway.
  // Push/Fetch/Pull (PRD §12, SPEC.md item 7) — fire-and-forget, same convention as the
  // cherry-pick/revert/branch-from-here dialogs above: each dialog only resolves once something
  // actually happened, never on Cancel/Escape, and `onMutated` refreshes sidebar+graph together.
  canvas.querySelector("#g-push").addEventListener("click", () => openPushDialog({ repoPath, onMutated }), { signal });
  canvas.querySelector("#g-fetch").addEventListener("click", () => openFetchDialog({ repoPath, onMutated }), { signal });
  canvas.querySelector("#g-pull").addEventListener("click", () => openPullDialog({ repoPath, onMutated }), { signal });

  const stageBtn = canvas.querySelector("#g-stage");
  let hasPendingChanges = false;

  function goToStaging() {
    if (conflicted || !hasPendingChanges) return;
    window.location.href = "staging.html";
  }
  stageBtn.addEventListener("click", conflicted ? () => (window.location.href = "resolve.html") : goToStaging, {
    signal,
  });
  document.addEventListener(
    "keydown",
    (e) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        goToStaging();
      }
    },
    { signal }
  );

  // ⌘⇧B / ⌘B → Create/Switch branch (PRD §6/§13, SPEC.md item 8) — same fire-and-forget
  // convention as the cherry-pick/revert/branch-from-here dialogs above: only `onCreated`/
  // `onSwitched` resolves, never Cancel/Escape, and `onMutated` refreshes sidebar+graph together.
  // ⌘⇧B is checked first since `e.key` for Shift+B is "B", which would otherwise also match a
  // bare ⌘B check.
  document.addEventListener(
    "keydown",
    (e) => {
      if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "b") return;
      e.preventDefault();
      if (e.shiftKey) {
        // The dialog itself already shows the create/checkout-failure toast.
        openCreateBranchDialog({ repoPath, onMutated })
          .then(async (result) => {
            if (!result?.created) return;
            await onMutated?.();
          })
          .catch((err) => showToast({ variant: "danger", message: String(err) }));
      } else {
        openSwitchBranchDialog({ repoPath, onMutated }).then((result) => {
          if (!result?.switched) return;
          showToast({ variant: "success", message: `Switched to ${result.name}.` });
        });
      }
    },
    { signal }
  );

  // Reflects the working tree's current file count on the button itself — "stage changes
  // (1432)" — disabled at zero so there's nothing to navigate to. No file-watcher yet (same
  // limitation noted elsewhere in this codebase), so this is a one-shot check on mount/refresh,
  // not a live count. Skipped entirely while conflicted — the button isn't "Stage changes" and
  // doesn't need a pending-file count.
  if (!conflicted) {
    getWorkingTreeStatus(repoPath)
      .then((status) => {
        const count = status.files.length;
        hasPendingChanges = count > 0;
        stageBtn.textContent = count > 0 ? `Stage changes (${count})` : "Stage changes";
        stageBtn.classList.toggle("disabled", !hasPendingChanges);
      })
      .catch(() => {
        hasPendingChanges = false;
        stageBtn.classList.add("disabled");
      });
  }

  let totalCount = 0;
  let filter = {};
  let selectedIndex = -1;
  let cache = { start: 0, rows: [] };
  let firstIndex = 0;
  let rafScheduled = false;
  let loadGeneration = 0;

  // The walk + lane-assignment pass behind `openGraph` is O(history size) — fast on a small
  // repo, but a large/busy one (thousands of merges, deep history) can take real seconds.
  // Show a spinner immediately rather than leaving an empty body with no feedback.
  const loadingEl = document.createElement("div");
  loadingEl.className = "loading-center";
  loadingEl.innerHTML = `<div class="spinner lg"></div><div>Loading commit graph…</div>`;
  body.append(loadingEl);

  let meta;
  try {
    meta = await openGraph(repoPath);
  } catch (err) {
    loadingEl.innerHTML = `<div>Couldn't load commit graph: ${String(err)}</div>`;
    return;
  }
  loadingEl.remove();

  totalCount = meta.total_count;
  sizer.style.height = `${totalCount * ROW_HEIGHT}px`;

  if (totalCount === 0) {
    body.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;text-align:center;">No commits yet.</div>`;
    return;
  }

  const poolSize = Math.max(1, Math.ceil(body.clientHeight / ROW_HEIGHT)) + OVERSCAN * 2;
  const pool = Array.from({ length: poolSize }, () => createCommitRow({ rowHeight: ROW_HEIGHT }));
  for (const r of pool) sizer.append(r.el);

  function renderWindow() {
    for (let i = 0; i < pool.length; i++) {
      const row = cache.rows[i];
      if (!row) {
        pool[i].el.style.display = "none";
        continue;
      }
      const globalIndex = cache.start + i;
      pool[i].el.style.display = "";
      pool[i].update(row, {
        selected: globalIndex === selectedIndex,
        dimmed: !row.matches,
        top: globalIndex * ROW_HEIGHT,
        index: globalIndex,
        gcolWidth: cache.gcolWidth,
      });
    }
  }

  // Lane column width is computed per loaded window, not for the whole history: a busy repo's
  // all-time peak concurrent-lane count can be huge in one stretch and tiny everywhere else —
  // sizing every row off that global peak wastes most of the row on empty space and crushes
  // the message column. Within one fetched window the width is still consistent across rows
  // (fixing the original per-row-independent-width overflow/misalignment bug).
  function windowLaneWidth(rows) {
    let maxLane = 0;
    for (const row of rows) {
      maxLane = Math.max(maxLane, row.lane, ...row.through_lanes.map((t) => t.lane), ...row.connectors.map((c) => c.lane));
    }
    return laneColumnWidth(maxLane);
  }

  async function loadWindow(start) {
    const generation = ++loadGeneration;
    const clampedStart = Math.max(0, Math.min(start, Math.max(0, totalCount - poolSize)));
    const rows = await getGraphRows(repoPath, clampedStart, poolSize, filter);
    if (generation !== loadGeneration) return; // a newer load superseded this one
    cache = { start: clampedStart, rows, gcolWidth: windowLaneWidth(rows) };
    renderWindow();
  }

  function scheduleScrollLoad() {
    const visibleFirst = Math.max(0, Math.floor(body.scrollTop / ROW_HEIGHT) - OVERSCAN);
    if (visibleFirst === firstIndex) return;
    firstIndex = visibleFirst;
    loadWindow(firstIndex);
  }

  body.addEventListener("scroll", () => {
    if (rafScheduled) return;
    rafScheduled = true;
    requestAnimationFrame(() => {
      rafScheduled = false;
      scheduleScrollLoad();
    });
  });

  sizer.addEventListener("click", (e) => {
    const rowEl = e.target.closest(".crow");
    if (!rowEl) return;
    selectedIndex = Number(rowEl.dataset.index);
    renderWindow();
    overlay.open(rowEl.dataset.sha, repoPath);
  });

  function scrollToIndex(index) {
    const viewTop = body.scrollTop;
    const viewBottom = viewTop + body.clientHeight;
    const rowTop = index * ROW_HEIGHT;
    const rowBottom = rowTop + ROW_HEIGHT;
    if (rowTop < viewTop) {
      body.scrollTop = rowTop;
    } else if (rowBottom > viewBottom) {
      body.scrollTop = rowBottom - body.clientHeight;
    }
  }

  // Arrow keys only ever move `selectedIndex` — they deliberately never open or update the
  // overlay, even if one is already open. Enter/Space is the separate, explicit "open commit
  // detail" trigger. This is the literal reading of PRD §6's keyboard-profile table, which
  // lists "Navigate commits" (↑↓) and "Open commit detail" (Enter/Space) as two separate rows,
  // not arrow-navigation implicitly opening/following with the overlay.
  body.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const delta = e.key === "ArrowDown" ? 1 : -1;
      const base = selectedIndex < 0 ? firstIndex : selectedIndex;
      selectedIndex = Math.max(0, Math.min(totalCount - 1, base + delta));
      scrollToIndex(selectedIndex);
      firstIndex = Math.max(0, Math.floor(body.scrollTop / ROW_HEIGHT) - OVERSCAN);
      loadWindow(firstIndex);
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      if (selectedIndex < 0) return;
      const row = cache.rows[selectedIndex - cache.start];
      if (!row) return; // selection raced ahead of the in-flight window load — no-op, not a crash
      e.preventDefault();
      overlay.open(row.sha, repoPath);
    }
  });

  const reloadFiltered = debounce(() => loadWindow(cache.start), DEBOUNCE_MS);

  function setQuickFilter(name) {
    canvas.querySelectorAll(".fpill[data-quick]").forEach((p) => p.classList.toggle("active", p.dataset.quick === name));
    delete filter.author;
    delete filter.date_from;
    if (name === "mine") {
      const myEmail = cache.rows.find((r) => r.is_head)?.author_email || cache.rows[0]?.author_email;
      if (myEmail) filter.author = myEmail;
    } else if (name === "week") {
      filter.date_from = Math.floor(Date.now() / 1000) - 7 * 24 * 60 * 60;
    }
    loadWindow(0);
  }

  canvas.querySelectorAll(".fpill[data-quick]").forEach((pill) => {
    pill.addEventListener("click", () => setQuickFilter(pill.dataset.quick));
  });

  canvas.querySelector("#g-toggle-filters").addEventListener("click", () => {
    filtersBar.hidden = !filtersBar.hidden;
  });

  authorInput.addEventListener("input", () => {
    filter.author = authorInput.value.trim() || undefined;
    reloadFiltered();
  });
  branchInput.addEventListener("input", () => {
    filter.branch = branchInput.value.trim() || undefined;
    reloadFiltered();
  });
  pathInput.addEventListener("input", () => {
    filter.path = pathInput.value.trim() || undefined;
    reloadFiltered();
  });

  searchInput.addEventListener("input", () => {
    const value = searchInput.value.trim();
    if (!value) {
      filter.message = undefined;
      filter.sha_prefix = undefined;
    } else if (/^[0-9a-f]{4,40}$/i.test(value)) {
      filter.sha_prefix = value;
      filter.message = undefined;
    } else {
      filter.message = value;
      filter.sha_prefix = undefined;
    }
    reloadFiltered();
  });

  // Lets callers outside this module (the command palette's commit search, via
  // `index-page.js`) jump straight to a result instead of reloading the whole page just to
  // re-show the graph it's already showing. `getCommitIndex` looks up the row index in the
  // same server-side cache `loadWindow` already slices, so this is a real jump-to-row, not a
  // filter-and-hope.
  async function goToCommit(sha) {
    const index = await getCommitIndex(repoPath, sha).catch(() => null);
    if (index == null) {
      showToast({ variant: "danger", message: "That commit isn't in the currently-loaded graph." });
      return;
    }
    selectedIndex = index;
    body.scrollTop = Math.max(0, index * ROW_HEIGHT - body.clientHeight / 2);
    firstIndex = Math.max(0, Math.floor(body.scrollTop / ROW_HEIGHT) - OVERSCAN);
    await loadWindow(firstIndex);
    overlay.open(sha, repoPath);
  }

  await loadWindow(0);
  return { goToCommit };
}
