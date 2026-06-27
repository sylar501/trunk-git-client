// Main graph view page controller (PRD §7, SPEC.md item 3) — mounted into `#graph-canvas`
// by `index-page.js`. Owns the toolbar/filter bar and the virtualised, recycled-row scroll
// container. Deliberately does NOT open a commit-detail overlay on row click (that's item 4)
// and does NOT wire the "rebase" toolbar button (item 10) — both are visual-only here.

import { openGraph, getGraphRows } from "./app.js";
import { createCommitRow, laneColumnWidth } from "../components/commit-row.js";

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

/** @param {HTMLElement} canvas */
export async function mountGraph(canvas, repoPath) {
  canvas.innerHTML = `
    <div class="graph-toolbar">
      <input class="search-input" id="g-search" placeholder="filter commits…" />
      <div class="fpill active" data-quick="all">all branches</div>
      <div class="fpill" data-quick="mine">mine</div>
      <div class="fpill" data-quick="week">this week</div>
      <div class="fpill" id="g-toggle-filters">filters</div>
      <div class="tb-spacer"></div>
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

  body.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const delta = e.key === "ArrowDown" ? 1 : -1;
    const base = selectedIndex < 0 ? firstIndex : selectedIndex;
    selectedIndex = Math.max(0, Math.min(totalCount - 1, base + delta));
    scrollToIndex(selectedIndex);
    firstIndex = Math.max(0, Math.floor(body.scrollTop / ROW_HEIGHT) - OVERSCAN);
    loadWindow(firstIndex);
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

  await loadWindow(0);
}
