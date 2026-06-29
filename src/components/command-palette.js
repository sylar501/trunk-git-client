// Command palette overlay (PRD §10, SPEC.md item 9) — global ⌘K fuzzy-search surface.
// Pure UI: knows nothing about what a command does or where its list comes from — that's
// `command-registry.js`'s job. Dismissal (capture-phase Escape, backdrop-only mousedown)
// mirrors `dialog.js`'s `openDialog` pattern rather than re-deriving it.

const ICON_VARIANTS = { blue: "ic-b", green: "ic-g", red: "ic-r", amber: "ic-a", purple: "ic-p", neutral: "ic-n" };
// Tabs by *result type* (what you're searching for), not by command category — the
// placeholder text ("Type a command, branch, or commit…") names exactly these three, and a
// result-type split is what actually disambiguates "git" results, which used to lump branch
// switches, commit search hits, and the git commands themselves into one indistinguishable
// scope. See SPEC.md item 9's revision note for why this departs from §10.2's literal
// All/Git/Navigate/View/Repos tab list.
const SCOPES = ["all", "commands", "branches", "commits"];
const SCOPE_LABELS = { all: "All", commands: "Commands", branches: "Branches", commits: "Commits" };

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function highlight(label, query) {
  if (!query) return escapeHtml(label);
  const idx = label.toLowerCase().indexOf(query.toLowerCase());
  if (idx < 0) return escapeHtml(label);
  return (
    escapeHtml(label.slice(0, idx)) +
    `<span class="cmdp-match">${escapeHtml(label.slice(idx, idx + query.length))}</span>` +
    escapeHtml(label.slice(idx + query.length))
  );
}

function matchesQuery(command, query) {
  if (!query) return true;
  const q = query.toLowerCase();
  return command.label.toLowerCase().includes(q) || (command.description || "").toLowerCase().includes(q);
}

/**
 * @param {Array<{id, label, description?, scope, iconVariant?, icon?, shortcutLabel?,
 *   disabled?, run: () => void}>} commands - the full inventory for the current context;
 *   already includes any dynamic branch/commit rows (PRD §10.2) flattened in by
 *   the caller — this module just filters/renders/runs.
 * @param {{
 *   onClose?: () => void - fires exactly once, whether dismissal came from inside
 *     (Escape/outside-click/running a command) or from the caller's own `close()` (used when
 *     ⌘K is pressed again while already open — see `command-registry.js`'s
 *     `mountCommandPalette`).
 *   fetchDynamic?: (query: string) => Promise<Array<object>> - query-dependent rows (commit
 *     search, §10.2) the caller can't supply up front since they depend on what's typed.
 *     Debounced 150ms (matches `graph-view.js`'s own filter bar) and merged into the static
 *     `commands` for filtering/rendering once resolved — a deliberate, documented exception
 *     to §10.5's "index-only" requirement for this one result category; see SPEC.md item 9's
 *     note.
 *   dynamicMinQueryLength?: number - `fetchDynamic` isn't called below this length (default
 *     0, i.e. always called once there's any query at all) — short queries against a search
 *     that scans real history are mostly noise, not worth the round trip.
 *   dynamicHint?: string - shown instead of the generic "No matching results." when the query
 *     is too short to have triggered `fetchDynamic` yet and nothing else matched either (e.g.
 *     "Type at least 3 characters to search commits.").
 * }} opts
 * @returns {{ close: () => void }}
 */
export function openCommandPalette(commands, { onClose, fetchDynamic, dynamicMinQueryLength = 0, dynamicHint } = {}) {
  const previouslyFocused = document.activeElement;

  const overlay = document.createElement("div");
  overlay.className = "cmdp-overlay";
  overlay.innerHTML = `
    <div class="cmdp-panel">
      <input class="cmdp-input" placeholder="Type a command, branch, or commit…" />
      <div class="cmdp-tabs">
        ${SCOPES.map((s) => `<div class="cmdp-tab${s === "all" ? " active" : ""}" data-scope="${s}">${SCOPE_LABELS[s]}</div>`).join("")}
      </div>
      <div class="cmdp-results"></div>
    </div>
  `;
  document.body.append(overlay);

  const input = overlay.querySelector(".cmdp-input");
  const resultsEl = overlay.querySelector(".cmdp-results");
  const tabEls = [...overlay.querySelectorAll(".cmdp-tab")];

  let scope = "all";
  let selectedIndex = 0;
  let visible = [];
  let closed = false;
  let dynamicExtra = [];
  let dynamicGeneration = 0;
  let debounceHandle;

  function close() {
    if (closed) return;
    closed = true;
    clearTimeout(debounceHandle);
    document.removeEventListener("keydown", onKeydown, true);
    overlay.remove();
    if (previouslyFocused?.focus) previouslyFocused.focus();
    onClose?.();
  }

  function runCommand(command, { keepOpen = false } = {}) {
    if (!command || command.disabled) return;
    command.run();
    if (!keepOpen) {
      close();
      return;
    }
    input.value = "";
    dynamicExtra = [];
    selectedIndex = 0;
    render();
  }

  function scheduleDynamicFetch(query) {
    clearTimeout(debounceHandle);
    if (!fetchDynamic || query.length < dynamicMinQueryLength) {
      dynamicExtra = [];
      return;
    }
    const generation = ++dynamicGeneration;
    debounceHandle = setTimeout(() => {
      fetchDynamic(query).then((extra) => {
        if (generation !== dynamicGeneration || closed) return;
        dynamicExtra = extra;
        render();
      });
    }, 150);
  }

  function firstEnabledIndex() {
    const i = visible.findIndex((c) => !c.disabled);
    return i < 0 ? 0 : i;
  }

  function render() {
    const query = input.value.trim();
    // `dynamicExtra` (commit/author search results) is already relevance-filtered server-side
    // by `fetchDynamic` against fields that aren't necessarily what's shown — a commit row's
    // label shows the *short* SHA, but a full-SHA query only matches `row.sha` on the backend
    // — so it's only scope-filtered here, never re-run through `matchesQuery`'s label/
    // description substring check, or a full-SHA search would silently filter its own results
    // back out.
    const inScope = (c) => scope === "all" || c.scope === scope;
    visible = [...commands.filter((c) => inScope(c) && matchesQuery(c, query)), ...dynamicExtra.filter(inScope)];
    if (!visible[selectedIndex] || visible[selectedIndex].disabled) selectedIndex = firstEnabledIndex();
    resultsEl.innerHTML = visible.length
      ? visible
          .map(
            (c, i) => `
              <div class="cmdp-row${i === selectedIndex ? " selected" : ""}${c.disabled ? " disabled" : ""}" data-index="${i}">
                <div class="cmdp-icon ${ICON_VARIANTS[c.iconVariant] || ICON_VARIANTS.neutral}">${c.icon || ""}</div>
                <div class="cmdp-row-text">
                  <div class="cmdp-row-label">${highlight(c.label, query)}</div>
                  ${c.description ? `<div class="cmdp-row-desc">${escapeHtml(c.description)}</div>` : ""}
                </div>
                ${c.shortcutLabel ? `<div class="cmdp-shortcut">${escapeHtml(c.shortcutLabel)}</div>` : ""}
              </div>`
          )
          .join("")
      : `<div class="cmdp-empty">${
          dynamicHint && query.length < dynamicMinQueryLength ? escapeHtml(dynamicHint) : "No matching results."
        }</div>`;
    resultsEl.querySelector(".cmdp-row.selected")?.scrollIntoView({ block: "nearest" });
  }

  function moveSelection(delta) {
    if (!visible.length) return;
    let next = selectedIndex;
    for (let i = 0; i < visible.length; i++) {
      next = (next + delta + visible.length) % visible.length;
      if (!visible[next].disabled) break;
    }
    selectedIndex = next;
    render();
  }

  function setScope(next) {
    scope = next;
    tabEls.forEach((t) => t.classList.toggle("active", t.dataset.scope === scope));
    selectedIndex = 0;
    render();
  }

  function onKeydown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      moveSelection(1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      moveSelection(-1);
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      setScope(SCOPES[(SCOPES.indexOf(scope) + 1) % SCOPES.length]);
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      runCommand(visible[selectedIndex], { keepOpen: e.metaKey || e.ctrlKey });
    }
  }

  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) close();
  });
  resultsEl.addEventListener("mousedown", (e) => {
    const row = e.target.closest(".cmdp-row");
    if (!row) return;
    runCommand(visible[Number(row.dataset.index)]);
  });
  tabEls.forEach((t) => t.addEventListener("mousedown", () => setScope(t.dataset.scope)));
  input.addEventListener("input", () => {
    selectedIndex = 0;
    render();
    scheduleDynamicFetch(input.value.trim());
  });

  document.addEventListener("keydown", onKeydown, true);
  input.focus();
  render();

  return { close };
}
