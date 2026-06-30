// Interactive rebase view (SPEC.md item 10, PRD §16).
// Manages four display modes within a single root element:
//   editing     — commit list (58%) + preview lane (42%) + header/footer
//   executing   — per-step progress spinner
//   conflict    — inline conflict resolver (reuses mountConflictResolver)
//   paused-edit — inline staging (reuses mountStaging) for the Edit step's amend phase

import {
  startInteractiveRebase,
  beginRebaseExecution,
  getRebaseSession,
  resumeRebaseExecution,
  continueRebaseAfterEdit,
  abortInteractiveRebase,
} from "./app.js";
import { mountConflictResolver } from "./resolve-view.js";
import { mountStaging } from "./staging-view.js";
import { showToast } from "../components/toast.js";
import { openDialog } from "../components/dialog.js";
import { renderRebaseRow } from "../components/rebase-row.js";
import { computePreviewRows } from "../components/rebase-preview-lane.js";

/**
 * @param {HTMLElement} root
 * @param {string} repoPath
 * @param {{ ontoRef?: string|null, resume?: boolean, onDone?: () => void }} opts
 */
export async function mountRebase(root, repoPath, { ontoRef = null, resume = false, onDone } = {}) {
  let controller = new AbortController();
  const { signal } = controller;
  // Separate controller for the document-level Esc listener — aborted when leaving editing mode
  // so it doesn't fire during execution, conflict, or paused-for-edit phases.
  let editingKeyController = null;

  // plan: null until loaded; modified in place as user edits rows.
  let plan = null;

  // Load plan or detect existing session.
  if (resume) {
    const session = await getRebaseSession(repoPath);
    if (!session) {
      // Session vanished since the page loaded (unusual edge case) — fall through to graph.
      onDone?.();
      return;
    }
    renderExecuting(`Resuming rebase…`);
    const outcome = await resumeRebaseExecution(repoPath).catch((e) => {
      showToast({ variant: "danger", message: String(e) });
      return null;
    });
    if (outcome) dispatch(outcome);
    return;
  }

  if (!ontoRef) {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">No target branch specified.</div>`;
    return;
  }

  plan = await startInteractiveRebase(repoPath, ontoRef).catch((e) => {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">${String(e)}</div>`;
    return null;
  });
  if (!plan) return;

  if (plan.steps.length === 0) {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">No commits to rebase — branch is already up to date with <strong>${plan.onto_display_name}</strong>.</div>`;
    return;
  }

  renderEditing();

  // --- Mode renderers -----------------------------------------------------------------------

  function renderEditing() {
    root.innerHTML = `
      <div class="rb-layout">
        <div class="rb-header">
          <div class="rb-header-info">
            <span class="rb-branch-label">${plan.branch_name}</span>
            <span class="rb-onto-label">onto ${plan.onto_display_name}</span>
            <span class="rb-count-label">${plan.steps.length} commit${plan.steps.length !== 1 ? "s" : ""}</span>
          </div>
          <span class="rb-esc-hint">Press <span class="kbd-badge">Esc</span> to cancel</span>
        </div>
        <div class="rb-body">
          <div class="rb-list-col" id="rb-list"></div>
          <div class="rb-preview-col" id="rb-preview"></div>
        </div>
        <div class="rb-footer">
          <div class="rb-summary" id="rb-summary"></div>
          <div style="display:flex;gap:6px;">
            <div class="btn btn-neutral" id="rb-cancel">Cancel</div>
            <div class="btn btn-blue" id="rb-begin">Begin Rebase</div>
          </div>
        </div>
      </div>
    `;

    renderList();
    renderPreview();
    renderSummary();

    root.querySelector("#rb-cancel").addEventListener("click", () => onDone?.(), { signal });
    root.querySelector("#rb-begin").addEventListener("click", doBeginRebase, { signal });
    editingKeyController?.abort();
    editingKeyController = new AbortController();
    document.addEventListener("keydown", onEditingKeydown, { signal: editingKeyController.signal });
  }

  function onEditingKeydown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      onDone?.();
    }
  }

  function renderList() {
    const listEl = root.querySelector("#rb-list");
    if (!listEl) return;
    listEl.innerHTML = "";
    plan.steps.forEach((step, index) => {
      const rowEl = renderRebaseRow(step, index, {
        onActionChange: (action) => {
          plan.steps[index] = { ...plan.steps[index], action };
          renderList();
          renderPreview();
          renderSummary();
        },
        onMessageChange: (msg) => {
          plan.steps[index] = { ...plan.steps[index], new_message: msg };
          renderPreview();
        },
        onReorder: reorderRow,
      });
      listEl.appendChild(rowEl);
    });
  }

  function reorderRow(fromIndex, toIndex) {
    if (fromIndex === toIndex) return;
    const item = plan.steps.splice(fromIndex, 1)[0];
    plan.steps.splice(toIndex, 0, item);
    // Squash/fixup cannot be the first step — reset to pick if dragged to position 0.
    if (plan.steps[0].action === "squash" || plan.steps[0].action === "fixup") {
      plan.steps[0] = { ...plan.steps[0], action: "pick" };
    }
    renderList();
    renderPreview();
    renderSummary();
  }

  function renderPreview() {
    const previewEl = root.querySelector("#rb-preview");
    if (!previewEl) return;
    const rows = computePreviewRows(plan.steps);
    if (rows.length === 0) {
      previewEl.innerHTML = `
        <div class="rb-preview-empty">
          <div class="info-box ib-amber" style="margin:16px;">
            All commits will be dropped — the rebase will produce an empty result.
          </div>
        </div>
      `;
      return;
    }
    previewEl.innerHTML = `
      <div class="rb-preview-header">After rebase</div>
      <div class="rb-preview-list">
        ${[...rows].reverse().map((r) => previewRowHtml(r)).join("")}
        <div class="rb-prev-row rb-prev-onto">
          <div class="rb-prev-dot" style="background:var(--border-default)"></div>
          <div class="rb-prev-info" style="flex-direction:row;align-items:center;gap:6px;">
            <span class="rb-prev-badge rb-badge-neutral" style="font-family:monospace;flex-shrink:0;">${plan.onto_short_sha}</span>
            <span class="rb-prev-msg" style="color:var(--text-secondary);">${plan.onto_summary}</span>
          </div>
        </div>
      </div>
    `;
  }

  function previewRowHtml(r) {
    if (r.kind === "squash-target") {
      const basedOnReword = r.baseKind === "reword";
      const basedOnEdit   = r.baseKind === "edit";
      const dotColor = basedOnReword ? "var(--green)" : basedOnEdit ? "var(--purple)" : "var(--amber)";
      return `
        <div class="rb-prev-row">
          <div class="rb-prev-dot" style="background:${dotColor}"></div>
          <div class="rb-prev-info">
            <div style="display:flex;gap:4px;align-items:center;">
              ${basedOnReword ? `<span class="rb-prev-badge rb-badge-green">reword</span>` : ""}
              ${basedOnEdit   ? `<span class="rb-prev-badge rb-badge-purple">edit</span>` : ""}
              <span class="rb-prev-badge rb-badge-amber">squash ×${r.absorbedCount}</span>
            </div>
            <span class="rb-prev-msg${basedOnReword ? " rb-prev-reworded" : ""}">${r.message}</span>
          </div>
        </div>
      `;
    }
    if (r.kind === "reword") {
      return `
        <div class="rb-prev-row">
          <div class="rb-prev-dot" style="background:var(--green)"></div>
          <div class="rb-prev-info">
            <span class="rb-prev-badge rb-badge-green">reword</span>
            <span class="rb-prev-msg rb-prev-reworded">${r.message}</span>
          </div>
        </div>
      `;
    }
    if (r.kind === "edit") {
      return `
        <div class="rb-prev-row">
          <div class="rb-prev-dot" style="background:var(--purple)"></div>
          <div class="rb-prev-info">
            <span class="rb-prev-badge rb-badge-purple">edit</span>
            <span class="rb-prev-msg">${r.message}</span>
          </div>
        </div>
      `;
    }
    return `
      <div class="rb-prev-row">
        <div class="rb-prev-dot" style="background:var(--blue)"></div>
        <div class="rb-prev-info">
          <span class="rb-prev-msg">${r.message}</span>
        </div>
      </div>
    `;
  }

  function renderSummary() {
    const summaryEl = root.querySelector("#rb-summary");
    if (!summaryEl) return;
    const counts = {};
    let picks = 0;
    plan.steps.forEach((s) => {
      if (s.action === "pick") picks++;
      else counts[s.action] = (counts[s.action] || 0) + 1;
    });
    const parts = [`${plan.steps.length} commit${plan.steps.length !== 1 ? "s" : ""}`];
    if (counts.reword) parts.push(`${counts.reword} reworded`);
    if (counts.squash) parts.push(`${counts.squash} squashed`);
    if (counts.fixup) parts.push(`${counts.fixup} fixed-up`);
    if (counts.edit) parts.push(`${counts.edit} pausing for edit`);
    if (counts.drop) parts.push(`${counts.drop} dropped`);
    summaryEl.textContent = parts.join(", ");

    const beginBtn = root.querySelector("#rb-begin");
    if (beginBtn) {
      const allDropped = plan.steps.every((s) => s.action === "drop");
      beginBtn.classList.toggle("disabled", allDropped);
    }
  }

  async function doBeginRebase() {
    const beginBtn = root.querySelector("#rb-begin");
    if (beginBtn?.classList.contains("disabled")) return;
    editingKeyController?.abort();
    renderExecuting("Starting rebase…");
    const outcome = await beginRebaseExecution(repoPath, plan).catch((e) => {
      showToast({ variant: "danger", message: String(e) });
      renderEditing();
      return null;
    });
    if (outcome) dispatch(outcome);
  }

  function renderExecuting(msg = "Rebasing…") {
    root.innerHTML = `
      <div class="rb-executing">
        <div class="spinner" style="margin-bottom:12px;"></div>
        <div class="rb-exec-msg">${msg}</div>
      </div>
    `;
  }

  // --- Outcome dispatch -----------------------------------------------------------------------

  async function dispatch(outcome) {
    if (outcome.status === "finished") {
      showToast({ variant: "success", message: "Rebase complete." });
      onDone?.();
    } else if (outcome.status === "conflict") {
      renderConflict();
    } else if (outcome.status === "paused_for_edit") {
      renderPausedForEdit(outcome.sidecar);
    }
  }

  // --- Conflict mode --------------------------------------------------------------------------

  function renderConflict() {
    // Mount the existing resolver inline — it renders its own full layout into root.
    // onDone fires after abort OR successful Continue; check sidecar afterward to
    // distinguish "truly finished" from "paused for edit" (PausedForEdit maps to
    // ConflictableOutcome::Completed from the resolver's perspective, so its own
    // onDone fires without distinguishing the two — we branch on sidecar presence).
    mountConflictResolver(root, repoPath, {
      onDone: async () => {
        const session = await getRebaseSession(repoPath).catch(() => null);
        if (!session) {
          // Finished or aborted — either way, leave the rebase page.
          onDone?.();
        } else if (session.paused_for_edit) {
          renderPausedForEdit(session);
        } else {
          // Shouldn't normally happen (a non-paused, non-finished sidecar after onDone would mean
          // another conflict fired without the resolver knowing — treat as unexpected conflict).
          renderConflict();
        }
      },
    });
  }

  // --- Paused-for-edit mode -------------------------------------------------------------------

  function renderPausedForEdit(sidecar) {
    root.innerHTML = `
      <div class="rb-layout">
        <div class="rb-paused-banner" style="display:flex;align-items:center;gap:10px;background:var(--amber-bg);border-bottom:1px solid var(--amber);padding:8px 16px;color:var(--amber);flex-shrink:0;">
          <span style="flex:1;">
            Paused for edit on <strong>${(sidecar.paused_for_edit || "").slice(0, 7)}</strong>
            — amend or add commits below, then exit staging to continue.
            ${
              sidecar.remaining_steps?.length > 0
                ? `<span style="opacity:.75;">${sidecar.remaining_steps.length} step${sidecar.remaining_steps.length !== 1 ? "s" : ""} remaining.</span>`
                : `<span style="opacity:.75;">This is the last step.</span>`
            }
          </span>
          <div class="btn btn-red rb-abort-btn" id="rb-abort-edit">Abort rebase</div>
        </div>
        <div class="rb-paused-staging" id="rb-paused-staging" style="flex:1;display:flex;overflow:hidden;"></div>
      </div>
    `;

    root.querySelector("#rb-abort-edit").addEventListener("click", () => {
      const dlg = openDialog({
        icon: "⚠",
        iconVariant: "red",
        title: "Abort rebase?",
        subtitle: "All rebase progress will be lost and the working tree will be restored to its original state.",
        size: "small",
        footerHtml: `
          <div class="btn btn-neutral" id="dlg-abort-cancel">Cancel</div>
          <div class="btn btn-red" id="dlg-abort-confirm">Abort rebase</div>
        `,
      });
      dlg.footerEl.querySelector("#dlg-abort-cancel").addEventListener("click", () => dlg.close());
      dlg.footerEl.querySelector("#dlg-abort-confirm").addEventListener("click", async () => {
        dlg.close();
        await abortInteractiveRebase(repoPath).catch(() => {});
        showToast({ variant: "info", message: "Rebase aborted — working tree restored." });
        onDone?.();
      });
    });

    const stagingRoot = root.querySelector("#rb-paused-staging");
    mountStaging(stagingRoot, repoPath, {
      exitLabel: "Continue rebase",
      rebaseMode: true,
      onExit: async () => {
        // "← history" in staging = "Continue rebase" here — resume the remaining plan.
        renderExecuting("Continuing rebase…");
        const outcome = await continueRebaseAfterEdit(repoPath).catch((e) => {
          showToast({ variant: "danger", message: String(e) });
          return null;
        });
        if (outcome) dispatch(outcome);
      },
    });
  }
}
