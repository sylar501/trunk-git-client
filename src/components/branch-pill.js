// Branch/tag pill shown inline on a tip-commit row (PRD §7.1): local branches, remote
// tracking branches, and tags are visually distinct via colour-coded backgrounds. Only the
// three kinds the backend's `RefBadge.kind` actually emits are handled here — no semantic
// feature/fix-prefix guessing, that's not part of the §7.1 requirement.

const VARIANT_CLASS = {
  local: "bp-main",
  remote: "bp-rem",
  tag: "bp-tag",
};

/** @param {{ name: string, kind: "local" | "remote" | "tag" }} refBadge */
export function createBranchPill(refBadge) {
  const el = document.createElement("span");
  el.className = `bpill ${VARIANT_CLASS[refBadge.kind] ?? "bp-rem"}`;
  el.textContent = refBadge.name;
  return el;
}
