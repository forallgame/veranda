/* Veranda UI - vanilla JS, no build step. Talks to the Rust side through Tauri IPC. */
// Apply the saved theme as early as possible (no inline script: keeps the CSP strict).
try { const saved = localStorage.getItem("veranda.theme"); if (saved === "light" || saved === "dark") document.documentElement.dataset.theme = saved; } catch (e) {}
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const ICONS = window.VERANDA_ICONS;

// Every downloadable kind. `svc` links a kind to the runtime service it provides (web/php/db/cache/mail);
// tools have no service. The sidebar and the service pages are generated from this table.
const KIND_META = {
  caddy:      { label: "Caddy",          svc: "web",   group: "web",   icon: "web" },
  nginx:      { label: "Nginx",          svc: "web",   group: "web",   icon: "web" },
  apache:     { label: "Apache",         svc: "web",   group: "web",   icon: "web" },
  php:        { label: "PHP",            svc: "php",   group: "lang",  icon: "php" },
  python:     { label: "Python",         group: "lang", icon: "lang" },
  go:         { label: "Go",             group: "lang", icon: "lang" },
  bun:        { label: "Bun",            group: "lang", icon: "lang" },
  java:       { label: "Java",           group: "lang", icon: "lang" },
  deno:       { label: "Deno",           group: "lang", icon: "lang" },
  zig:        { label: "Zig",            group: "lang", icon: "lang" },
  dotnet:     { label: ".NET SDK",       group: "lang", icon: "lang" },
  mariadb:    { label: "MariaDB",        svc: "db",    group: "db",    icon: "db" },
  mysql:      { label: "MySQL",          svc: "db",    group: "db",    icon: "db" },
  postgres:   { label: "PostgreSQL",     svc: "pg",    group: "db",    icon: "db" },
  mongodb:    { label: "MongoDB",        svc: "mongodb", group: "db",  icon: "db" },
  meilisearch:{ label: "Meilisearch",    svc: "meilisearch", group: "search", icon: "search", open: "meili" },
  qdrant:     { label: "Qdrant",         svc: "qdrant", group: "search", icon: "vector", open: "qdrant" },
  cache:      { label: "Valkey",         svc: "cache", group: "more",  icon: "cache" },   // "Redis" on Windows (set at boot)
  mailpit:    { label: "Mailpit",        svc: "mail",  group: "more",  icon: "mail" },
  composer:   { label: "Composer",       group: "tools", icon: "pkg" },
  node:       { label: "Node.js",        group: "tools", icon: "node" },
  adminer:    { label: "Adminer",        group: "tools", icon: "tool", open: "adminer" },
  phpmyadmin: { label: "phpMyAdmin",     group: "tools", icon: "tool", open: "phpmyadmin" },
  cabundle:   { label: "CA bundle",      group: "tools", icon: "shield" },
};
const GROUPS = [["web", "group.web"], ["lang", "group.lang"], ["db", "group.db"], ["search", "group.search"], ["more", "group.more"], ["tools", "group.tools"]];
const SVC_SECTION = { web: "web", php: "php", db: "db", pg: "pg", cache: "cache", mail: "mail", mongodb: "mongodb", meilisearch: "meilisearch", qdrant: "qdrant" };
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
const TOAST_ICON = { ok: ICONS.toastOk, bad: ICONS.toastBad, info: ICONS.toastInfo };
// Static markup (index.html) declares icons as <i class="ic" data-icon="name"></i>; fill them once.
const applyIcons = () => { $$("i.ic[data-icon]").forEach((el) => { if (!el.firstChild) el.innerHTML = ICONS[el.dataset.icon] || ""; }); };
applyIcons();
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
const externalVersions = (kind) => (state.external || []).filter((x) => x.kind === kind);
const svcStatus = (svc) => state.services.find((s) => s.id === svc);
function activeKind(svc) {
  return { web: state.config.web.server, db: state.config.db.engine, pg: "postgres", php: "php", cache: "cache", mail: "mailpit", mongodb: "mongodb", meilisearch: "meilisearch", qdrant: "qdrant" }[svc];
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
      const dot = act ? "busy" : st?.running ? "run" : installedVersions(kind).length || externalVersions(kind).length ? "inst" : "";
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
  if (!on && svcStatus(svc)?.running) {
    busy[svc] = "stop"; renderAll();
    try { await invoke("service_action", { id: svc, action: "stop" }); } catch (e) { fail(e); }
    delete busy[svc];
  }
  try { await invoke("save_config", { config: cfg }); } catch (e) { fail(e); }
  await refresh(true);
  if (on && svcStatus(svc)?.installed && !svcStatus(svc)?.running) await serviceAction(svc, "start");
}

/* ---------------- overview ---------------- */
function renderServices() {
  const visible = state.services.filter((s) => !isHidden(s.id) && (s.enabled || s.running));
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
  const conflict = bad.find((r) => r.port_in_use && r !== perm);
  const rest = bad.filter((r) => r !== perm && r !== conflict);
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
  if (conflict) await offerPortMove(conflict);
}
// Every port a service listens on, as [section, key] in the config.
const PORT_FIELDS = { web: [["web", "http_port"], ["web", "https_port"]], db: [["db", "port"]], pg: [["pg", "port"]], php: [["php", "fcgi_port"]], cache: [["cache", "port"]], mail: [["mail", "smtp_port"], ["mail", "http_port"]], mongodb: [["mongodb", "port"]], meilisearch: [["meilisearch", "port"]], qdrant: [["qdrant", "port"]] };
async function offerPortMove(r) {
  const c = r.port_in_use, name = serviceName(r.id);
  const DEFAULT_PORT = { mongodb: 27017, meilisearch: 7700, qdrant: 6333 };
  const field = (PORT_FIELDS[r.id] || []).find(([sec, key]) => (state.config[sec][key] || DEFAULT_PORT[sec] || 0) === c.port);
  if (!field) { toast(`${name}: ${r.error.split("\n")[0]}`, "bad"); return; }
  $("#confirm-title").textContent = t("ports.busyTitle", { port: c.port });
  const ok = await askConfirm(t("ports.busyBody", { owner: c.owner || "?", name, new: c.suggestion }), t("btn.usePort", { new: c.suggestion }));
  $("#confirm-title").textContent = t("confirm.title");
  if (!ok) return;
  const cfg = clone(state.config);
  cfg[field[0]][field[1]] = c.suggestion;
  try { await invoke("save_config", { config: cfg }); toast(t("msg.portMoved", { name, new: c.suggestion }), "ok"); } catch (e) { fail(e); return; }
  await refresh(true);
  await serviceAction(r.id, "start");
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
  const ext = externalVersions(kind);
  const all = [...new Set([...ups.map((p) => p.version), ...built.map((p) => p.version), ...inst, ...ext.map((x) => x.version)])].sort(cmpVer).reverse();
  const source = ups[0]?.source || built[0]?.source || "";
  const newest = [...new Set([...ups.map((p) => p.version), ...built.map((p) => p.version)])].sort(cmpVer).reverse()[0] || all[0] || "";
  return { all, inst, ext, source, newest };
}
function renderService() {
  const box = $("#service-page");
  if (!currentKind || !state) { box.innerHTML = ""; return; }
  const kind = currentKind, m = KIND_META[kind];
  const st = m.svc && isActive(kind) ? svcStatus(m.svc) : null;
  const act = st ? busyOf(m.svc) : null;
  const on = kindOn(kind);
  const { all, inst, ext, source, newest } = versionsFor(kind);
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
    if (m.svc === "pg") quick.push(`<button class="btn small" data-open="adminer-pg">${ICONS.open}<span>Adminer (PostgreSQL)</span></button>`);
    if (m.open && m.svc) quick.unshift(`<button class="btn small" data-open="${m.open}" ${st?.running ? "" : "disabled"}>${ICONS.open}<span>${t("btn.openUi")}</span></button>`);
    if (m.svc === "cache") notice += `<div class="notice">${ICONS.info}<span>${t(state.platform.startsWith("windows") ? "cache.redisNote" : "cache.valkeyNote")}</span></div>`;
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
        ${m.svc === "pg" ? `<div class="kv"><span>${t("db.user")}</span><code>postgres</code></div><div class="kv"><span>${t("db.password")}</span><code id="pg-password" class="secret">••••••••••••</code><button type="button" class="icon-btn small" id="btn-pg-show" title="${t("db.password")}">${ICONS.open}</button></div>` : ""}
        ${st?.message && st.installed ? `<div class="kv"><span></span><span class="sub" style="margin:0">${esc(st.message)}</span></div>` : ""}
      </section>
      <section class="card"><h2>${t("svc.files")}</h2><div class="btn-row">${quick.join("")}</div></section>
    </div>`;
  } else {
    const rows = all.map((v) => {
      const isInst = inst.includes(v);
      const x = ext.find((e) => e.version === v);
      if (x && !isInst) {
        const isRemovingExt = removing && removing.kind === kind && removing.version === v;
        const removeBtn = x.source === "nvm"
          ? isRemovingExt ? `<span class="state busy">${ICONS.spinner}${t("pkg.removing")}</span>`
            : `<button class="btn small subtle" data-remove-ext="${kind}" data-version="${esc(v)}" data-source="${esc(x.source)}" ${installing || removing ? "disabled" : ""}>${ICONS.trash}<span>${t("btn.removeNvm")}</span></button>`
          : "";
        return `<div class="vrow ${isRemovingExt ? "removing" : ""}" data-row="${kind}:${esc(v)}"><span class="v">${esc(v)}</span><span class="src">${esc(x.bin_dir)}</span><span><span class="pill ok">${esc(x.source)}</span></span>
          <span class="act"><button class="icon-btn small" data-open="path:${esc(x.bin_dir)}" title="${esc(x.bin_dir)}">${ICONS.folder}</button>${removeBtn}</span></div>`;
      }
      const inUse = !!st && st.installed && st.version === v;
      const isInstalling = installing && installing.kind === kind && installing.version === v;
      const isRemoving = removing && removing.kind === kind && removing.version === v;
      const status = inUse ? `<span class="state on"><i></i>${t("ver.inUse")}</span>` : isInst ? `<span class="pill ok">${t("ver.installed")}</span>` : v === newest ? `<span class="pill accent">${t("ver.newest")}</span>` : "";
      const action = isInstalling ? progressMarkup(installing)
        : isRemoving ? `<span class="state busy">${ICONS.spinner}${t("pkg.removing")}</span>`
        : isInst ? `<button class="btn small subtle" data-remove="${kind}" data-version="${esc(v)}" ${installing || removing ? "disabled" : ""}>${ICONS.trash}<span>${t("btn.remove")}</span></button>`
        : `<button class="btn small primary" data-install="${kind}" data-version="${esc(v)}" ${installing || removing ? "disabled" : ""}>${kind === "node" && state.nvm_available ? t("btn.installNvm") : t("btn.install")}</button>`;
      return `<div class="vrow ${inUse ? "inuse" : ""} ${isInstalling ? "installing" : ""} ${isRemoving ? "removing" : ""} ${upstreamFresh && !isInst ? "fresh" : ""}" data-row="${kind}:${esc(v)}">
        <span class="v">${esc(v)}</span><span class="src">${esc(source)}</span><span>${status}</span><span class="act">${action}</span></div>`;
    }).join("");
    const skeleton = upstreamLoading
      ? [0, 1, 2].map(() => `<div class="vrow skeleton"><span class="sk w1"></span><span class="sk w2"></span><span class="sk w3"></span><span class="act"><span class="sk w4"></span></span></div>`).join("")
      : "";
    body = `${kind === "node" && state.nvm_available ? `<div class="notice">${ICONS.info}<span>${t("ver.nvmInstall")}</span></div>` : ""}${ext.length ? `<div class="notice">${ICONS.info}<span>${t("ver.external", { n: ext.length, source: [...new Set(ext.map((e) => e.source))].join(", ") })}</span></div>` : ""}
      <div class="vhead ${upstreamLoading ? "loading" : ""}"><span>${upstreamLoading ? `${ICONS.spinner}${t("ver.fetching", { source: source || "—" })}` : t("ver.hint", { source: source || "—" })}</span>
        <button class="btn small ${upstreamLoading ? "busy" : ""}" id="btn-check-upstream" ${upstreamLoading ? "disabled" : ""}>${ICONS.refresh}<span>${t("btn.refresh")}</span></button></div>
      <div class="vtable ${upstreamLoading ? "loading" : ""}">${upstreamLoading ? `<div class="loadbar"><div class="fill"></div></div>` : ""}<div class="vrow head"><span>${t("ver.version")}</span><span>${t("ver.source")}</span><span>${t("ver.status")}</span><span class="act">${t("ver.action")}</span></div>
      ${rows}${skeleton}${!rows && !upstreamLoading ? `<div class="vrow"><span class="sub" style="margin:0">${t("ver.empty")}</span></div>` : ""}</div>`;
  }

  box.innerHTML = `<header class="top svc-head ${headCls}">
      <div class="ico">${ICONS[m.icon]}</div>
      <div class="titles"><h1>${esc(m.label)}</h1><div class="meta">${m.svc ? stateBadge(st, act) : inst.length ? `<span class="pill ok">${t("ver.installed")} ${esc(inst[0])}</span>` : ext.length ? `<span class="pill ok">${esc(ext[0].source)} ${esc(ext[0].version)}</span>` : `<span class="state"><i></i>${t("pkg.notInstalled")}</span>`}${source ? `<span class="pill">${esc(source)}</span>` : ""}</div></div>
      <div class="spacer"></div>
      <div class="acts">${actions}</div>
    </header>${tabs}${body}`;
}
let upstreamFresh = false;   // true right after a fetch, so new rows fade in once
async function loadUpstream() {
  upstreamLoading = true; renderService();
  try { upstream = await invoke("upstream_versions"); upstreamFresh = true; } catch (e) { fail(e); }
  upstreamLoading = false;
  renderService();
  upstreamFresh = false;
}

/* ---------------- install / remove ---------------- */
function progressText(p) {
  if (p.note) return p.note.length > 48 ? `…${p.note.slice(-46)}` : p.note;
  if (p.phase === "extract") return t("pkg.extracting");
  const mb = (p.downloaded / 1048576).toFixed(1);
  return p.total ? `${(100 * p.downloaded / p.total).toFixed(0)}% · ${mb}/${(p.total / 1048576).toFixed(1)} MiB` : `${mb} MiB`;
}
function progressMarkup(p) {
  const pct = p.phase === "extract" ? 100 : p.total ? (100 * p.downloaded / p.total) : 0;
  const cancel = p.phase === "download" && !p.note && !p.cancelling
    ? `<button class="icon-btn small" data-cancel-install="1" title="${t("btn.cancel")}">${ICONS.x}</button>` : "";
  return `<div class="row-progress"><div class="bar"><div class="fill ${p.phase === "extract" ? "indeterminate" : ""}" style="width:${pct.toFixed(1)}%"></div></div><span class="txt">${p.cancelling ? t("pkg.cancelling") : progressText(p)}</span>${cancel}</div>`;
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
  } catch (e) {
    if (/download cancelled/i.test(String(e?.message || e))) toast(t("msg.cancelled", { name: KIND_META[kind]?.label || kind }), "info"); else fail(e);
  }
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
  installing.downloaded = p.downloaded; installing.total = p.total ?? installing.total; installing.note = p.note || null;
  installing.phase = p.note ? "extract" : installing.total && installing.downloaded >= installing.total ? "extract" : "download";
  paintProgress();
});
listen("services-changed", () => refresh());
listen("autostart-done", async (ev) => { await refresh(true); await reportResults(ev.payload, "start"); });
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
    const raw = await invoke("read_log", { name: currentLog, lines: 400 });
    const text = raw.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "");   // strip ANSI colour codes some servers print
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
  if (b.id === "btn-pg-show") {
    const el = $("#pg-password");
    if (el.classList.contains("secret")) { try { el.textContent = (await invoke("pg_password")) || t("db.unknownPassword"); el.classList.remove("secret"); } catch (e) { fail(e); } }
    else { el.textContent = "••••••••••••"; el.classList.add("secret"); }
    return;
  }
  if (b.dataset.open) { try { await invoke("open_target", { what: b.dataset.open }); } catch (e) { fail(e); } return; }
  if (b.dataset.copy) {
    let text = $(b.dataset.copy).textContent;
    if (b.dataset.copySecret) text = (await invoke("db_password")) || "";
    if (text) { await navigator.clipboard.writeText(text); toast(t("msg.copied"), "ok"); }
    return;
  }
  if (b.dataset.action) { b.disabled = true; await serviceAction(b.dataset.id, b.dataset.action); return; }
  if (b.dataset.install) { await installPackage(b.dataset.install, !!b.dataset.latest, b.dataset.version || null); return; }
  if (b.dataset.cancelInstall) { if (installing) { installing.cancelling = true; paintProgress(); } try { await invoke("cancel_install"); } catch (e) { fail(e); } return; }
  if (b.dataset.remove) { await removePackage(b.dataset.remove, b.dataset.version); return; }
  if (b.dataset.removeExt) {
    const kind = b.dataset.removeExt, version = b.dataset.version, source = b.dataset.source;
    if (!(await askConfirm(t("msg.confirmRemoveExt", { version, source })))) return;
    removing = { kind, version }; renderAll();
    try { await invoke("remove_external", { kind, version, source }); toast(t("msg.removedExt", { version, source }), "ok"); } catch (e) { fail(e); }
    removing = null; await refresh(true); return;
  }
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

/* ---------------- scroll benchmark (VERANDA_UI_PERF=1, dev only) ---------------- */
async function perfBenchmark() {
  showTab("settings");
  await new Promise((r) => setTimeout(r, 800));
  const doc = document.documentElement;
  const frames = [];
  let last = performance.now(), dir = 1, start = last;
  await new Promise((done) => {
    const step = (now) => {
      frames.push(now - last); last = now;
      if (doc.scrollTop + window.innerHeight >= doc.scrollHeight - 2) dir = -1;
      if (doc.scrollTop <= 0) dir = 1;
      window.scrollBy(0, 8 * dir);
      if (now - start < 4000) requestAnimationFrame(step); else done();
    };
    requestAnimationFrame(step);
  });
  frames.shift();
  const sorted = [...frames].sort((a, b) => a - b);
  const avg = frames.reduce((a, b) => a + b, 0) / frames.length;
  const p95 = sorted[Math.floor(sorted.length * 0.95)];
  const long = frames.filter((f) => f > 20).length;
  const text = `frames=${frames.length} fps=${(1000 / avg).toFixed(1)} avg=${avg.toFixed(2)}ms p95=${p95.toFixed(2)}ms long(>20ms)=${long} max=${sorted[sorted.length - 1].toFixed(1)}ms scrollHeight=${doc.scrollHeight}`;
  invoke("perf_report", { text }).catch(() => {});
}

/* ---------------- scripted checks (VERANDA_UI_AUTOTEST=..., dev only) ---------------- */
async function autotest(name) {
  const report = (text) => invoke("perf_report", { text: `autotest ${name}: ${text}` }).catch(() => {});
  if (name === "cancel-install") {
    // start the biggest download we have (Go), cancel it after a few seconds, report what happened
    openService("go");
    const { newest } = versionsFor("go");
    const started = performance.now();
    const done = installPackage("go", false, newest);
    setTimeout(() => { const b = $("[data-cancel-install]"); if (b) b.click(); else report("no cancel button found"); }, 4000);
    await done;
    const last = activity[0]?.msg || "";
    report(`newest=${newest} elapsed=${((performance.now() - started) / 1000).toFixed(1)}s installed=${installedVersions("go").length} last="${last}"`);
  }
}

/* ---------------- boot ---------------- */
(async () => {
  await refresh();
  if (state?.platform.startsWith("windows")) { KIND_META.cache.label = "Redis"; renderAll(); }
  if (state?.ui_theme === "light" || state?.ui_theme === "dark") setTheme(state.ui_theme);
  if (state?.ui_tab) {
    const [tab, sub] = state.ui_tab.split(":");   // e.g. "php:versions" (screenshot helper)
    if (KIND_META[tab]) { if (sub) svcTab = sub; openService(tab); } else { showTab(tab); if (sub === "migrate") openMigrate(); }
  }
  try { builtin = await invoke("builtin_versions"); renderService(); } catch (e) { fail(e); }
  if (state?.ui_perf) setTimeout(perfBenchmark, 1200);
  if (state?.ui_autotest) setTimeout(() => autotest(state.ui_autotest), 1500);
  setInterval(() => { if (document.visibilityState !== "visible" || installing || removing) return; refresh(); if (currentTab === "logs") loadLog(); }, 15000);
  setInterval(() => invoke("ui_heartbeat").catch(() => {}), 5000);
})();
