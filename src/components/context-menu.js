// Item/hover/danger/separator context menu, positioned at a click point.

export function openContextMenu(x, y, items) {
  document.querySelectorAll(".ctx-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "ctx-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  for (const item of items) {
    if (item === "separator") {
      const sep = document.createElement("div");
      sep.className = "ctx-separator";
      menu.append(sep);
      continue;
    }
    const row = document.createElement("div");
    row.className = item.danger ? "ctx-item danger" : "ctx-item";
    row.textContent = item.label;
    row.addEventListener("click", () => {
      close();
      item.onClick();
    });
    menu.append(row);
  }

  document.body.append(menu);

  function close() {
    document.removeEventListener("mousedown", onOutside, true);
    document.removeEventListener("keydown", onKeydown, true);
    menu.remove();
  }

  function onOutside(e) {
    if (!menu.contains(e.target)) close();
  }

  function onKeydown(e) {
    if (e.key === "Escape") close();
  }

  document.addEventListener("mousedown", onOutside, true);
  document.addEventListener("keydown", onKeydown, true);

  return { close };
}
