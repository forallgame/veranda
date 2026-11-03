/* Padu UI - vanilla JS, no build step. Talks to the Rust side through Tauri IPC. */
// Apply the saved theme as early as possible (no inline script: keeps the CSP strict).
try { const saved = localStorage.getItem("padu.theme"); if (saved === "light" || saved === "dark") document.documentElement.dataset.theme = saved; } catch (e) {}
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const KINDS = [
  ["php", "PHP"], ["caddy", "Caddy"], ["mariadb", "MariaDB"], ["mysql", "MySQL"], ["cache", "Valkey / Redis"], ["mailpit", "Mailpit"],
  ["node", "Node.js"], ["composer", "Composer"], ["adminer", "Adminer"], ["phpmyadmin", "phpMyAdmin"], ["nginx", "Nginx"], ["apache", "Apache"], ["cabundle", "CA bundle"],
];
const ICONS = {
  web: '<svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm7.9 9h-3a15 15 0 0 0-1.4-5.6A8 8 0 0 1 19.9 11zM12 4c.9 1 2.1 3.3 2.4 7H9.6C9.9 7.3 11.1 5 12 4zM4.1 13h3c.2 2.1.6 4 1.3 5.6A8 8 0 0 1 4.1 13zm3-2h-3a8 8 0 0 1 4.3-5.6A15 15 0 0 0 7.1 11zM12 20c-.9-1-2.1-3.3-2.4-7h4.8c-.3 3.7-1.5 6-2.4 7zm2.6-1.4c.7-1.6 1.1-3.5 1.3-5.6h3a8 8 0 0 1-4.3 5.6z"/></svg>',
  php: '<svg viewBox="0 0 24 24"><path d="M4 7h16a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2zm1.5 2.5v5h1.4v-1.6h1c1.3 0 2.1-.7 2.1-1.7s-.8-1.7-2.1-1.7H5.5zm1.4 1.1h.9c.5 0 .8.2.8.6s-.3.6-.8.6h-.9v-1.2zm3.9-1.1v5h1.4v-1.9h1.6v1.9h1.4v-5h-1.4v2h-1.6v-2h-1.4zm5.3 0v5h1.4v-1.6h1c1.3 0 2.1-.7 2.1-1.7s-.8-1.7-2.1-1.7h-2.4zm1.4 1.1h.9c.5 0 .8.2.8.6s-.3.6-.8.6h-.9v-1.2z"/></svg>',
  db: '<svg viewBox="0 0 24 24"><path d="M12 2C7 2 4 3.8 4 6v12c0 2.2 3 4 8 4s8-1.8 8-4V6c0-2.2-3-4-8-4zm6 16c0 .7-2.1 2-6 2s-6-1.3-6-2v-2.9c1.5.9 3.6 1.4 6 1.4s4.5-.5 6-1.4V18zm0-6c0 .7-2.1 2-6 2s-6-1.3-6-2V9.1c1.5.9 3.6 1.4 6 1.4s4.5-.5 6-1.4V12zm-6-4C8.1 8 6 6.7 6 6s2.1-2 6-2 6 1.3 6 2-2.1 2-6 2z"/></svg>',
  cache: '<svg viewBox="0 0 24 24"><path d="M13 2 4 14h6l-1 8 9-12h-6l1-8z"/></svg>',
  mail: '<svg viewBox="0 0 24 24"><path d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm0 2v.5l8 5 8-5V6H4zm16 3-8 5-8-5v9h16V9z"/></svg>',
  pkg: '<svg viewBox="0 0 24 24"><path d="M12 2 3 6.5v11L12 22l9-4.5v-11L12 2zm0 2.2 6.6 3.3L12 10.8 5.4 7.5 12 4.2zM5 9.2l6 3v7.3l-6-3V9.2zm8 10.3v-7.3l6-3v7.3l-6 3z"/></svg>',
  site: '<svg viewBox="0 0 24 24"><path d="M10 4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h6zm-6 4v10h16V8H4z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>',
  stop: '<svg viewBox="0 0 24 24"><path d="M6 6h12v12H6z"/></svg>',
  refresh: '<svg viewBox="0 0 24 24"><path d="M12 4a8 8 0 0 1 7.5 5.2l1.9-.7A10 10 0 0 0 3.1 7.9L1.5 6.5V12h5.5L4.8 9.8A8 8 0 0 1 12 4zm7 8-2.2 2.2A8 8 0 0 1 4.5 14.8l-1.9.7A10 10 0 0 0 20.9 16.1l1.6 1.4V12H17z"/></svg>',
  open: '<svg viewBox="0 0 24 24"><path d="M14 3h7v7h-2V6.4l-9.3 9.3-1.4-1.4L17.6 5H14V3zM5 5h6v2H5v12h12v-6h2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M10 4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h6z"/></svg>',
  edit: '<svg viewBox="0 0 24 24"><path d="M3 17.3V21h3.7L17.8 9.9l-3.7-3.7L3 17.3zm17.7-10.2a1 1 0 0 0 0-1.4l-2.4-2.4a1 1 0 0 0-1.4 0l-1.8 1.8 3.7 3.7 1.9-1.7z"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M9 3h6l1 2h4v2H4V5h4l1-2zm-3 6h12l-1 12H7L6 9zm4 2v8h2v-8h-2zm4 0v8h-2v-8h2z"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M18.3 5.7 12 12l6.3 6.3-1.4 1.4L12 13.4l-6.3 6.3-1.4-1.4L10.6 12 4.3 5.7l1.4-1.4L12 10.6l6.3-6.3z"/></svg>',
};
const SERVICE_ICON = { web: "web", php: "php", db: "db", cache: "cache", mail: "mail" };
const SERVICE_KIND = { web: { caddy: "caddy", nginx: "nginx", apache: "apache" }, php: "php", db: { mariadb: "mariadb", mysql: "mysql" }, cache: "cache", mail: "mailpit" };

let state = null, lang = "en", upstream = null, builtin = null, upstreamLoading = false;
let installing = null; // { kind, downloaded, total, phase: "download"|"extract" }
let removing = null;   // { kind, version }
async function loadUpstream() {
  const btn = $("#btn-check-upstream");
  const label = $("#btn-check-upstream-label");
  upstreamLoading = true; btn.disabled = true; btn.classList.add("busy"); label.textContent = t("btn.checking");
  try { upstream = await invoke("upstream_versions"); } catch (e) { fail(e); }
  upstreamLoading = false; btn.disabled = false; btn.classList.remove("busy"); label.textContent = t("btn.checkLatest");
  renderPackages();
}

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const t = (key, vars = {}) => {
  const dict = window.PADU_I18N[lang] || window.PADU_I18N.en;
  let s = dict[key] ?? window.PADU_I18N.en[key] ?? key;
  for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, v);
  return s;
};
const cmpVer = (a, b) => {
  const pa = String(a).match(/\d+/g)?.map(Number) || [], pb = String(b).match(/\d+/g)?.map(Number) || [];
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) { const d = (pa[i] ?? -1) - (pb[i] ?? -1); if (d) return d; }
  return 0;
};

const activity = [];
function logActivity(msg, cls) {
  const d = new Date();
  activity.unshift({ t: `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`, msg, cls });
  if (activity.length > 100) activity.pop();
  renderActivity();
}
function renderActivity() {
  const ul = $("#activity-list");
  if (!ul) return;
  ul.innerHTML = activity.map((a) => `<li class="${a.cls}"><time>${a.t}</time><i></i><span>${esc(a.msg)}</span></li>`).join("") || `<div class="activity-empty">—</div>`;
}
const TOAST_ICON = {
  ok: '<svg viewBox="0 0 24 24"><path d="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z"/></svg>',
  bad: '<svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/></svg>',
  info: '<svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/></svg>',
};
function toast(msg, cls = "") {
  const box = $("#toasts");
  // keep at most 3 on screen; drop the oldest
  while (box.children.length >= 3) box.firstChild.remove();
  const el = document.createElement("div");
  el.className = `toast ${cls}`;
  el.innerHTML = `${TOAST_ICON[cls] || TOAST_ICON.info}<div class="body"></div>`;
  $(".body", el).textContent = msg;
  const close = () => { if (!el.isConnected) return; el.classList.add("out"); setTimeout(() => el.remove(), 180); };
  el.onclick = close;
  box.appendChild(el);
  setTimeout(close, cls === "bad" ? 6000 : 2800);
  logActivity(msg, cls === "bad" ? "bad" : cls === "ok" ? "ok" : "");
}
const fail = (e) => { const m = typeof e === "string" ? e : e?.message || String(e); toast(m, "bad"); console.error(e); };
window.addEventListener("error", (ev) => toast(`UI error: ${ev.message} (${(ev.filename || "").split("/").pop()}:${ev.lineno})`, "bad"));
window.addEventListener("unhandledrejection", (ev) => toast(`UI error: ${ev.reason?.message || ev.reason}`, "bad"));
function askConfirm(text, okLabel) {
  return new Promise((resolve) => {
    const dlg = $("#dlg-confirm");
    $("#confirm-text").textContent = text;
    $("#btn-confirm-ok").textContent = okLabel || t("btn.remove");
    const done = (v) => { dlg.close(); resolve(v); };
    dlg.querySelector("form").onsubmit = (ev) => { ev.preventDefault(); done(true); };
    $("#btn-confirm-cancel").onclick = () => done(false);
    dlg.oncancel = (ev) => { ev.preventDefault(); done(false); };
    dlg.showModal();
  });
}
const applyI18n = () => { $$("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); }); document.documentElement.lang = lang; };

// theme: system -> light -> dark
function setTheme(mode) {
  try { if (mode === "system") localStorage.removeItem("padu.theme"); else localStorage.setItem("padu.theme", mode); } catch (e) {}
  if (mode === "system") delete document.documentElement.dataset.theme; else document.documentElement.dataset.theme = mode;
}
$("#theme-toggle").onclick = () => {
  const cur = document.documentElement.dataset.theme || "system";
  const dark = cur === "dark" || (cur === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  setTheme(dark ? "light" : "dark");
};

let lastSnapshot = "";
async function refresh(force = false) {
  try {
    const next = await invoke("get_state");
    const snap = JSON.stringify(next);
    if (!force && snap === lastSnapshot) return;   // nothing changed: leave the DOM alone (no scroll jank)
    lastSnapshot = snap;
    state = next;
    lang = state.config.general.language || "en";
    applyI18n();
    $("#version").textContent = `v${state.version}`;
    for (const [name, fn] of [["services", renderServices], ["packages", renderPackages], ["sites", renderSites], ["database", renderDatabase], ["settings", renderSettings]]) {
      try { fn(); } catch (e) { fail(`${name}: ${e?.message || e}`); }
    }
  } catch (e) { fail(e); }
}
function showTab(id) {
  $$(".nav-item").forEach((x) => x.classList.toggle("active", x.dataset.tab === id));
  $$(".tab").forEach((x) => x.classList.toggle("active", x.id === `tab-${id}`));
  if (id === "logs") { renderActivity(); loadLogList(); }
  if (id === "packages" && !upstream && !upstreamLoading) loadUpstream();
}

/* ---------------- dashboard ---------------- */
const isHidden = (id) => (state.config.general.hidden_services || []).includes(id);
function renderServices() {
  const visible = state.services.filter((s) => !isHidden(s.id));
  const enabled = visible.filter((s) => s.enabled);
  const running = enabled.filter((s) => s.running).length;
  const pill = $("#global-status");
  pill.classList.toggle("on", running > 0 && running === enabled.length);
  pill.classList.toggle("partial", running > 0 && running < enabled.length);
  $(".txt", pill).textContent = running === 0 ? t("status.stopped") : running === enabled.length ? t("status.running") : t("status.partial");
  $("#hosts-notice").classList.toggle("hidden", state.hosts_in_sync);

  $$("[data-open='adminer']").forEach((b) => b.classList.toggle("hidden", isHidden("db")));
  $$("[data-open='mail']").forEach((b) => b.classList.toggle("hidden", isHidden("mail")));
  if (!visible.length) { $("#services").innerHTML = `<div class="empty">${t("dash.empty")}</div>`; return; }
  $("#services").innerHTML = visible.map((s) => {
    const kind = typeof SERVICE_KIND[s.id] === "string" ? SERVICE_KIND[s.id] : SERVICE_KIND[s.id][s.id === "web" ? state.config.web.server : state.config.db.engine];
    const versions = state.installed.filter((i) => i.kind === kind).map((i) => i.version).sort(cmpVer).reverse();
    const cls = s.running ? "on" : s.installed ? "" : "missing";
    const version = versions.length > 1
      ? `<span class="pill"><select data-version-kind="${kind}" data-service="${s.id}">${versions.map((v) => `<option ${v === s.version ? "selected" : ""}>${esc(v)}</option>`).join("")}</select></span>`
      : s.installed ? `<span class="pill">${esc(s.version)}</span>`
      : installing && installing.kind === kind ? `<span data-dash="${kind}">${progressMarkup(installing)}</span>`
      : `<button class="btn small" data-install="${kind}">${t("btn.install")}</button>`;
    const ports = s.ports.map((p) => `<span class="pill">:${p}</span>`).join("");
    const action = s.running
      ? `<button class="icon-btn" data-action="${s.id === "web" ? "reload" : "restart"}" data-id="${s.id}" title="${s.id === "web" ? t("btn.reload") : t("btn.restart")}">${ICONS.refresh}</button>
         <button class="icon-btn" data-action="stop" data-id="${s.id}" title="${t("btn.stop")}">${ICONS.stop}</button>`
      : `<button class="icon-btn" data-action="start" data-id="${s.id}" title="${t("btn.start")}" ${s.installed ? "" : "disabled"}>${ICONS.play}</button>`;
    return `<div class="row ${cls}">
      <div class="ico">${ICONS[SERVICE_ICON[s.id]]}</div>
      <div><div class="name">${esc(s.name)} ${version}</div><div class="meta">${ports}${s.message && !s.installed ? "" : s.message ? `<span>${esc(s.message)}</span>` : ""}</div></div>
      <div class="act"><input type="checkbox" class="switch" data-enable="${s.id}" ${s.enabled ? "checked" : ""} title="${t("settings.enabled")}">${action}</div>
    </div>`;
  }).join("");
}

async function serviceAction(id, action) {
  try { await invoke("service_action", { id, action }); } catch (e) { fail(e); }
  refresh(true);
}
function reportResults(results) {
  const bad = results.filter((r) => !r.ok);
  if (!bad.length) { toast("OK", "ok"); return; }
  bad.forEach((r) => logActivity(`${r.id}: ${r.error}`, "bad"));
  const allMissing = bad.every((r) => /not installed/i.test(r.error));
  toast(allMissing ? t("msg.installFirst") : `${t("msg.startFailed")}:\n${bad.map((r) => `${r.id}: ${r.error.split("\n")[0]}`).join("\n")}`, "bad");
}


/* ---------------- customize dashboard ---------------- */
$("#btn-customize").onclick = () => {
  const f = $("#form-customize");
  f.elements.show_web.checked = !isHidden("web"); f.elements.show_php.checked = !isHidden("php");
  f.elements.show_db.checked = !isHidden("db"); f.elements.show_cache.checked = !isHidden("cache"); f.elements.show_mail.checked = !isHidden("mail");
  f.elements.web_server.value = state.config.web.server; f.elements.db_engine.value = state.config.db.engine;
  $("#dlg-customize").showModal();
};
$("#btn-customize-cancel").onclick = () => $("#dlg-customize").close();
$("#form-customize").onsubmit = async (ev) => {
  ev.preventDefault(); const f = ev.target;
  const cfg = JSON.parse(JSON.stringify(state.config));
  const hidden = [];
  for (const [id, box, sec] of [["web", "show_web", "web"], ["php", "show_php", "php"], ["db", "show_db", "db"], ["cache", "show_cache", "cache"], ["mail", "show_mail", "mail"]]) {
    const on = f.elements[box].checked;
    if (!on) hidden.push(id);
    cfg[sec].enabled = on;            // visible => enabled, hidden => disabled
  }
  cfg.general.hidden_services = hidden;
  cfg.web.server = f.elements.web_server.value;
  cfg.db.engine = f.elements.db_engine.value;
  try { await invoke("save_config", { config: cfg }); $("#dlg-customize").close(); toast(t("msg.saved"), "ok"); } catch (e) { fail(e); }
  refresh(true);
};


/* ---------------- close behaviour ---------------- */
listen("close-requested", () => {
  const dlg = $("#dlg-close");
  $("#form-close").elements.remember.checked = false;
  if (!dlg.open) dlg.showModal();
});
async function closeWith(action) {
  const remember = $("#form-close").elements.remember.checked;
  $("#dlg-close").close();
  try { await invoke("close_decision", { action, remember }); } catch (e) { fail(e); }
  if (remember) refresh(true);
}
$("#btn-close-tray").onclick = () => closeWith("tray");
$("#btn-close-quit").onclick = () => closeWith("quit");
$("#dlg-close").oncancel = (ev) => { ev.preventDefault(); $("#dlg-close").close(); };

/* ---------------- packages ---------------- */
const GROUPS = [
  ["pkg.group.web", ["caddy", "nginx", "apache"]],
  ["pkg.group.php", ["php", "composer"]],
  ["pkg.group.db", ["mariadb", "mysql", "adminer", "phpmyadmin"]],
  ["pkg.group.cache", ["cache"]],
  ["pkg.group.mail", ["mailpit"]],
  ["pkg.group.tools", ["node", "cabundle"]],
];
function renderPackages() {
  const label = Object.fromEntries(KINDS);
  $("#packages").innerHTML = GROUPS.map(([gkey, kinds]) => {
    const rows = kinds.map((kind) => {
      const inst = state.installed.filter((i) => i.kind === kind).map((i) => i.version).sort(cmpVer).reverse();
      const ups = upstream ? upstream.filter((p) => p.kind === kind) : [];
      const versions = [...new Set([...ups.map((p) => p.version), ...(builtin ? builtin.filter((p) => p.kind === kind).map((p) => p.version) : [])])].sort(cmpVer).reverse();
      const newest = versions[0] || "";
      if (!versions.length && !inst.length) return "";
      const source = ups[0]?.source || builtin?.find((p) => p.kind === kind)?.source || "";
      const chips = inst.map((v) => removing && removing.kind === kind && removing.version === v
        ? `<span class="chip ok removing">${esc(v)} · ${t("pkg.removing")}</span>`
        : `<span class="chip ok">${esc(v)}<button class="icon-btn small" data-remove="${kind}" data-version="${esc(v)}" title="${t("pkg.remove", { version: v })}">${ICONS.trash}</button></span>`).join("");
      const isInstalling = installing && installing.kind === kind;
      const act = isInstalling
        ? progressMarkup(installing)
        : versions.length
          ? `<span class="pkg-ver"><select data-pkg-select="${kind}">${versions.map((v) => `<option value="${esc(v)}" ${inst.includes(v) ? "disabled" : ""}>${esc(v)}${v === newest ? " ★" : ""}${inst.includes(v) ? " ✓" : ""}</option>`).join("")}</select>
             <button class="btn small primary" data-install-selected="${kind}" ${versions.every((v) => inst.includes(v)) ? "disabled" : ""}>${t("btn.install")}</button></span>`
          : "";
      return `<div class="row pkg ${isInstalling ? "installing" : inst.length ? "on" : "missing"}" data-row="${kind}">
        <div class="ico">${ICONS.pkg}</div>
        <div class="pkg-main">
          <div class="pkg-head"><span class="name">${label[kind] || kind}</span>${source ? `<span class="src">${esc(source)}</span>` : ""}</div>
          <div class="pkg-installed">${chips || `<span class="none">${isInstalling ? t("pkg.installing") : t("pkg.notInstalled")}</span>`}</div>
        </div>
        <div class="act">${act}</div>
      </div>`;
    }).join("");
    return rows ? `<section class="group"><h3>${t(gkey)}</h3><div class="list">${rows}</div></section>` : "";
  }).join("");
  // preselect the first installable version per select
  $$("[data-pkg-select]").forEach((sel) => { const first = [...sel.options].find((o) => !o.disabled); if (first) sel.value = first.value; });
}
function progressText(p) {
  if (p.phase === "extract") return t("pkg.extracting");
  const mb = (p.downloaded / 1048576).toFixed(1);
  return p.total ? `${(100 * p.downloaded / p.total).toFixed(0)}% · ${mb}/${(p.total / 1048576).toFixed(1)} MiB` : `${mb} MiB`;
}
function progressMarkup(p) {
  const pct = p.phase === "extract" ? 100 : p.total ? (100 * p.downloaded / p.total) : 0;
  return `<div class="row-progress"><div class="bar"><div class="fill ${p.phase === "extract" ? "indeterminate" : ""}" style="width:${pct.toFixed(1)}%"></div></div><span class="txt">${progressText(p)}</span></div>`;
}
function paintProgress() {
  if (!installing) return;
  const html = progressMarkup(installing);
  const row = $(`[data-row="${installing.kind}"] .act`);
  if (row) row.innerHTML = html;
  const dash = $(`[data-dash="${installing.kind}"]`);
  if (dash) dash.innerHTML = html;
}
async function installPackage(kind, latest, version = null) {
  if (installing) { toast(t("pkg.installing")); return; }
  installing = { kind, downloaded: 0, total: null, phase: "download" };
  renderPackages(); renderServices(); paintProgress();
  try {
    const i = await invoke("install_package", { kind, version, latest: !!latest });
    toast(t("msg.installed", { name: i.kind, version: i.version }), "ok");
  } catch (e) { fail(e); }
  installing = null;
  refresh();
}
listen("install-progress", (ev) => {
  const p = ev.payload;
  if (!installing || installing.kind !== p.kind) return;
  installing.downloaded = p.downloaded; installing.total = p.total ?? installing.total;
  // download finished -> extraction phase until the command returns
  installing.phase = installing.total && installing.downloaded >= installing.total ? "extract" : "download";
  paintProgress();
});
listen("services-changed", refresh);
listen("backup-done", (ev) => toast(t("msg.backup", { path: ev.payload }), "ok"));

/* ---------------- sites ---------------- */
function renderSites() {
  const root = state.config.general.document_root || `${state.root}/www`;
  $("#sites-hint").textContent = t("sites.hint", { root, tld: state.config.general.tld });
  $("#sites").innerHTML = state.sites.map((s) => `<div class="row on">
    <div class="ico">${ICONS.site}</div>
    <div><div class="name">${esc(s.hostname)}${s.auto ? "" : ' <span class="pill">manual</span>'}</div><div class="meta"><span class="mono">${esc(s.root)}</span></div></div>
    <div class="act">
      <button class="icon-btn" data-open="site:${esc(s.hostname)}" title="Open">${ICONS.open}</button>
      <button class="icon-btn" data-open="path:${esc(s.path)}" title="Folder">${ICONS.folder}</button>
      <button class="icon-btn" data-open="edit:${esc(s.path)}" title="Edit">${ICONS.edit}</button>
      ${s.auto ? `<button class="icon-btn danger" data-delete-project="${esc(s.name)}" title="Delete">${ICONS.trash}</button>` : `<button class="icon-btn danger" data-remove-site="${esc(s.hostname)}" title="${t("btn.remove")}">${ICONS.x}</button>`}
    </div></div>`).join("") || `<div class="empty">${ICONS.site}<span>${esc(root)}</span></div>`;
}
async function syncSites() {
  try { const r = await invoke("sync_sites"); if (r.missing_hosts.length) await invoke("hosts_sync"); toast(t("msg.synced"), "ok"); } catch (e) { fail(e); }
  refresh();
}

/* ---------------- database ---------------- */
function renderDatabase() {
  $("#db-host").textContent = `127.0.0.1:${state.config.db.port}`;
  if (!state.db_password_known) { $("#db-password").textContent = "—"; $("#db-password").title = t("db.unknownPassword"); $("#db-password").classList.remove("secret"); }
}


/* ---------------- logs ---------------- */
let currentLog = "";
async function loadLogList() {
  try {
    const files = await invoke("list_logs");
    const sel = $("#log-file");
    const prev = currentLog || sel.value;
    sel.innerHTML = files.map((f) => `<option value="${esc(f.name)}">${esc(f.name)} · ${(f.size / 1024).toFixed(0)} KB</option>`).join("") || `<option value="">—</option>`;
    if (files.some((f) => f.name === prev)) sel.value = prev;
    currentLog = sel.value;
    await loadLog();
  } catch (e) { fail(e); }
}
async function loadLog() {
  const view = $("#log-view");
  view.dataset.empty = t("logs.empty");
  if (!currentLog) { view.textContent = ""; return; }
  try {
    const text = await invoke("read_log", { name: currentLog, lines: 400 });
    view.innerHTML = text.split("\n").map((l) => {
      const cls = /error|fatal|panic|denied|failed/i.test(l) ? "ln-err" : /warn/i.test(l) ? "ln-warn" : "";
      return cls ? `<span class="${cls}">${esc(l)}</span>` : esc(l);
    }).join("\n");
    view.scrollTop = view.scrollHeight;
  } catch (e) { fail(e); }
}
$("#log-file").onchange = (ev) => { currentLog = ev.target.value; loadLog(); };
$("#btn-log-refresh").onclick = loadLogList;
$("#btn-log-clear").onclick = async () => { if (!currentLog) return; try { await invoke("clear_log", { name: currentLog }); await loadLog(); } catch (e) { fail(e); } };
$("#btn-activity-clear").onclick = () => { activity.length = 0; renderActivity(); };

/* ---------------- settings ---------------- */
function renderSettings() {
  $("#root-path").textContent = state.root; $("#root-path").title = state.root;
  for (const el of $("#form-settings").elements) {
    if (!el.name) continue;
    const [sec, key] = el.name.split(".");
    const v = state.config[sec]?.[key];
    if (el.type === "checkbox") el.checked = !!v; else el.value = v ?? "";
  }
  const versions = state.installed.filter((i) => i.kind === "php").map((i) => i.version).sort(cmpVer).reverse();
  $("#php-version").innerHTML = `<option value="">auto</option>${versions.map((v) => `<option ${v === state.config.php.version ? "selected" : ""}>${esc(v)}</option>`).join("")}`;
}
function readSettings() {
  const cfg = JSON.parse(JSON.stringify(state.config));
  for (const el of $("#form-settings").elements) {
    if (!el.name) continue;
    const [sec, key] = el.name.split(".");
    cfg[sec][key] = el.type === "checkbox" ? el.checked : el.type === "number" ? Number(el.value) : el.value;
  }
  return cfg;
}

/* ---------------- events ---------------- */
document.addEventListener("click", async (ev) => {
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.tab) { showTab(b.dataset.tab); return; }
  if (b.dataset.open) { try { await invoke("open_target", { what: b.dataset.open }); } catch (e) { fail(e); } return; }
  if (b.dataset.copy) {
    let text = $(b.dataset.copy).textContent;
    if (b.dataset.copySecret) text = (await invoke("db_password")) || "";
    if (text) { await navigator.clipboard.writeText(text); toast(t("msg.copied"), "ok"); }
    return;
  }
  if (b.dataset.action) { b.disabled = true; await serviceAction(b.dataset.id, b.dataset.action); return; }
  if (b.dataset.install) { await installPackage(b.dataset.install, b.dataset.latest); return; }
  if (b.dataset.installSelected) {
    const sel = $(`[data-pkg-select="${b.dataset.installSelected}"]`);
    const v = sel?.value || null;
    await installPackage(b.dataset.installSelected, false, v); return;
  }
  if (b.dataset.remove) {
    const kind = b.dataset.remove, version = b.dataset.version;
    if (!(await askConfirm(t("msg.confirmRemove", { name: kind, version })))) return;
    removing = { kind, version }; renderPackages();
    try { await invoke("remove_package", { kind, version }); toast(t("msg.removed", { name: kind, version }), "ok"); } catch (e) { fail(e); }
    removing = null; refresh(); return;
  }
  if (b.dataset.deleteProject) {
    if (!(await askConfirm(t("msg.confirmDelete", { name: b.dataset.deleteProject })))) return;
    try { await invoke("delete_project", { name: b.dataset.deleteProject }); await syncSites(); } catch (e) { fail(e); }
    return;
  }
  if (b.dataset.removeSite) { try { await invoke("remove_site", { hostname: b.dataset.removeSite }); await syncSites(); } catch (e) { fail(e); } }
});

document.addEventListener("change", async (ev) => {
  const el = ev.target;
  if (el.dataset.enable) {
    const cfg = JSON.parse(JSON.stringify(state.config));
    cfg[el.dataset.enable].enabled = el.checked;
    try { await invoke("save_config", { config: cfg }); } catch (e) { fail(e); }
    refresh();
  }
  if (el.dataset.versionKind) {
    const cfg = JSON.parse(JSON.stringify(state.config));
    const map = { php: "php", web: "web", db: "db", cache: "cache", mail: "mail" };
    cfg[map[el.dataset.service]].version = el.value;
    try { await invoke("save_config", { config: cfg }); toast(t("msg.saved"), "ok"); } catch (e) { fail(e); }
    refresh();
  }
});

$("#btn-start-all").onclick = async () => { $("#btn-start-all").disabled = true; try { reportResults(await invoke("start_all")); } catch (e) { fail(e); } $("#btn-start-all").disabled = false; refresh(); };
$("#btn-stop-all").onclick = async () => { try { reportResults(await invoke("stop_all")); } catch (e) { fail(e); } refresh(); };
$("#btn-hosts-sync").onclick = async () => { try { await invoke("hosts_sync"); toast("OK", "ok"); } catch (e) { fail(e); } refresh(); };
$("#btn-sync-sites").onclick = syncSites;
$("#btn-check-upstream").onclick = loadUpstream;
$("#btn-db-show").onclick = async () => {
  const el = $("#db-password");
  if (!state.db_password_known) return;
  if (el.classList.contains("secret")) { el.textContent = (await invoke("db_password")) || ""; el.classList.remove("secret"); }
  else { el.textContent = "••••••••••••"; el.classList.add("secret"); }
};
$("#btn-db-rotate").onclick = async () => { try { const pw = await invoke("db_set_password", { new: null }); $("#db-password").textContent = pw; $("#db-password").classList.remove("secret"); toast("OK", "ok"); } catch (e) { fail(e); } };
$("#btn-db-backup").onclick = async () => { try { const p = await invoke("db_backup"); toast(t("msg.backup", { path: p }), "ok"); } catch (e) { fail(e); } };
$("#form-db").onsubmit = async (ev) => {
  ev.preventDefault(); const name = ev.target.elements.name.value.trim();
  try { await invoke("db_create", { name }); toast(t("msg.created", { name }), "ok"); ev.target.reset(); const l = await invoke("db_list"); $("#db-list").innerHTML = l.map((d) => `<span class="chip">${esc(d)}</span>`).join(""); } catch (e) { fail(e); }
};
$("#form-site").onsubmit = async (ev) => {
  ev.preventDefault(); const f = ev.target;
  try { await invoke("add_site", { hostname: f.elements.hostname.value.trim(), path: f.elements.path.value.trim(), public: f.elements.public.value.trim() }); f.reset(); await syncSites(); } catch (e) { fail(e); }
};
$("#form-settings").onsubmit = async (ev) => { ev.preventDefault(); try { await invoke("save_config", { config: readSettings() }); toast(t("msg.saved"), "ok"); } catch (e) { fail(e); } refresh(); };
$("#btn-ssl-trust").onclick = async () => {
  try { const r = await invoke("ssl_trust"); toast(t("msg.trust", { ok: r.filter((x) => x.ok).length, total: r.length }), "ok"); r.filter((x) => !x.ok).forEach((x) => toast(`${x.description}: ${x.error}`, "bad")); } catch (e) { fail(e); }
};
/* ---------------- app updates ---------------- */
let pendingUpdate = null;
function showUpdate(info) {
  pendingUpdate = info;
  const b = $("#update-banner");
  $("#update-version").textContent = t("update.available", { version: info.version });
  $("#update-notes").textContent = info.installable ? (info.notes || "").split("\n")[0] : t("update.manual");
  $("#btn-update-install").classList.toggle("hidden", !info.installable);
  $("#update-progress").classList.add("hidden");
  b.classList.remove("hidden");
}
async function checkForUpdate(interactive) {
  try {
    const r = await invoke("update_check");
    if (r) showUpdate(r); else if (interactive) toast(t("msg.upToDate", { version: state.version }), "ok");
  } catch (e) { if (interactive) fail(e); }
}
$("#btn-check-update").onclick = () => checkForUpdate(true);
$("#btn-update-dismiss").onclick = () => $("#update-banner").classList.add("hidden");
$("#btn-update-page").onclick = () => { if (pendingUpdate?.url) invoke("open_url_external", { url: pendingUpdate.url }).catch(() => invoke("open_target", { what: "web" })); };
$("#btn-update-install").onclick = async () => {
  const btn = $("#btn-update-install"); btn.disabled = true;
  const prog = $("#update-progress"); prog.classList.remove("hidden"); $(".fill", prog).style.width = "0%"; $(".txt", prog).textContent = "";
  try { await invoke("update_install"); toast(t("update.restarting"), "ok"); } catch (e) { fail(e); btn.disabled = false; }
};
listen("update-progress", (ev) => {
  const p = ev.payload, prog = $("#update-progress");
  if (p.total) { $(".fill", prog).style.width = `${(100 * p.downloaded / p.total).toFixed(0)}%`; $(".txt", prog).textContent = `${(p.downloaded / 1048576).toFixed(1)} / ${(p.total / 1048576).toFixed(1)} MiB`; }
  else $(".txt", prog).textContent = `${(p.downloaded / 1048576).toFixed(1)} MiB`;
  if (p.done) $(".txt", prog).textContent = t("update.restarting");
});
listen("update-available", (ev) => showUpdate(ev.payload));
listen("update-auto", () => { const b = $("#btn-update-install"); b.disabled = true; $("#update-notes").textContent = t("update.auto"); $("#update-progress").classList.remove("hidden"); });
listen("update-error", (ev) => { fail(ev.payload); $("#btn-update-install").disabled = false; });

// new project dialog
$("#btn-new-project").onclick = () => {
  const sel = $("#project-template");
  sel.innerHTML = state.templates.map(([id, label]) => `<option value="${id}">${esc(label)}</option>`).join("");
  sel.onchange(); $("#project-url").textContent = ""; $("#dlg-project").showModal();
};
$("#project-template").onchange = function () {
  const v = this.value;
  $("#project-arg-row").classList.toggle("hidden", !(v === "composer" || v === "git"));
  $("#project-arg-label").textContent = v === "git" ? "URL" : "Package";
  $("#form-project").elements.arg.placeholder = v === "git" ? "https://github.com/user/repo.git" : "vendor/package";
};
$("#form-project").elements.name.oninput = (ev) => { $("#project-url").textContent = ev.target.value ? `${state.scheme}://${ev.target.value}.${state.config.general.tld}` : ""; };
$("#btn-project-cancel").onclick = () => $("#dlg-project").close();
$("#form-project").onsubmit = async (ev) => {
  ev.preventDefault(); const f = ev.target;
  const req = { name: f.elements.name.value.trim(), template: f.elements.template.value, arg: f.elements.arg.value.trim(), create_db: f.elements.create_db.checked };
  const btn = $("button.primary", f); btn.disabled = true;
  try { const p = await invoke("create_project", { req }); toast(t("msg.created", { name: p }), "ok"); $("#dlg-project").close(); f.reset(); await syncSites(); } catch (e) { fail(e); }
  btn.disabled = false;
};

(async () => {
  await refresh();
  if (state?.ui_theme === "light" || state?.ui_theme === "dark") setTheme(state.ui_theme);
  if (state?.ui_tab) showTab(state.ui_tab);
  try { builtin = await invoke("builtin_versions"); renderPackages(); } catch (e) { fail(e); }
  setInterval(() => { if (document.visibilityState !== "visible" || installing || removing) return; refresh(); if ($("#tab-logs").classList.contains("active")) loadLog(); }, 15000);
})();
