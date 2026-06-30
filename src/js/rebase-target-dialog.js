// Interactive rebase target picker (SPEC.md item 10, PRD §16.1) — opened from the toolbar
// "Rebase" button or ⌘⇧R. The user picks a branch/ref to rebase the current branch *onto*;
// the chosen ref is stashed in sessionStorage so rebase-page.js can read it on load.

import { listBranchesForSwitch } from "./app.js";
import { laneColorVar } from "../components/commit-row.js";
import { openDialog } from "../components/dialog.js";

/**
 * Opens the rebase target picker and navigates to rebase.html on confirm.
 * Never resolves (resolving would mean staying on the current page).
 * @param {{ repoPath: string }} opts
 */
export function openRebaseTargetDialog({ repoPath }) {
  const dlg = openDialog({
    icon: "↕",
    iconVariant: "purple",
    title: "Interactive rebase",
    size: "small",
    bodyHtml: `<div class="loading-center"><div class="spinner"></div></div>`,
    footerHtml: "",
  });

  listBranchesForSwitch(repoPath).then((entries) => {
    const others = entries.filter((e) => !e.is_head && !e.is_remote_only);
    const state = { query: "", selected: 0 };

    function filtered() {
      const q = state.query.toLowerCase();
      return others.filter((e) => e.name.toLowerCase().includes(q));
    }

    function selectedEntry() {
      return filtered()[state.selected];
    }

    function render() {
      const rows = filtered();
      const target = selectedEntry();
      dlg.setBody(`
        <input class="inp" id="rbt-search" placeholder="filter branches…" value="${state.query}" autocomplete="off">
        <div class="sb-branch-list" id="rbt-list">
          ${
            rows.length === 0
              ? `<div class="empty-state-hint" style="padding:12px 8px;">No other branches.</div>`
              : rows
                  .map(
                    (r, i) => `
              <div class="sb-branch-row ${i === state.selected ? "selected" : ""}" data-index="${i}">
                <span class="sb-dot" style="background:${laneColorVar(r.color_index)}"></span>
                <span class="sb-branch-name">${r.name}</span>
              </div>
            `
                  )
                  .join("")
          }
        </div>
        <div class="info-box ib-blue" style="margin-top:8px;">
          Rebase current branch onto <strong>${target ? target.name : "…"}</strong>.
          Commits after this branch's tip will be replayed interactively.
        </div>
      `);
      dlg.setFooter(`
        <div class="btn btn-neutral" id="rbt-cancel">Cancel</div>
        <div class="btn btn-blue ${!target ? "disabled" : ""}" id="rbt-confirm">Continue</div>
      `);

      const listEl = dlg.bodyEl.querySelector("#rbt-list");
      listEl.querySelectorAll(".sb-branch-row").forEach((row) => {
        row.addEventListener("click", () => {
          state.selected = Number(row.dataset.index);
          render();
        });
        row.addEventListener("dblclick", () => doConfirm());
      });

      const searchInput = dlg.bodyEl.querySelector("#rbt-search");
      searchInput.focus();
      searchInput.addEventListener("input", () => {
        state.query = searchInput.value;
        state.selected = 0;
        render();
      });
      searchInput.addEventListener("keydown", (e) => {
        const rowCount = filtered().length;
        if (e.key === "ArrowDown") {
          e.preventDefault();
          state.selected = Math.min(state.selected + 1, rowCount - 1);
          render();
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          state.selected = Math.max(state.selected - 1, 0);
          render();
        } else if (e.key === "Enter") {
          e.preventDefault();
          doConfirm();
        } else if (e.key === "Escape") {
          dlg.close();
        }
      });

      dlg.footerEl.querySelector("#rbt-cancel").addEventListener("click", () => dlg.close());
      const confirmBtn = dlg.footerEl.querySelector("#rbt-confirm");
      if (!confirmBtn.classList.contains("disabled")) {
        confirmBtn.addEventListener("click", () => doConfirm());
      }
    }

    function doConfirm() {
      const t = selectedEntry();
      if (!t) return;
      sessionStorage.setItem("trunk-rebase-onto", t.name);
      dlg.close();
      window.location.href = "rebase.html";
    }

    render();
  });
}
