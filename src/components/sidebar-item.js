// Row factory for the sidebar's Branches section (PRD §4.2) — local branches, coloured with
// the same hash-based lane colour the graph uses, so sidebar dots and graph lanes match.
// The existing Repositories section (`sidebar.js`, SPEC.md item 2) keeps its own inline
// rendering rather than being routed through this factory — it already shipped and works,
// and re-plumbing it isn't part of this session's scope.

/** @param {{ dotColor?: string, label: string, badgeText?: string, active?: boolean }} opts */
export function createSidebarItem({ dotColor, label, badgeText, active = false }) {
  const row = document.createElement("div");
  row.className = active ? "sb-item active" : "sb-item";
  row.innerHTML = `<span class="sb-dot"></span><span class="sb-name"></span>${
    badgeText ? `<span class="sb-badge"></span>` : ""
  }`;
  if (dotColor) row.querySelector(".sb-dot").style.background = dotColor;
  row.querySelector(".sb-name").textContent = label;
  if (badgeText) row.querySelector(".sb-badge").textContent = badgeText;
  return row;
}

export function createSidebarSection(label) {
  const sec = document.createElement("div");
  sec.className = "sb-sec";
  sec.innerHTML = `<span></span>`;
  sec.querySelector("span").textContent = label;
  return sec;
}
