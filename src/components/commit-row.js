// Commit row (PRD §7.1): graph lane column (SVG), branch/tag/remote pills, message/author/
// timestamp/SHA. A single recycled DOM node is created per visible row and mutated in place
// by `update()` — this is what lets the virtualised scroll container in graph-view.js avoid
// per-frame DOM churn.

import { createBranchPill } from "./branch-pill.js";

const LANE_START = 16;
const LANE_SPACING = 16;
const GCOL_MIN_WIDTH = 64;

export function laneColorVar(colorIndex) {
  return `var(--lane-${((colorIndex - 1 + 7) % 7) + 1})`;
}

function laneX(lane) {
  return LANE_START + lane * LANE_SPACING;
}

/**
 * Width of the whole lane column for a graph whose deepest lane index is `maxLane` (from
 * `GraphMeta.max_lane`, computed once for the whole history). Every row must share this same
 * width — sizing the column per row independently is what caused wide rows to overflow into
 * the message text while narrower rows clipped their own far-right lines.
 */
export function laneColumnWidth(maxLane) {
  return Math.max(GCOL_MIN_WIDTH, laneX(maxLane) + LANE_START);
}

const STROKE_WIDTH = 2;

/**
 * One smoothed 90° turn between two points — used for merge/join connectors instead of a
 * diagonal sweep across the whole cell, matching the convention of GitKraken-style graphs
 * (a short straight run, one rounded corner, then a short straight run into the node).
 * `startAxis: "vertical"` travels straight along the start point's axis first then turns
 * onto the end point's axis (used for a lane closing in from above); `"horizontal"` turns
 * the other way (used for a lane forking out from the node).
 */
function roundedElbow(fromX, fromY, toX, toY, startAxis, r) {
  const dx = toX - fromX;
  const dy = toY - fromY;
  const signX = dx < 0 ? -1 : 1;
  const signY = dy < 0 ? -1 : 1;
  const rx = Math.min(r, Math.abs(dx));
  const ry = Math.min(r, Math.abs(dy));
  if (startAxis === "vertical") {
    const cornerY = toY - signY * ry;
    return `M${fromX} ${fromY} L${fromX} ${cornerY} Q${fromX} ${toY} ${fromX + signX * rx} ${toY} L${toX} ${toY}`;
  }
  const cornerX = toX - signX * rx;
  return `M${fromX} ${fromY} L${cornerX} ${fromY} Q${toX} ${fromY} ${toX} ${fromY + signY * ry} L${toX} ${toY}`;
}

/**
 * Builds the per-row lane SVG from a `GraphRow` (see `src-tauri/src/git/mod.rs`):
 * `through_lanes` are branches alive but not touched by this commit (drawn as plain
 * straight lines so parallel history reads as continuous); `connectors` describe this
 * commit's own merge/branch-point edges, each carrying the *other* lane's own colour
 * (`color_index`) so a merge/join always reads as the branch it touches, never as the
 * primary lane's colour; `lane`/`lane_color_index` is this commit's own position and
 * colour. Each row is fully self-describing — no state carried between rows — which is
 * what makes recycling arbitrary rows during fast scroll safe. `width` is the graph-wide
 * lane column width (see `laneColumnWidth`), not derived per row.
 */
function renderLaneSvg(row, rowHeight, width) {
  const half = rowHeight / 2;
  const radius = Math.min(6, rowHeight * 0.19);

  const connectorByLane = new Map(row.connectors.map((c) => [c.lane, c]));
  const primaryColor = laneColorVar(row.lane_color_index);
  const primaryX = laneX(row.lane);
  const isFreshPrimary = !row.through_lanes.some((t) => t.lane === row.lane);
  const hasParents = row.parents.length > 0;

  const parts = [];

  // Plain pass-through lanes (not this row's own, not a JoinIn closing here): a full-height
  // straight line, since nothing about this commit touches them.
  for (const t of row.through_lanes) {
    if (t.lane === row.lane) continue;
    if (connectorByLane.get(t.lane)?.kind === "join_in") continue;
    const x = laneX(t.lane);
    parts.push(`<line x1="${x}" y1="0" x2="${x}" y2="${rowHeight}" stroke="${laneColorVar(t.color_index)}" stroke-width="${STROKE_WIDTH}" stroke-linecap="round"/>`);
  }

  // Connectors: JoinIn = another lane closing into this commit's node from above — straight
  // run down, one rounded turn, then horizontal into the node. MergeIn = this commit (the
  // node) forking out to an additional parent lane below — horizontal run from the node, one
  // rounded turn, then straight down into the target lane. Both are coloured as the *other*
  // lane (`c.color_index`), never the primary lane's colour.
  for (const c of row.connectors) {
    const x2 = laneX(c.lane);
    const color = laneColorVar(c.color_index);
    if (c.kind === "join_in") {
      parts.push(`<path d="${roundedElbow(x2, 0, primaryX, half, "vertical", radius)}" fill="none" stroke="${color}" stroke-width="${STROKE_WIDTH}" stroke-linecap="round"/>`);
    } else {
      parts.push(`<path d="${roundedElbow(primaryX, half, x2, rowHeight, "horizontal", radius)}" fill="none" stroke="${color}" stroke-width="${STROKE_WIDTH}" stroke-linecap="round"/>`);
    }
  }

  // This row's own lane — a plain full-height straight line (no gap; the node paints over it
  // below), trimmed to whichever half doesn't actually exist (a fresh tip has nothing above,
  // a root commit has nothing below).
  const topY = isFreshPrimary ? half : 0;
  const bottomY = hasParents ? rowHeight : half;
  if (bottomY > topY) {
    parts.push(`<line x1="${primaryX}" y1="${topY}" x2="${primaryX}" y2="${bottomY}" stroke="${primaryColor}" stroke-width="${STROKE_WIDTH}" stroke-linecap="round"/>`);
  }

  const fill = row.is_head ? primaryColor : "var(--bg-primary)";
  parts.push(`<circle cx="${primaryX}" cy="${half}" r="${radius}" fill="${fill}" stroke="${primaryColor}" stroke-width="${STROKE_WIDTH}"/>`);

  return `<svg width="${width}" height="${rowHeight}" style="overflow:visible">${parts.join("")}</svg>`;
}

function relativeTime(epochSeconds) {
  const deltaSeconds = Math.max(0, Math.floor(Date.now() / 1000) - epochSeconds);
  if (deltaSeconds < 60) return "now";
  const minutes = Math.floor(deltaSeconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d`;
  const months = Math.floor(days / 30);
  return `${months}mo`;
}

/** @param {{ rowHeight: number }} opts */
export function createCommitRow({ rowHeight }) {
  const el = document.createElement("div");
  el.className = "crow";
  el.style.height = `${rowHeight}px`;
  el.innerHTML = `
    <div class="gcol"></div>
    <div class="cmeta">
      <span class="bpills"></span>
      <span class="cmsg"></span>
      <span class="cauth"></span>
      <span class="ctime"></span>
      <span class="csha"></span>
    </div>
  `;
  const gcolEl = el.querySelector(".gcol");
  const bpillsEl = el.querySelector(".bpills");
  const msgEl = el.querySelector(".cmsg");
  const authEl = el.querySelector(".cauth");
  const timeEl = el.querySelector(".ctime");
  const shaEl = el.querySelector(".csha");

  /**
   * @param {object} row - a `GraphRow` from `app.js`'s `getGraphRows()`
   * @param {{ gcolWidth: number }} opts - `gcolWidth` is the lane column width for the
   *   *currently loaded scroll window* (see `graph-view.js`), not the whole history — a
   *   repo's peak concurrent-lane count can be huge in one busy stretch and tiny elsewhere,
   *   so sizing every row off the all-time max wastes most of the row on empty space and
   *   crushes the message column. Re-applied every update, not just at creation, since the
   *   window (and its width) changes as the user scrolls.
   */
  function update(row, { selected = false, dimmed = false, top = 0, index = -1, gcolWidth } = {}) {
    el.style.transform = `translateY(${top}px)`;
    el.classList.toggle("sel", selected);
    el.classList.toggle("dimmed", dimmed);
    el.dataset.sha = row.sha;
    el.dataset.index = String(index);

    gcolEl.style.width = `${gcolWidth}px`;
    gcolEl.innerHTML = renderLaneSvg(row, rowHeight, gcolWidth);

    bpillsEl.innerHTML = "";
    for (const ref of row.refs) {
      bpillsEl.append(createBranchPill(ref));
    }

    msgEl.textContent = row.summary;
    authEl.textContent = row.author_name;
    timeEl.textContent = relativeTime(row.time);
    shaEl.textContent = row.short_sha;
  }

  return { el, update };
}
