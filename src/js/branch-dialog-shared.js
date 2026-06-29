// Shared submit-flow helper for the four branch dialogs (PRD §13.5, SPEC.md item 8). Revised
// from §13.5's original "inline success message (spinner → success → auto-close)" wording
// during this session: a dialog that lingers on a static success message after the operation
// already finished reads as a stuck/frozen dialog rather than confirmation, especially for
// near-instant local git calls like these. Success now closes the dialog immediately and lets
// the caller show a toast (the toast.js component, same as push/pull/cherry-pick elsewhere in
// the app) — the dialog only stays open on failure, so it can show the error inline with enough
// context to fix and retry.

import { openDialog } from "../components/dialog.js";

/**
 * The dirty-tree "stash automatically / carry over" choice (§13.2), shared by the full Switch
 * dialog, the sidebar's direct branch-row click (sidebar.js), and Create branch's "checkout
 * after creating" step (create-branch-dialog.js — checking out a new branch at a different
 * starting point is exactly the same "move HEAD across trees" operation as switching) — all
 * three go through the exact same confirm UI. Resolves `"stash"` / `"carry"`, or `null` on
 * Cancel.
 * @param {string} [actionLabel] - primary button label; defaults to "Switch".
 * @returns {Promise<"stash"|"carry"|null>}
 */
export function confirmDirtyTreeStrategy(actionLabel = "Switch") {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "⚠",
      iconVariant: "amber",
      title: "Uncommitted changes",
      size: "small",
      bodyHtml: `
        <div class="info-box ib-amber">Your working tree has uncommitted changes.</div>
        <label class="cb-opt"><input type="radio" name="dts-strategy" value="stash" checked /> Stash automatically, apply after switch</label>
        <label class="cb-opt"><input type="radio" name="dts-strategy" value="carry" /> Carry over (may fail on conflict)</label>
      `,
      footerHtml: `
        <div class="btn btn-neutral" id="dts-cancel">Cancel</div>
        <div class="btn btn-blue" id="dts-go">${actionLabel}</div>
      `,
    });
    dlg.footerEl.querySelector("#dts-cancel").addEventListener("click", () => {
      dlg.close();
      resolve(null);
    });
    dlg.footerEl.querySelector("#dts-go").addEventListener("click", () => {
      const strategy = dlg.bodyEl.querySelector('input[name="dts-strategy"]:checked').value;
      dlg.close();
      resolve(strategy);
    });
  });
}

// Mirrors `git check-ref-format`'s rules closely enough for instant client-side feedback
// (§13.1/§13.3's real-time green/red border) — illegal characters, leading/trailing/doubled
// slashes, ".."/".lock"/trailing "." segments. The backend's own `git2` call is still the
// authoritative check; this only avoids a round trip for the common-case typo.
const ILLEGAL_CHARS = /[\s~^:?*[\\\x00-\x1f]/;

/** @param {string} name @param {string[]} existingNames @returns {{ valid: boolean, error?: string }} */
export function validateBranchName(name, existingNames = []) {
  if (!name) return { valid: false, error: "Branch name can't be empty." };
  if (ILLEGAL_CHARS.test(name)) return { valid: false, error: "Contains an illegal character (space, ~^:?*[\\)." };
  if (name.includes("..")) return { valid: false, error: "Can't contain '..'." };
  if (name.startsWith("/") || name.endsWith("/") || name.includes("//")) {
    return { valid: false, error: "Can't start/end with '/' or contain '//'." };
  }
  if (name.endsWith(".") || name.endsWith(".lock")) return { valid: false, error: "Can't end with '.' or '.lock'." };
  if (name === "@") return { valid: false, error: "Can't be just '@'." };
  if (existingNames.includes(name)) return { valid: false, error: "A branch with this name already exists." };
  return { valid: true };
}

/**
 * Runs `task()`, swapping the dialog body for a spinner while it's in flight. On success, closes
 * the dialog immediately (the caller shows a toast — see this file's header comment). On
 * failure, calls `onError` with the thrown value so the caller can re-render its own form (with
 * its listeners intact) and show an inline error the way it already does elsewhere (red border,
 * error box, etc.) — failure never closes the dialog.
 *
 * @param {ReturnType<typeof import("../components/dialog.js").openDialog>} dlg
 * @param {{
 *   task: () => Promise<unknown>,
 *   onError: (err: unknown) => void,
 *   onMutated?: () => Promise<void> | void,
 * }} opts
 */
export function runDialogTask(dlg, { task, onError, onMutated }) {
  dlg.setBody(`<div class="loading-center"><div class="spinner lg"></div></div>`);
  dlg.setFooter("");

  task()
    .then(async () => {
      dlg.close();
      await onMutated?.();
    })
    .catch(onError);
}
