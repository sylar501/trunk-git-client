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
