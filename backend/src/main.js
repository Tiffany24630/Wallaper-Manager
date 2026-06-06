const invoke = window.__TAURI__?.core?.invoke;

const elements = {};
let wallpapers = [];

function queryElements() {
  [
    "status-ready",
    "status-total",
    "status-active",
    "status-uptime",
    "path-backend",
    "path-wallpapers",
    "path-config",
    "active-wallpaper",
    "active-subtitle",
    "library-subtitle",
    "library",
    "message",
    "refresh-button",
    "import-form",
    "source-path",
  ].forEach((id) => {
    elements[id] = document.querySelector(`#${id}`);
  });

  elements.template = document.querySelector("#wallpaper-template");
}

async function invokeCommand(command, payload) {
  if (!invoke) {
    throw new Error("Run this screen inside Tauri to use backend commands.");
  }

  return invoke(command, payload);
}

async function refresh() {
  setMessage("Scanning backend...");

  try {
    const [status, library] = await Promise.all([
      invokeCommand("get_backend_status"),
      invokeCommand("list_wallpapers"),
    ]);

    wallpapers = library;
    renderStatus(status);
    renderLibrary(library);
    setMessage("Ready");
  } catch (error) {
    setMessage(error.message || String(error), true);
  }
}

function renderStatus(status) {
  elements["status-ready"].textContent = status.ready ? "Ready" : "Offline";
  elements["status-total"].textContent = status.totalWallpapers;
  elements["status-active"].textContent =
    status.activeWallpaper?.title || "None";
  elements["status-uptime"].textContent = formatDuration(status.uptimeSeconds);

  elements["path-backend"].textContent = status.backendRoot;
  elements["path-wallpapers"].textContent = status.wallpaperRoot;
  elements["path-config"].textContent = status.configPath;

  renderActive(status.activeWallpaper);
}

function renderActive(wallpaper) {
  if (!wallpaper) {
    elements["active-subtitle"].textContent = "No wallpaper selected.";
    elements["active-wallpaper"].className = "active-wallpaper empty";
    elements["active-wallpaper"].innerHTML =
      "<span>No hay wallpaper activo todavia.</span>";
    return;
  }

  elements["active-subtitle"].textContent = `${wallpaper.kind} - ${formatBytes(
    wallpaper.sizeBytes
  )}`;
  elements["active-wallpaper"].className = "active-wallpaper";
  elements["active-wallpaper"].innerHTML = `
    <div>
      <span class="kind">${escapeHtml(wallpaper.kind)}</span>
      <h3>${escapeHtml(wallpaper.title)}</h3>
      <p>${escapeHtml(wallpaper.relativePath)}</p>
    </div>
  `;
}

function renderLibrary(library) {
  elements["library"].innerHTML = "";
  elements["library-subtitle"].textContent = library.length
    ? `${library.length} archivos encontrados`
    : "Agrega archivos en backend/wallpapers/images o importa una ruta local.";

  if (!library.length) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "No hay wallpapers en la biblioteca.";
    elements["library"].append(empty);
    return;
  }

  library.forEach((wallpaper) => {
    const card = elements.template.content.cloneNode(true);
    const article = card.querySelector(".wallpaper-card");
    const preview = card.querySelector(".wallpaper-preview");
    const kind = card.querySelector(".kind");
    const title = card.querySelector("h3");
    const meta = card.querySelector(".meta");
    const favoriteButton = card.querySelector(".favorite-button");
    const applyButton = card.querySelector(".apply-button");

    article.classList.toggle("active", wallpaper.active);
    preview.dataset.kind = wallpaper.kind;
    kind.textContent = wallpaper.kind;
    title.textContent = wallpaper.title;
    meta.textContent = `${wallpaper.relativePath} - ${formatBytes(
      wallpaper.sizeBytes
    )}`;
    favoriteButton.textContent = wallpaper.favorite ? "Saved" : "Star";
    applyButton.textContent = wallpaper.canApplyToDesktop
      ? "Aplicar"
      : "Seleccionar";

    favoriteButton.addEventListener("click", () => toggleFavorite(wallpaper.id));
    applyButton.addEventListener("click", () => setActive(wallpaper.id));

    elements["library"].append(card);
  });
}

async function importWallpaper(event) {
  event.preventDefault();
  const sourcePath = elements["source-path"].value.trim();

  if (!sourcePath) {
    setMessage("Pega una ruta local primero.", true);
    return;
  }

  try {
    setMessage("Importing wallpaper...");
    await invokeCommand("import_wallpaper", { sourcePath });
    elements["source-path"].value = "";
    await refresh();
  } catch (error) {
    setMessage(error.message || String(error), true);
  }
}

async function setActive(wallpaperId) {
  try {
    const result = await invokeCommand("set_active_wallpaper", { wallpaperId });
    setMessage(result.message);
    await refresh();
  } catch (error) {
    setMessage(error.message || String(error), true);
  }
}

async function toggleFavorite(wallpaperId) {
  try {
    await invokeCommand("toggle_favorite", { wallpaperId });
    await refresh();
  } catch (error) {
    setMessage(error.message || String(error), true);
  }
}

function setMessage(message, isError = false) {
  elements.message.textContent = message;
  elements.message.classList.toggle("error", isError);
}

function formatBytes(bytes) {
  if (!bytes) return "0 B";

  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${
    units[unitIndex]
  }`;
}

function formatDuration(seconds) {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}m ${rest}s`;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

window.addEventListener("DOMContentLoaded", () => {
  queryElements();
  elements["refresh-button"].addEventListener("click", refresh);
  elements["import-form"].addEventListener("submit", importWallpaper);
  refresh();
});
