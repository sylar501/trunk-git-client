// Transient toasts: success/warning/info/danger, bottom-centre.

function toastRoot() {
  let root = document.getElementById("toast-root");
  if (!root) {
    root = document.createElement("div");
    root.id = "toast-root";
    root.className = "toast-root";
    document.body.append(root);
  }
  return root;
}

export function showToast({ variant = "info", message, duration = 4000 }) {
  const el = document.createElement("div");
  el.className = `toast toast-${variant}`;
  el.textContent = message;
  toastRoot().append(el);
  setTimeout(() => el.remove(), duration);
  return el;
}
