// Pure-JS mirror of rebase_plan.rs's squash/fixup folding logic, used for the result-preview
// panel (PRD §16.3). Keeps latency to zero (no backend round-trip per keystroke) by computing
// the after-state entirely in JS — the backend is the authoritative executor, but preview only
// needs to show the expected shape, not the exact commit shas.

/**
 * Computes the list of result commits from the given plan steps (newest-first display order).
 * Mirrors `finish_step`'s squash/fixup folding exactly.
 *
 * @param {object[]} steps — plan.steps in newest-first order (same as user sees in the list).
 * @returns {object[]} — rows in newest-first order; each is:
 *   { kind: "pick"|"reword"|"edit"|"squash-target", message: string, absorbedCount?: number }
 */
export function computePreviewRows(steps) {
  // steps is oldest-first (execution order) — same order as `git rebase -i` display.
  const accumulated = [];

  for (const step of steps) {
    switch (step.action) {
      case "drop":
        break; // skipped entirely, not even a placeholder
      case "pick":
        accumulated.push({ kind: "pick", message: step.summary });
        break;
      case "reword":
        accumulated.push({
          kind: "reword",
          message: step.new_message?.split("\n")[0] || step.summary,
        });
        break;
      case "edit":
        accumulated.push({ kind: "edit", message: step.summary });
        break;
      case "squash": {
        const prev = accumulated[accumulated.length - 1];
        if (!prev) {
          // Squash as first step (invalid plan) — show as standalone pick.
          accumulated.push({ kind: "pick", message: step.summary });
        } else {
          // Replace previous with a combined entry showing absorbed count.
          const count = (prev.absorbedCount || 0) + 1;
          accumulated[accumulated.length - 1] = {
            kind: "squash-target",
            message: prev.message,
            absorbedCount: count,
            absorbedMessages: [...(prev.absorbedMessages || []), step.summary],
          };
        }
        break;
      }
      case "fixup": {
        const prev = accumulated[accumulated.length - 1];
        if (!prev) {
          accumulated.push({ kind: "pick", message: step.summary });
        } else {
          const count = (prev.absorbedCount || 0) + 1;
          accumulated[accumulated.length - 1] = {
            kind: "squash-target",
            message: prev.message,
            absorbedCount: count,
            absorbedMessages: [...(prev.absorbedMessages || [])],
          };
        }
        break;
      }
    }
  }

  return accumulated;
}
