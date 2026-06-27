// Dialog chrome: header/body/footer, small (440px) and standard (520px) widths (PRD §10).
// One reusable builder — concrete dialogs (clone wizard, nested-repo choice,
// create-workspace prompt) supply content and call setBody/setFooter to step through flows.

const ICON_VARIANTS = { blue: "ic-b", green: "ic-g", red: "ic-r", amber: "ic-a", purple: "ic-p" };

export function openDialog({ icon = "", iconVariant = "blue", title, subtitle = "", bodyHtml = "", footerHtml = "", size = "standard" }) {
  const overlay = document.createElement("div");
  overlay.className = "dlg-overlay";

  const dlg = document.createElement("div");
  dlg.className = size === "small" ? "dlg dlg-small" : "dlg";

  const header = document.createElement("div");
  header.className = "dh";
  header.innerHTML = `
    <div class="dh-icon ${ICON_VARIANTS[iconVariant] || ICON_VARIANTS.blue}">${icon}</div>
    <div>
      <div class="dh-title"></div>
      <div class="dh-sub"></div>
    </div>
  `;
  header.querySelector(".dh-title").textContent = title;
  header.querySelector(".dh-sub").textContent = subtitle;
  if (!subtitle) header.querySelector(".dh-sub").style.display = "none";

  const body = document.createElement("div");
  body.className = "db";
  body.innerHTML = bodyHtml;

  const footer = document.createElement("div");
  footer.className = "dfoot";
  footer.innerHTML = footerHtml;

  dlg.append(header, body, footer);
  overlay.append(dlg);
  document.body.append(overlay);

  function close() {
    document.removeEventListener("keydown", onKeydown, true);
    overlay.remove();
  }

  function onKeydown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) close();
  });
  document.addEventListener("keydown", onKeydown, true);

  const firstFocusable = body.querySelector("input, select, textarea, button");
  if (firstFocusable) firstFocusable.focus();

  return {
    el: dlg,
    bodyEl: body,
    footerEl: footer,
    setBody(html) {
      body.innerHTML = html;
    },
    setFooter(html) {
      footer.innerHTML = html;
    },
    close,
  };
}
