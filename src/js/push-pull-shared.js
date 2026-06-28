// Shared rendering helpers for the Push/Fetch/Pull dialogs (PRD §12, SPEC.md item 7) — all
// three need the same commit-summary list markup and remote-URL footer, just with a different
// badge per direction (push's "new" vs pull's "incoming").

function formatDate(epochSeconds) {
  return new Date(epochSeconds * 1000).toLocaleString();
}

/**
 * @param {{sha:string, short_sha:string, summary:string, author_name:string, time:number}[]} commits
 * @param {{ badgeClass: string, badgeText: string }} badge
 */
export function renderCommitList(commits, badge) {
  if (commits.length === 0) {
    return `<div class="info-box ib-blue">No commits to show.</div>`;
  }
  return `<div class="pf-commit-list">${commits
    .map(
      (c) => `
        <div class="pf-commit-row">
          <span class="pf-commit-sha">${c.short_sha}</span>
          <span class="pf-badge ${badge.badgeClass}">${badge.badgeText}</span>
          <span class="pf-commit-message"></span>
          <span class="pf-commit-meta"></span>
        </div>`
    )
    .join("")}</div>`;
}

/** Fills in the free-form text nodes `renderCommitList`'s markup left as `textContent` targets
 * (message/author/date aren't safe to interpolate as HTML). Call right after inserting the
 * markup returned by `renderCommitList`. */
export function fillCommitListText(containerEl, commits) {
  const rows = containerEl.querySelectorAll(".pf-commit-row");
  rows.forEach((row, i) => {
    const c = commits[i];
    row.querySelector(".pf-commit-message").textContent = c.summary;
    row.querySelector(".pf-commit-meta").textContent = `${c.author_name} · ${formatDate(c.time)}`;
  });
}

export { formatDate };

function formatBytes(n) {
  if (n < 1024) return `${n.toFixed(2)} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(2)} KiB`;
  return `${(n / 1024 / 1024).toFixed(2)} MiB`;
}

function formatCounterLine(stage, current, total, bytes, rate) {
  const done = total > 0 && current >= total;
  let line = total > 0 ? `${stage}: ${Math.floor((current / total) * 100)}% (${current}/${total})` : `${stage}: ${current}`;
  if (bytes != null) {
    line += `, ${formatBytes(bytes)}`;
    if (rate != null && !done) line += ` | ${formatBytes(rate)}/s`;
  }
  if (done) line += ", done.";
  return line;
}

/**
 * Builds a terminal-style progress log from the backend's `ProgressEvent` stream (PRD §12).
 * Both kinds of event can repeat many times — including many *identical* already-"done" ticks,
 * since the underlying libgit2 callbacks don't stop firing the instant a stage's counters hit
 * 100% — for what's really one evolving line, exactly like a real terminal overwriting an
 * in-progress `\r`-terminated line. So both kinds upsert the same line in place, keyed by their
 * stage label, for the lifetime of this log (never "released" back to start a fresh line):
 * - `stage` events (client-computed counters for Counting/Compressing/Writing/Receiving objects,
 *   Resolving deltas) carry their label in `stage` directly.
 * - `remote` events are the server's own sideband text (e.g. "Enumerating objects: 2073, done.")
 *   — same repeating-tick shape, just as a string, so the label is taken as the text up to its
 *   first `:` (falling back to the whole line for one-off summary lines with no early colon,
 *   e.g. "Total 2073 (delta 799)…", which then never collide with anything and just append once).
 */
export function createProgressLog() {
  const lines = [];
  const stageRows = new Map();
  const stageMeta = new Map();

  // Deliberately never "closes out" a key once its line reports done: the underlying libgit2
  // callbacks keep firing identical already-done ticks well after a stage finishes (e.g.
  // "Receiving objects" keeps reporting 274/274 while deltas are still resolving), and those
  // must keep idempotently overwriting the same frozen line, not spawn a new one each time.
  function upsert(key, text) {
    if (stageRows.has(key)) {
      lines[stageRows.get(key)] = text;
    } else {
      stageRows.set(key, lines.length);
      lines.push(text);
    }
  }

  function onEvent(payload) {
    if (payload.kind === "remote") {
      const colon = payload.text.indexOf(":");
      const key = colon === -1 ? payload.text : payload.text.slice(0, colon);
      upsert(key, payload.text);
      return;
    }
    const { stage, current, total, bytes } = payload;
    const now = performance.now();
    let meta = stageMeta.get(stage);
    if (!meta) {
      meta = { startTime: now, startBytes: bytes ?? 0 };
      stageMeta.set(stage, meta);
    }
    const rate = bytes != null ? (bytes - meta.startBytes) / Math.max((now - meta.startTime) / 1000, 0.001) : null;
    const text = formatCounterLine(stage, current, total, bytes, rate);
    upsert(stage, text);
  }

  function appendLine(text) {
    lines.push(text);
  }

  function render(el) {
    el.textContent = lines.join("\n");
    el.scrollTop = el.scrollHeight;
  }

  return { onEvent, appendLine, render };
}

/**
 * Enter-to-close, alongside whatever Close button the caller already wired — scoped to one
 * dialog instance. `dialog.js`'s Escape/click-outside dismissal calls its own internal `close`
 * directly (not through `dlg.close`), so this can't hook that path to clean itself up; instead
 * the listener checks `dlg.el.isConnected` on its next firing and quietly removes itself if the
 * dialog is already gone, rather than firing `action` again.
 */
export function attachEnterToClose(dlg, action) {
  function onKeydown(e) {
    if (!dlg.el.isConnected) {
      document.removeEventListener("keydown", onKeydown);
      return;
    }
    if (e.key === "Enter") {
      document.removeEventListener("keydown", onKeydown);
      action();
    }
  }
  document.addEventListener("keydown", onKeydown);
}
