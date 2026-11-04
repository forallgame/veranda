/* Veranda UI - vanilla JS, no build step. Talks to the Rust side through Tauri IPC. */
// Apply the saved theme as early as possible (no inline script: keeps the CSP strict).
try { const saved = localStorage.getItem("veranda.theme"); if (saved === "light" || saved === "dark") document.documentElement.dataset.theme = saved; } catch (e) {}
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const ICONS = {
  web: '<svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm7.9 9h-3a15 15 0 0 0-1.4-5.6A8 8 0 0 1 19.9 11zM12 4c.9 1 2.1 3.3 2.4 7H9.6C9.9 7.3 11.1 5 12 4zM4.1 13h3c.2 2.1.6 4 1.3 5.6A8 8 0 0 1 4.1 13zm3-2h-3a8 8 0 0 1 4.3-5.6A15 15 0 0 0 7.1 11zM12 20c-.9-1-2.1-3.3-2.4-7h4.8c-.3 3.7-1.5 6-2.4 7zm2.6-1.4c.7-1.6 1.1-3.5 1.3-5.6h3a8 8 0 0 1-4.3 5.6z"/></svg>',
  php: '<svg viewBox="0 0 24 24"><path d="M4 7h16a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2zm1.5 2.5v5h1.4v-1.6h1c1.3 0 2.1-.7 2.1-1.7s-.8-1.7-2.1-1.7H5.5zm1.4 1.1h.9c.5 0 .8.2.8.6s-.3.6-.8.6h-.9v-1.2zm3.9-1.1v5h1.4v-1.9h1.6v1.9h1.4v-5h-1.4v2h-1.6v-2h-1.4zm5.3 0v5h1.4v-1.6h1c1.3 0 2.1-.7 2.1-1.7s-.8-1.7-2.1-1.7h-2.4zm1.4 1.1h.9c.5 0 .8.2.8.6s-.3.6-.8.6h-.9v-1.2z"/></svg>',
  db: '<svg viewBox="0 0 24 24"><path d="M12 2C7 2 4 3.8 4 6v12c0 2.2 3 4 8 4s8-1.8 8-4V6c0-2.2-3-4-8-4zm6 16c0 .7-2.1 2-6 2s-6-1.3-6-2v-2.9c1.5.9 3.6 1.4 6 1.4s4.5-.5 6-1.4V18zm0-6c0 .7-2.1 2-6 2s-6-1.3-6-2V9.1c1.5.9 3.6 1.4 6 1.4s4.5-.5 6-1.4V12zm-6-4C8.1 8 6 6.7 6 6s2.1-2 6-2 6 1.3 6 2-2.1 2-6 2z"/></svg>',
  cache: '<svg viewBox="0 0 24 24"><path d="M13 2 4 14h6l-1 8 9-12h-6l1-8z"/></svg>',
  mail: '<svg viewBox="0 0 24 24"><path d="M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm0 2v.5l8 5 8-5V6H4zm16 3-8 5-8-5v9h16V9z"/></svg>',
  pkg: '<svg viewBox="0 0 24 24"><path d="M12 2 3 6.5v11L12 22l9-4.5v-11L12 2zm0 2.2 6.6 3.3L12 10.8 5.4 7.5 12 4.2zM5 9.2l6 3v7.3l-6-3V9.2zm8 10.3v-7.3l6-3v7.3l-6 3z"/></svg>',
  node: '<svg viewBox="0 0 24 24"><path d="M12 2 3 7v10l9 5 9-5V7l-9-5zm0 2.3L18.6 8 12 11.7 5.4 8 12 4.3zM5 9.7l6 3.4v6.6l-6-3.3V9.7zm8 10v-6.6l6-3.4v6.7l-6 3.3z"/></svg>',
  tool: '<svg viewBox="0 0 24 24"><path d="M21.7 18.3 14.6 11a6 6 0 0 0-7.9-7.4l3.6 3.6-1.4 2.8-2.8 1.4L2.5 7.8A6 6 0 0 0 10 15.6l7.3 7.1a1.5 1.5 0 0 0 2.1 0l2.3-2.3a1.5 1.5 0 0 0 0-2.1z"/></svg>',
  shield: '<svg viewBox="0 0 24 24"><path d="M12 2 4 5v6c0 5 3.4 9.7 8 11 4.6-1.3 8-6 8-11V5l-8-3zm-1 14-3.5-3.5 1.4-1.4L11 13.2l4.6-4.6 1.4 1.4L11 16z"/></svg>',
  site: '<svg viewBox="0 0 24 24"><path d="M10 4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h6zm-6 4v10h16V8H4z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>',
  spinner: '<svg class="spin" viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 10 10h-2.5A7.5 7.5 0 1 1 12 4.5z"/></svg>',
  stop: '<svg viewBox="0 0 24 24"><path d="M6 6h12v12H6z"/></svg>',
  refresh: '<svg viewBox="0 0 24 24"><path d="M12 4a8 8 0 0 1 7.5 5.2l1.9-.7A10 10 0 0 0 3.1 7.9L1.5 6.5V12h5.5L4.8 9.8A8 8 0 0 1 12 4zm7 8-2.2 2.2A8 8 0 0 1 4.5 14.8l-1.9.7A10 10 0 0 0 20.9 16.1l1.6 1.4V12H17z"/></svg>',
  open: '<svg viewBox="0 0 24 24"><path d="M14 3h7v7h-2V6.4l-9.3 9.3-1.4-1.4L17.6 5H14V3zM5 5h6v2H5v12h12v-6h2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M10 4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h6z"/></svg>',
  edit: '<svg viewBox="0 0 24 24"><path d="M3 17.3V21h3.7L17.8 9.9l-3.7-3.7L3 17.3zm17.7-10.2a1 1 0 0 0 0-1.4l-2.4-2.4a1 1 0 0 0-1.4 0l-1.8 1.8 3.7 3.7 1.9-1.7z"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M9 3h6l1 2h4v2H4V5h4l1-2zm-3 6h12l-1 12H7L6 9zm4 2v8h2v-8h-2zm4 0v8h-2v-8h2z"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M18.3 5.7 12 12l6.3 6.3-1.4 1.4L12 13.4l-6.3 6.3-1.4-1.4L10.6 12 4.3 5.7l1.4-1.4L12 10.6l6.3-6.3z"/></svg>',
  info: '<svg viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/></svg>',
  file: '<svg viewBox="0 0 24 24"><path d="M6 2h9l5 5v15H6V2zm8 1.5V8h4.5L14 3.5zM8 12h8v2H8v-2zm0 4h8v2H8v-2z"/></svg>',
};

// Every downloadable kind. `svc` links a kind to the runtime service it provides (web/php/db/cache/mail);
// tools have no service. The sidebar and the service pages are generated from this table.
const KIND_META = {
  caddy:      { label: "Caddy",          svc: "web",   group: "web",   icon: "web" },
  nginx:      { label: "Nginx",          svc: "web",   group: "web",   icon: "web" },
  apache:     { label: "Apache",         svc: "web",   group: "web",   icon: "web" },
  php:        { label: "PHP",            svc: "php",   group: "lang",  icon: "php" },
  mariadb:    { label: "MariaDB",        svc: "db",    group: "db",    icon: "db" },
  mysql:      { label: "MySQL",          svc: "db",    group: "db",    icon: "db" },
  cache:      { label: "Valkey / Redis", svc: "cache", group: "more",  icon: "cache" },
  mailpit:    { label: "Mailpit",        svc: "mail",  group: "more",  icon: "mail" },
  composer:   { label: "Composer",       group: "tools", icon: "pkg" },
  node:       { label: "Node.js",        group: "tools", icon: "node" },
  adminer:    { label: "Adminer",        group: "tools", icon: "tool", open: "adminer" },
  phpmyadmin: { label: "phpMyAdmin",     group: "tools", icon: "tool", open: "phpmyadmin" },
  cabundle:   { label: "CA bundle",      group: "tools", icon: "shield" },
};
const GROUPS = [["web", "group.web"], ["lang", "group.lang"], ["db", "group.db"], ["more", "group.more"], ["tools", "group.tools"]];
const SVC_SECTION = { web: "web", php: "php", db: "db", cache: "cache", mail: "mail" };
const SVC_ROLE = { web: "role.web", db: "role.db" };

let state = null, lang = "en", upstream = null, builtin = null, upstreamLoading = false;
let installing = null; // { kind, downloaded, total, phase: "download"|"extract" }
let removing = null;   // { kind, version }
let busy = {};         // service id -> "start"|"stop"|"restart"|"reload" while a request runs
let busyAll = null;    // "start"|"stop" while Start All / Stop All runs
let currentKind = null, svcTab = "service", currentTab = "dashboard";

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const t = (key, vars = {}) => {
  const dict = window.VERANDA_I18N[lang] || window.VERANDA_I18N.en;
  let s = dict[key] ?? window.VERANDA_I18N.en[key] ?? key;
  for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, v);
  return s;
};
const cmpVer = (a, b) => {
  const pa = String(a).match(/\d+/g)?.map(Number) || [], pb = String(b).match(/\d+/g)?.map(Number) || [];
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) { const d = (pa[i] ?? -1) - (pb[i] ?? -1); if (d) return d; }
  return 0;
};
const clone = (o) => JSON.parse(JSON.stringify(o));

/* ---------------- activity + toasts ---------------- */
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
  info: ICONS.info,
};
function toast(msg, cls = "") {
  const box = $("#toasts");
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

/* ---------------- theme ---------------- */
function setTheme(mode) {
  if (mode) document.documentElement.dataset.theme = mode; else delete document.documentElement.dataset.theme;
  try { if (mode) localStorage.setItem("veranda.theme", mode); else localStorage.removeItem("veranda.theme"); } catch (e) {}
}
$("#theme-toggle").onclick = () => {
  const dark = document.documentElement.dataset.theme === "dark" || (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
  setTheme(dark ? "light" : "dark");
};

/* ---------------- state ---------------- */
let lastSnapshot = "", uiShown = false;
function uiReady() { if (uiShown) return; uiShown = true; invoke("ui_ready").catch(() => {}); }
function renderAll() {
  for (const [name, fn] of [["nav", renderNav], ["services", renderServices], ["service", renderService], ["sites", renderSites], ["database", renderDatabase], ["settings", renderSettings]]) {
    try { fn(); } catch (e) { fail(`${name}: ${e?.message || e}`); }
  }
}
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
    renderAll();
  } catch (e) { fail(e); }
  uiReady();
}
function showTab(id) {
  currentTab = id;
  $$(".nav-item").forEach((x) => x.classList.toggle("active", x.dataset.tab === id));
  $$(".tab").forEach((x) => x.classList.toggle("active", x.id === `tab-${id}`));
  $$(".svc-item").forEach((x) => x.classList.toggle("active", id === "service" && x.dataset.svc === currentKind));
  if (id === "logs") loadLogList();
  window.scrollTo(0, 0);
}

/* ---------------- helpers over state ---------------- */
const isHidden = (id) => (state.config.general.hidden_services || []).includes(id);
const installedVersions = (kind) => state.installed.filter((i) => i.kind === kind).map((i) => i.version).sort(cmpVer).reverse();
const svcStatus = (svc) => state.services.find((s) => s.id === svc);
function activeKind(svc) {
  return { web: state.config.web.server, db: state.config.db.engine, php: "php", cache: "cache", mail: "mailpit" }[svc];
}
const isActive = (kind) => !!KIND_META[kind].svc && activeKind(KIND_META[kind].svc) === kind;
function kindOn(kind) {
  const m = KIND_META[kind];
  return !!m.svc && isActive(kind) && !!state.config[SVC_SECTION[m.svc]].enabled && !isHidden(m.svc);
}
const serviceName = (id) => (state?.services.find((s) => s.id === id) || {}).name || id;
function busyOf(svc) {
  const st = svcStatus(svc);
  if (!st) return null;
  if (busy[svc]) return busy[svc];
  if (busyAll && st.enabled && st.installed && (busyAll === "start" ? !st.running : st.running)) return busyAll;
  return null;
}
const busyText = (act) => t(act === "stop" ? "status.stopping" : act === "start" ? "status.starting" : "status.restarting");
function stateBadge(st, act) {
  if (act) return `<span class="state busy">${ICONS.spinner}${busyText(act)}</span>`;
  if (!st) return `<span class="state"><i></i>${t("svc.notSelected")}</span>`;
  if (st.running) return `<span class="state on"><i></i>${t("status.running")}</span>`;
  if (st.installed) return `<span class="state"><i></i>${t("status.stopped")}</span>`;
  return `<span class="state warn"><i></i>${t("pkg.notInstalled")}</span>`;
}

/* ---------------- sidebar ---------------- */
function renderNav() {
  $("#svc-nav").innerHTML = GROUPS.map(([g, key]) => {
    const kinds = Object.keys(KIND_META).filter((k) => KIND_META[k].group === g);
    return `<div class="sgroup"><h4>${t(key)}</h4>${kinds.map((kind) => {
      const m = KIND_META[kind];
      const st = m.svc && isActive(kind) ? svcStatus(m.svc) : null;
      const act = st ? busyOf(m.svc) : null;
      const dot = act ? "busy" : st?.running ? "run" : installedVersions(kind).length ? "inst" : "";
      const on = kindOn(kind);
      return `<div class="svc-item ${currentTab === "service" && currentKind === kind ? "active" : ""}" data-svc="${kind}" role="button" tabindex="0">
        <span class="sico">${ICONS[m.icon]}</span><span class="sname">${m.label}</span><i class="sdot ${dot}" title="${st?.running ? t("status.running") : ""}"></i>
        ${m.svc ? `<input type="checkbox" class="switch mini" data-sw="${kind}" ${on ? "checked" : ""} title="${t("svc.enable")}">` : ""}
      </div>`;
    }).join("")}</div>`;
  }).join("");
}
async function toggleKind(kind, on) {
  const m = KIND_META[kind];
  if (!m.svc) return;
  const svc = m.svc, sec = SVC_SECTION[svc];
  const cfg = clone(state.config);
  const hidden = new Set(cfg.general.hidden_services || []);
  if (on) {
    // switching engines while the old one runs: stop it first so the status stays truthful
    if ((svc === "web" || svc === "db") && activeKind(svc) !== kind && svcStatus(svc)?.running) {
      busy[svc] = "stop"; renderAll();
      try { await invoke("service_action", { id: svc, action: "stop" }); } catch (e) { fail(e); }
      delete busy[svc];
    }
    if (svc === "web") cfg.web.server = kind;
    if (svc === "db") cfg.db.engine = kind;
    cfg[sec].enabled = true;
    hidden.delete(svc);
  } else {
    cfg[sec].enabled = false;
    hidden.add(svc);
  }
  cfg.general.hidden_services = [...hidden];
  try { await invoke("save_config", { config: cfg }); } catch (e) { fail(e); }
  await refresh(true);
}

/* ---------------- overview ---------------- */
function renderServices() {
  const visible = state.services.filter((s) => !isHidden(s.id));
  const enabled = visible.filter((s) => s.enabled && s.installed);
  const running = enabled.filter((s) => s.running).length;
  const pill = $("#global-status");
  pill.classList.toggle("on", !busyAll && running > 0 && running === enabled.length);
  pill.classList.toggle("partial", !busyAll && running > 0 && running < enabled.length);
  pill.classList.toggle("busy", !!busyAll);
  $(".txt", pill).textContent = busyAll ? busyText(busyAll)
    : running === 0 ? t("status.stopped") : running === enabled.length ? t("status.running") : t("status.partial");
  for (const [id, kind] of [["#btn-start-all", "start"], ["#btn-stop-all", "stop"]]) {
    const b = $(id);
    b.disabled = !!busyAll;
    b.classList.toggle("busy", busyAll === kind);
  }
  $("#hosts-notice").classList.toggle("hidden", state.hosts_in_sync);
  $$("[data-open='adminer']").forEach((b) => b.classList.toggle("hidden", isHidden("db")));
  $$("[data-open='mail']").forEach((b) => b.classList.toggle("hidden", isHidden("mail")));
  if (!visible.length) { $("#services").innerHTML = `<div class="empty">${ICONS.info}<span>${t("dash.empty")}</span></div>`; return; }
  $("#services").innerHTML = visible.map((s) => {
    const kind = activeKind(s.id);
    const versions = installedVersions(kind);
    const act = busyOf(s.id);
    const cls = act ? "busy" : s.running ? "on" : s.installed ? "off" : "missing";
    const version = versions.length > 1
      ? `<span class="pill"><select data-version-kind="${kind}" data-service="${s.id}">${versions.map((v) => `<option ${v === s.version ? "selected" : ""}>${esc(v)}</option>`).join("")}</select></span>`
      : s.installed ? `<span class="pill">${esc(s.version)}</span>`
      : installing && installing.kind === kind ? `<span data-dash="${kind}">${progressMarkup(installing)}</span>`
      : `<button class="btn small" data-install="${kind}" data-latest="1">${t("btn.install")}</button>`;
    const ports = s.ports.map((p) => `<span class="pill">:${p}</span>`).join("");
    const dis = act ? "disabled" : "";
    const action = s.running
      ? `<button class="icon-btn" data-action="${s.id === "web" ? "reload" : "restart"}" data-id="${s.id}" title="${s.id === "web" ? t("btn.reload") : t("btn.restart")}" ${dis}>${ICONS.refresh}</button>
         <button class="icon-btn danger" data-action="stop" data-id="${s.id}" title="${t("btn.stop")}" ${dis}>${ICONS.stop}</button>`
      : `<button class="icon-btn go" data-action="start" data-id="${s.id}" title="${t("btn.start")}" ${s.installed && !act ? "" : "disabled"}>${ICONS.play}</button>`;
    return `<div class="row ${cls}">
      <div class="ico">${ICONS[KIND_META[kind].icon]}</div>
      <div><div class="name"><button class="linkish" data-svc="${kind}">${esc(KIND_META[kind].label)}</button> ${version}</div><div class="meta">${stateBadge(s, act)}${ports}${s.message && s.installed ? `<span>${esc(s.message)}</span>` : ""}</div></div>
      <div class="act"><input type="checkbox" class="switch" data-sw="${kind}" ${s.enabled && s.installed ? "checked" : ""} ${s.installed ? "" : "disabled"} title="${s.installed ? t("svc.enable") : t("pkg.notInstalled")}">${action}</div>
    </div>`;
  }).join("");
}

async function serviceAction(id, action) {
  if (busy[id] || busyAll) return;
  busy[id] = action; renderAll();
  let results = null;
  try { results = await invoke("service_action", { id, action }); } catch (e) { fail(e); }
  delete busy[id];
  await refresh(true);
  if (results) await reportResults(results, action);
}
async function runAll(action) {
  if (busyAll) return;
  busyAll = action; renderAll();
  let results = null;
  try { results = await invoke(action === "stop" ? "stop_all" : "start_all"); } catch (e) { fail(e); }
  busyAll = null;
  await refresh(true);
  if (results) await reportResults(results, action);
}
async function reportResults(results, action) {
  const bad = results.filter((r) => !r.ok);
  const verb = action === "stop" ? "stopped" : action === "reload" ? "reloaded" : action === "restart" ? "restarted" : "started";
  if (!bad.length) {
    if (!results.length) { toast(t("msg.nothingToDo"), "info"); return; }
    toast(results.length > 1 ? t(`msg.all.${verb}`) : t(`msg.one.${verb}`, { name: serviceName(results[0].id) }), "ok");
    return;
  }
  bad.forEach((r) => logActivity(`${serviceName(r.id)}: ${r.error}`, "bad"));
  const perm = bad.find((r) => r.port_permission);
  const rest = bad.filter((r) => r !== perm);
  if (rest.length) {
    const allMissing = rest.every((r) => /not installed/i.test(r.error));
    const head = t(action === "stop" ? "msg.stopFailed" : "msg.startFailed");
    toast(allMissing ? t("msg.installFirst") : `${head}:\n${rest.map((r) => `${serviceName(r.id)}: ${r.error.split("\n")[0]}`).join("\n")}`, "bad");
  }
  if (perm) {
    $("#confirm-title").textContent = t("ports.title", { port: perm.port_permission });
    const ok = await askConfirm(t("ports.body"), t("btn.allow"));
    $("#confirm-title").textContent = t("confirm.title");
    if (!ok) return;
    try { await invoke("grant_web_ports"); toast(t("msg.portsGranted"), "ok"); } catch (e) { fail(e); return; }
    await serviceAction(perm.id, "start");
  }
}

/* ---------------- service / tool page ---------------- */
function openService(kind) {
  if (!KIND_META[kind]) return;
  currentKind = kind;
  if (!KIND_META[kind].svc) svcTab = "versions";
  renderService();
  showTab("service");
  if (!upstream && !upstreamLoading) loadUpstream();
}
function versionsFor(kind) {
  const ups = upstream ? upstream.filter((p) => p.kind === kind) : [];
  const built = builtin ? builtin.filter((p) => p.kind === kind) : [];
  const inst = installedVersions(kind);
  const all = [...new Set([...ups.map((p) => p.version), ...built.map((p) => p.version), ...inst])].sort(cmpVer).reverse();
  const source = ups[0]?.source || built[0]?.source || "";
  return { all, inst, source, newest: all[0] || "" };
}
function renderService() {
  const box = $("#service-page");
  if (!currentKind || !state) { box.innerHTML = ""; return; }
  const kind = currentKind, m = KIND_META[kind];
  const st = m.svc && isActive(kind) ? svcStatus(m.svc) : null;
  const act = st ? busyOf(m.svc) : null;
  const on = kindOn(kind);
  const { all, inst, source, newest } = versionsFor(kind);
  const headCls = act ? "busy" : st?.running ? "on" : "";

  // header actions: enable switch + start/stop for services, Open for web tools
  let actions = "";
  if (m.svc) {
    actions += `<label class="sw-label"><span>${t("svc.enable")}</span><input type="checkbox" class="switch" data-sw="${kind}" ${on ? "checked" : ""}></label>`;
    if (st && st.installed) {
      const dis = act ? "disabled" : "";
      actions += st.running
        ? `<button class="btn" data-action="${m.svc === "web" ? "reload" : "restart"}" data-id="${m.svc}" ${dis}>${ICONS.refresh}<span>${m.svc === "web" ? t("btn.reload") : t("btn.restart")}</span></button>
           <button class="btn danger" data-action="stop" data-id="${m.svc}" ${dis}>${ICONS.stop}<span>${t("btn.stop")}</span></button>`
        : `<button class="btn primary" data-action="start" data-id="${m.svc}" ${dis || !on ? "disabled" : ""}>${ICONS.play}<span>${t("btn.start")}</span></button>`;
    }
  }
  if (m.open) actions += `<button class="btn" data-open="${m.open}" ${inst.length ? "" : "disabled"}>${ICONS.open}<span>${t("btn.open")}</span></button>`;

  const tabs = m.svc
    ? `<div class="tabs"><button class="tab-btn ${svcTab === "service" ? "active" : ""}" data-svctab="service">${t("svc.tab.service")}</button><button class="tab-btn ${svcTab === "versions" ? "active" : ""}" data-svctab="versions">${t("svc.tab.versions")} <span class="count">${all.length}</span></button></div>`
    : "";

  let body = "";
  if (m.svc && svcTab === "service") {
    let notice = "";
    if (!isActive(kind)) notice = `<div class="notice">${ICONS.info}<span>${t("svc.offAlt", { name: m.label, role: t(SVC_ROLE[m.svc] || "role.service") })}</span></div>`;
    else if (!on) notice = `<div class="notice">${ICONS.info}<span>${t("svc.offHint")}</span></div>`;
    else if (st && !st.installed) notice = `<div class="notice">${ICONS.info}<span>${t("svc.installHint")}</span></div>`;
    const ver = st
      ? inst.length > 1
        ? `<select class="small" data-version-kind="${kind}" data-service="${m.svc}">${inst.map((v) => `<option ${v === st.version ? "selected" : ""}>${esc(v)}</option>`).join("")}</select>`
        : st.installed ? `<code>${esc(st.version)}</code>` : `<span class="state warn"><i></i>${t("pkg.notInstalled")}</span>`
      : `<code>${esc(inst[0] || "—")}</code>`;
    const ports = st ? st.ports.map((p) => `<span class="pill">:${p}</span>`).join("") : "—";
    const pids = st && st.pids.length ? `<code>${st.pids.join(", ")}</code>` : "—";
    const quick = [];
    if (m.svc === "web") quick.push(`<button class="btn small" data-open="web">${ICONS.open}<span>${t("btn.openWeb")}</span></button>`);
    if (m.svc === "mail") quick.push(`<button class="btn small" data-open="mail">${ICONS.open}<span>${t("btn.openMail")}</span></button>`);
    if (m.svc === "db") { quick.push(`<button class="btn small" data-open="adminer">${ICONS.open}<span>Adminer</span></button>`); quick.push(`<button class="btn small" data-tab="database">${ICONS.db}<span>${t("nav.database")}</span></button>`); }
    if (m.svc === "php") quick.push(`<button class="btn small" data-open="phpini">${ICONS.edit}<span>php.ini</span></button>`);
    quick.push(`<button class="btn small" data-open="config">${ICONS.file}<span>${t("btn.configFile")}</span></button>`);
    quick.push(`<button class="btn small" data-open="logs">${ICONS.folder}<span>${t("btn.logsFolder")}</span></button>`);
    quick.push(`<button class="btn small" data-tab="logs">${ICONS.file}<span>${t("nav.logs")}</span></button>`);
    body = `${notice}<div class="svc-cards">
      <section class="card"><h2>${t("svc.tab.service")}</h2>
        <div class="kv"><span>${t("svc.state")}</span>${stateBadge(st, act)}</div>
        <div class="kv"><span>${t("svc.versionInUse")}</span>${ver}</div>
        <div class="kv"><span>${t("svc.ports")}</span>${ports}</div>
        <div class="kv"><span>${t("svc.pids")}</span>${pids}</div>
        ${st?.message && st.installed ? `<div class="kv"><span></span><span class="sub" style="margin:0">${esc(st.message)}</span></div>` : ""}
      </section>
      <section class="card"><h2>${t("svc.files")}</h2><div class="btn-row">${quick.join("")}</div></section>
    </div>`;
  } else {
    const rows = all.map((v) => {
      const isInst = inst.includes(v);
      const inUse = !!st && st.installed && st.version === v;
      const isInstalling = installing && installing.kind === kind && installing.version === v;
      const isRemoving = removing && removing.kind === kind && removing.version === v;
      const status = inUse ? `<span class="state on"><i></i>${t("ver.inUse")}</span>` : isInst ? `<span class="pill ok">${t("ver.installed")}</span>` : v === newest ? `<span class="pill accent">${t("ver.newest")}</span>` : "";
      const action = isInstalling ? progressMarkup(installing)
        : isRemoving ? `<span class="state busy">${ICONS.spinner}${t("pkg.removing")}</span>`
        : isInst ? `<button class="btn small subtle" data-remove="${kind}" data-version="${esc(v)}" ${installing || removing ? "disabled" : ""}>${ICONS.trash}<span>${t("btn.remove")}</span></button>`
        : `<button class="btn small primary" data-install="${kind}" data-version="${esc(v)}" ${installing || removing ? "disabled" : ""}>${t("btn.install")}</button>`;
      return `<div class="vrow ${inUse ? "inuse" : ""} ${isInstalling ? "installing" : ""} ${isRemoving ? "removing" : ""}" data-row="${kind}:${esc(v)}">
        <span class="v">${esc(v)}</span><span class="src">${esc(source)}</span><span>${status}</span><span class="act">${action}</span></div>`;
    }).join("");
    body = `<div class="vhead"><span>${upstreamLoading ? t("btn.checking") : t("ver.hint", { source: source || "—" })}</span>
        <button class="btn small ${upstreamLoading ? "busy" : ""}" id="btn-check-upstream" ${upstreamLoading ? "disabled" : ""}>${ICONS.refresh}<span>${t("btn.refresh")}</span></button></div>
      <div class="vtable"><div class="vrow head"><span>${t("ver.version")}</span><span>${t("ver.source")}</span><span>${t("ver.status")}</span><span class="act">${t("ver.action")}</span></div>
      ${rows || `<div class="vrow"><span class="sub" style="margin:0">${upstreamLoading ? t("btn.checking") : t("ver.empty")}</span></div>`}</div>`;
  }

  box.innerHTML = `<header class="top svc-head ${headCls}">
      <div class="ico">${ICONS[m.icon]}</div>
      <div class="titles"><h1>${esc(m.label)}</h1><div class="meta">${m.svc ? stateBadge(st, act) : inst.length ? `<span class="pill ok">${t("ver.installed")} ${esc(inst[0])}</span>` : `<span class="state"><i></i>${t("pkg.notInstalled")}</span>`}${source ? `<span class="pill">${esc(source)}</span>` : ""}</div></div>
      <div class="spacer"></div>
      <div class="acts">${actions}</div>
    </header>${tabs}${body}`;
}
async function loadUpstream() {
  upstreamLoading = true; renderService();
  try { upstream = await invoke("upstream_versions"); } catch (e) { fail(e); }
  upstreamLoading = false;
  renderService();
}

/* ---------------- install / remove ---------------- */
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
  const cell = $(`[data-row="${installing.kind}:${installing.version}"] .act`);
  if (cell) cell.innerHTML = html;
  const dash = $(`[data-dash="${installing.kind}"]`);
  if (dash) dash.innerHTML = html;
}
async function installPackage(kind, latest, version = null) {
  if (installing) { toast(t("pkg.installing")); return; }
  const v = version || versionsFor(kind).newest || "";
  installing = { kind, version: v, downloaded: 0, total: null, phase: "download" };
  renderAll(); paintProgress();
  try {
    const i = await invoke("install_package", { kind, version: version || null, latest: !!latest || !version });
    toast(t("msg.installed", { name: KIND_META[i.kind]?.label || i.kind, version: i.version }), "ok");
  } catch (e) { fail(e); }
  installing = null;
  await refresh(true);
}
async function removePackage(kind, version) {
  const name = KIND_META[kind]?.label || kind;
  if (!(await askConfirm(t("msg.confirmRemove", { name, version })))) return;
  removing = { kind, version }; renderAll();
  try { await invoke("remove_package", { kind, version }); toast(t("msg.removed", { name, version }), "ok"); } catch (e) { fail(e); }
  removing = null;
  await refresh(true);
}
listen("install-progress", (ev) => {
  const p = ev.payload;
  if (!installing || installing.kind !== p.kind) return;
  installing.downloaded = p.downloaded; installing.total = p.total ?? installing.total;
  installing.phase = installing.total && installing.downloaded >= installing.total ? "extract" : "download";
  paintProgress();
});
listen("services-changed", () => refresh());
listen("backup-done", (ev) => toast(t("msg.backup", { path: ev.payload }), "ok"));

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

/* ---------------- sites ---------------- */
function renderSites() {
  const root = state.config.general.document_root || `${state.root}/www`;
  $("#sites-hint").textContent = t("sites.hint", { root, tld: state.config.general.tld });
  $("#sites").innerHTML = state.sites.map((s) => `<div class="row on">
    <div class="ico">${ICONS.site}</div>
    <div><div class="name">${esc(s.hostname)}${s.auto ? "" : ' <span class="pill">manual</span>'}</div><div class="meta"><span class="mono">${esc(s.root)}</span></div></div>
    <div class="act">
      <button class="icon-btn" data-open="site:${esc(s.hostname)}" title="${t("btn.open")}">${ICONS.open}</button>
      <button class="icon-btn" data-open="path:${esc(s.path)}" title="${t("quick.root")}">${ICONS.folder}</button>
      <button class="icon-btn" data-open="edit:${esc(s.path)}" title="${t("settings.editor")}">${ICONS.edit}</button>
      ${s.auto ? `<button class="icon-btn danger" data-delete-project="${esc(s.name)}" title="${t("btn.remove")}">${ICONS.trash}</button>` : `<button class="icon-btn danger" data-remove-site="${esc(s.hostname)}" title="${t("btn.remove")}">${ICONS.x}</button>`}
    </div></div>`).join("") || `<div class="empty">${ICONS.site}<span>${esc(root)}</span></div>`;
}
async function syncSites() {
  try { const r = await invoke("sync_sites"); if (r.missing_hosts.length) await invoke("hosts_sync"); toast(t("msg.synced"), "ok"); } catch (e) { fail(e); }
  refresh();
}

/* ---------------- migrate from another stack ---------------- */
let migrateSources = [];
async function openMigrate() {
  const dlg = $("#dlg-migrate"), f = $("#form-migrate");
  try { migrateSources = await invoke("migrate_detect"); } catch (e) { fail(e); return; }
  const sel = $("#migrate-platform");
  sel.innerHTML = migrateSources.map((s) => `<option value="${s.platform}">${esc(s.label)}${s.found ? " ✓" : ""}</option>`).join("");
  const first = migrateSources.find((s) => s.found) || migrateSources[0];
  sel.value = first.platform;
  $("#migrate-report").classList.add("hidden"); $("#migrate-report").innerHTML = "";
  $("#migrate-progress").classList.add("hidden");
  $("#btn-migrate-run").disabled = false;
  f.elements.databases.checked = false; $("#migrate-db").classList.add("hidden");
  applyMigrateSource();
  dlg.showModal();
}
function applyMigrateSource() {
  const f = $("#form-migrate");
  const s = migrateSources.find((x) => x.platform === f.elements.platform.value);
  if (!s) return;
  f.elements.path.value = s.path || "";
  f.elements.path.placeholder = s.platform === "flyenv" ? "…/FlyEnv/server/host.json" : "/path/to/www";
  $("#migrate-detected").textContent = s.found ? t("migrate.found", { path: s.path }) : t("migrate.notFound");
  f.elements.db_port.value = s.db_port; f.elements.db_user.value = s.db_user; f.elements.db_password.value = s.db_password;
}
$("#btn-migrate").onclick = openMigrate;
$("#migrate-platform").onchange = applyMigrateSource;
$("#btn-migrate-cancel").onclick = () => $("#dlg-migrate").close();
$("#form-migrate").elements.databases.onchange = (ev) => $("#migrate-db").classList.toggle("hidden", !ev.target.checked);
$("#btn-migrate-browse").onclick = async () => {
  const f = $("#form-migrate");
  const isFile = f.elements.platform.value === "flyenv";
  try {
    const picked = await window.__TAURI__.dialog.open({ directory: !isFile, multiple: false, title: t("migrate.folder") });
    if (picked) f.elements.path.value = typeof picked === "string" ? picked : picked.path || "";
  } catch (e) { toast(t("migrate.noPicker"), "info"); }
};
listen("migrate-progress", (ev) => { $("#migrate-progress .txt").textContent = ev.payload; });
$("#form-migrate").onsubmit = async (ev) => {
  ev.preventDefault(); const f = ev.target;
  const req = {
    platform: f.elements.platform.value, path: f.elements.path.value.trim(), mode: f.elements.mode.value,
    databases: f.elements.databases.checked, db_host: f.elements.db_host.value.trim(), db_port: Number(f.elements.db_port.value) || 3306,
    db_user: f.elements.db_user.value.trim(), db_password: f.elements.db_password.value,
  };
  const btn = $("#btn-migrate-run"); btn.disabled = true;
  const prog = $("#migrate-progress"); prog.classList.remove("hidden"); $(".txt", prog).textContent = t("migrate.working");
  $("#migrate-report").classList.add("hidden");
  try {
    const r = await invoke("migrate_run", { req });
    const line = (cls, label, items) => items.length ? `<div class="rep ${cls}"><b>${label} (${items.length})</b><ul>${items.map((x) => `<li>${esc(x)}</li>`).join("")}</ul></div>` : "";
    $("#migrate-report").innerHTML = line("ok", t("migrate.sites"), r.sites) + line("ok", t("migrate.databases"), r.databases) + line("", t("migrate.skipped"), r.skipped) + line("bad", t("migrate.errors"), r.errors)
      || `<div class="rep">${t("migrate.nothing")}</div>`;
    $("#migrate-report").classList.remove("hidden");
    if (r.sites.length) { try { const s = await invoke("sync_sites"); if (s.missing_hosts.length) await invoke("hosts_sync"); } catch (e) { fail(e); } }
    toast(t("migrate.done", { sites: r.sites.length, dbs: r.databases.length }), r.errors.length ? "info" : "ok");
  } catch (e) { fail(e); }
  prog.classList.add("hidden"); btn.disabled = false;
  refresh(true);
};

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
  $$("[data-os]").forEach((el) => el.classList.toggle("hidden", !state.platform.startsWith(el.dataset.os)));
  for (const el of $("#form-settings").elements) {
    if (!el.name) continue;
    const [sec, key] = el.name.split(".");
    const v = state.config[sec]?.[key];
    if (el.type === "checkbox") el.checked = !!v; else el.value = v ?? "";
  }
}
function readSettings() {
  const cfg = clone(state.config);
  for (const el of $("#form-settings").elements) {
    if (!el.name) continue;
    const [sec, key] = el.name.split(".");
    cfg[sec][key] = el.type === "checkbox" ? el.checked : el.type === "number" ? Number(el.value) : el.value;
  }
  return cfg;
}

/* ---------------- events ---------------- */
document.addEventListener("click", async (ev) => {
  const item = ev.target.closest("[data-svc]");
  if (item && !ev.target.closest("input")) { openService(item.dataset.svc); return; }
  const b = ev.target.closest("button");
  if (!b) return;
  if (b.dataset.svctab) { svcTab = b.dataset.svctab; renderService(); return; }
  if (b.dataset.tab) { showTab(b.dataset.tab); return; }
  if (b.id === "btn-check-upstream") { loadUpstream(); return; }
  if (b.dataset.open) { try { await invoke("open_target", { what: b.dataset.open }); } catch (e) { fail(e); } return; }
  if (b.dataset.copy) {
    let text = $(b.dataset.copy).textContent;
    if (b.dataset.copySecret) text = (await invoke("db_password")) || "";
    if (text) { await navigator.clipboard.writeText(text); toast(t("msg.copied"), "ok"); }
    return;
  }
  if (b.dataset.action) { b.disabled = true; await serviceAction(b.dataset.id, b.dataset.action); return; }
  if (b.dataset.install) { await installPackage(b.dataset.install, !!b.dataset.latest, b.dataset.version || null); return; }
  if (b.dataset.remove) { await removePackage(b.dataset.remove, b.dataset.version); return; }
  if (b.dataset.deleteProject) {
    if (!(await askConfirm(t("msg.confirmDelete", { name: b.dataset.deleteProject })))) return;
    try { await invoke("delete_project", { name: b.dataset.deleteProject }); await syncSites(); } catch (e) { fail(e); }
    return;
  }
  if (b.dataset.removeSite) { try { await invoke("remove_site", { hostname: b.dataset.removeSite }); await syncSites(); } catch (e) { fail(e); } }
});
document.addEventListener("keydown", (ev) => {
  const item = ev.target.closest?.("[data-svc]");
  if (item && (ev.key === "Enter" || ev.key === " ")) { ev.preventDefault(); openService(item.dataset.svc); }
});
document.addEventListener("change", async (ev) => {
  const el = ev.target;
  if (el.dataset.sw) { await toggleKind(el.dataset.sw, el.checked); return; }
  if (el.dataset.versionKind) {
    const cfg = clone(state.config);
    cfg[SVC_SECTION[el.dataset.service]].version = el.value;
    try { await invoke("save_config", { config: cfg }); toast(t("msg.saved"), "ok"); } catch (e) { fail(e); }
    refresh(true);
  }
});

$("#btn-start-all").onclick = () => runAll("start");
$("#btn-stop-all").onclick = () => runAll("stop");
$("#btn-hosts-sync").onclick = async () => { try { await invoke("hosts_sync"); toast(t("msg.hostsUpdated"), "ok"); } catch (e) { fail(e); } refresh(); };
$("#btn-sync-sites").onclick = syncSites;
$("#btn-db-show").onclick = async () => {
  const el = $("#db-password");
  if (!state.db_password_known) return;
  if (el.classList.contains("secret")) { el.textContent = (await invoke("db_password")) || ""; el.classList.remove("secret"); }
  else { el.textContent = "••••••••••••"; el.classList.add("secret"); }
};
$("#btn-db-rotate").onclick = async () => { try { const pw = await invoke("db_set_password", { new: null }); $("#db-password").textContent = pw; $("#db-password").classList.remove("secret"); toast(t("msg.pwRotated"), "ok"); } catch (e) { fail(e); } };
$("#btn-db-backup").onclick = async () => { try { const p = await invoke("db_backup"); toast(t("msg.backup", { path: p }), "ok"); } catch (e) { fail(e); } };
$("#form-db").onsubmit = async (ev) => {
  ev.preventDefault(); const name = ev.target.elements.name.value.trim();
  try { await invoke("db_create", { name }); toast(t("msg.created", { name }), "ok"); ev.target.reset(); const l = await invoke("db_list"); $("#db-list").innerHTML = l.map((d) => `<span class="chip">${esc(d)}</span>`).join(""); } catch (e) { fail(e); }
};
$("#form-site").onsubmit = async (ev) => {
  ev.preventDefault(); const f = ev.target;
  try { await invoke("add_site", { hostname: f.elements.hostname.value.trim(), path: f.elements.path.value.trim(), public: f.elements.public.value.trim() }); f.reset(); await syncSites(); } catch (e) { fail(e); }
};
$("#form-settings").onsubmit = async (ev) => { ev.preventDefault(); try { await invoke("save_config", { config: readSettings() }); toast(t("msg.saved"), "ok"); } catch (e) { fail(e); } refresh(true); };
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

/* ---------------- new project dialog ---------------- */
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

/* ---------------- boot ---------------- */
(async () => {
  await refresh();
  if (state?.ui_theme === "light" || state?.ui_theme === "dark") setTheme(state.ui_theme);
  if (state?.ui_tab) {
    const [tab, sub] = state.ui_tab.split(":");   // e.g. "php:versions" (screenshot helper)
    if (KIND_META[tab]) { if (sub) svcTab = sub; openService(tab); } else { showTab(tab); if (sub === "migrate") openMigrate(); }
  }
  try { builtin = await invoke("builtin_versions"); renderService(); } catch (e) { fail(e); }
  setInterval(() => { if (document.visibilityState !== "visible" || installing || removing) return; refresh(); if (currentTab === "logs") loadLog(); }, 15000);
})();
