/* Veranda Remote - the SSH client: hosts, terminals (xterm.js), SFTP, port forwarding, snippets,
   keychain, known hosts and history. Uses the helpers defined in app.js (invoke, listen, $, t, toast...). */
(function () {
  const R = {
    view: null,            // last `remote_state` result
    terms: new Map(),      // id -> { id, term, fit, search, el, tab, info, hostId, quick, local, closed }
    active: null,          // active terminal id
    sftp: null,            // { id, host_id, label, home }
    panes: { local: { path: "", entries: [], sel: new Set() }, remote: { path: "", entries: [], sel: new Set() } },
    transfers: [],         // { id, name, dir, done, total, state }
    search: "", tag: null,
    promptQueue: [], promptOpen: null,
    pendingTheme: null,
  };
  const settings = () => R.view?.store.settings || {};
  const hostsOf = () => R.view?.store.hosts || [];
  const hostById = (id) => hostsOf().find((h) => h.id === id);
  const newId = () => Array.from(crypto.getRandomValues(new Uint8Array(8)), (b) => b.toString(16).padStart(2, "0")).join("");
  const fmtSize = (n) => n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : n < 1073741824 ? `${(n / 1048576).toFixed(1)} MB` : `${(n / 1073741824).toFixed(2)} GB`;
  const fmtTime = (s) => { if (!s) return "—"; const d = new Date(s * 1000); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`; };
  const osIcon = (os) => /ubuntu|debian|linux|fedora|centos|alpine|arch|rocky|alma/i.test(os) ? "lang" : /darwin|mac/i.test(os) ? "local" : /windows|microsoft/i.test(os) ? "local" : "host";
  const COLORS = ["", "#3b6cf6", "#16a34a", "#d97706", "#dc2626", "#9333ea", "#0891b2", "#db2777", "#64748b"];
  const b64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

  /* ---------------- state ---------------- */
  async function refresh() {
    try { R.view = await invoke("remote_state"); } catch (e) { fail(e); return; }
    renderAll();
  }
  function renderAll() {
    for (const fn of [renderHosts, renderForwards, renderSnippets, renderKeychain, renderHistory, renderSftpHosts, renderCounts]) {
      try { fn(); } catch (e) { fail(`remote ${fn.name}: ${e?.message || e}`); }
    }
  }
  function renderCounts() {
    const open = [...R.terms.values()].filter((x) => !x.closed).length;
    const tc = $("#term-count"); tc.textContent = open; tc.classList.toggle("hidden", !open);
    const fa = (R.view?.forwards_active || []).length;
    const fc = $("#fwd-count"); fc.textContent = fa; fc.classList.toggle("hidden", !fa);
  }

  /* ---------------- hosts ---------------- */
  function hostMatches(h) {
    const q = R.search.trim().toLowerCase();
    if (R.tag && !h.tags.includes(R.tag)) return false;
    if (!q) return true;
    return [h.label, h.address, h.username, h.notes, h.os, ...h.tags].some((x) => (x || "").toLowerCase().includes(q));
  }
  function hostRow(h) {
    const ident = h.auth === "identity" ? (R.view.store.identities.find((i) => i.id === h.identity_id) || {}) : null;
    const key = h.auth === "key" ? (R.view.store.keys.find((k) => k.id === h.key_id) || {}) : null;
    const authPill = h.auth === "agent" ? "agent" : h.auth === "key" ? (key?.label || "key") : h.auth === "identity" ? (ident?.label || "identity") : h.auth === "password" ? (h.has_password ? "password" : "password?") : "";
    const jump = h.jump_host_id ? hostById(h.jump_host_id) : null;
    return `<div class="row host-row" data-host="${esc(h.id)}" tabindex="0" style="${h.color ? `--hc:${esc(h.color)}` : ""}">
      <div class="ico" style="${h.color ? `color:${esc(h.color)};background:color-mix(in srgb, ${esc(h.color)} 14%, transparent)` : ""}">${ICONS[osIcon(h.os)]}</div>
      <div><div class="name">${esc(h.label)}${h.os ? ` <span class="pill">${esc(h.os.split(" ")[0])}</span>` : ""}</div>
        <div class="meta"><span class="mono">${esc(h.summary || hostSummary(h))}</span>${authPill ? `<span class="pill">${esc(authPill)}</span>` : ""}${jump ? `<span class="pill" title="${t("rm.jump")}">${ICONS.link}${esc(jump.label)}</span>` : ""}${h.tags.map((x) => `<span class="tag" data-tag="${esc(x)}">${esc(x)}</span>`).join("")}</div></div>
      <div class="act">
        <button class="icon-btn go" data-rm="connect" title="${t("rm.openTerminal")}">${ICONS.terminal}</button>
        <button class="icon-btn" data-rm="sftp" title="SFTP">${ICONS.sftp}</button>
        <button class="icon-btn" data-rm="edit" title="${t("btn.edit")}">${ICONS.edit}</button>
        <button class="icon-btn" data-rm="duplicate" title="${t("rm.duplicate")}">${ICONS.copy}</button>
        <button class="icon-btn danger" data-rm="delete" title="${t("btn.remove")}">${ICONS.trash}</button>
      </div></div>`;
  }
  const hostSummary = (h) => `${h.username ? h.username + "@" : ""}${h.address}${h.port && h.port !== 22 ? ":" + h.port : ""}`;
  function renderHosts() {
    if (!R.view) return;
    const { hosts, groups } = R.view.store;
    const tags = [...new Set(hosts.flatMap((h) => h.tags))].sort();
    $("#rm-tag-filter").innerHTML = tags.map((x) => `<button class="tag ${R.tag === x ? "on" : ""}" data-tag="${esc(x)}">${esc(x)}</button>`).join("");
    const visible = hosts.filter(hostMatches);
    const byGroup = (gid) => visible.filter((h) => (h.group_id || null) === gid);
    const renderGroup = (g, depth) => {
      const children = groups.filter((x) => (x.parent_id || null) === g.id);
      const own = byGroup(g.id);
      if (!own.length && !children.some((c) => countIn(c))) return "";
      return `<details class="rm-group" open style="--d:${depth}"><summary><span class="sico">${ICONS.group}</span><span>${esc(g.name)}</span><span class="count">${countIn(g)}</span>
        <span class="gact"><button class="icon-btn small" data-group-edit="${esc(g.id)}" title="${t("btn.edit")}">${ICONS.edit}</button><button class="icon-btn small" data-group-delete="${esc(g.id)}" title="${t("btn.remove")}">${ICONS.trash}</button></span></summary>
        <div class="list">${own.map(hostRow).join("")}${children.map((c) => renderGroup(c, depth + 1)).join("")}</div></details>`;
    };
    const countIn = (g) => byGroup(g.id).length + groups.filter((x) => (x.parent_id || null) === g.id).reduce((n, c) => n + countIn(c), 0);
    const top = groups.filter((g) => !g.parent_id);
    const loose = byGroup(null);
    const emptyGroups = groups.filter((g) => !countIn(g) && !g.parent_id);
    $("#rm-host-list").innerHTML =
      (loose.length ? `<div class="list">${loose.map(hostRow).join("")}</div>` : "") +
      top.map((g) => renderGroup(g, 0)).join("") +
      emptyGroups.map((g) => `<details class="rm-group" open><summary><span class="sico">${ICONS.group}</span><span>${esc(g.name)}</span><span class="count">0</span><span class="gact"><button class="icon-btn small" data-group-edit="${esc(g.id)}">${ICONS.edit}</button><button class="icon-btn small" data-group-delete="${esc(g.id)}">${ICONS.trash}</button></span></summary><div class="list"><div class="empty small">${t("rm.groupEmpty")}</div></div></details>`).join("") ||
      `<div class="empty">${ICONS.hosts}<span>${hosts.length ? t("rm.noMatch") : t("rm.noHosts")}</span></div>`;
  }
  function fillSelect(sel, items, current, none) {
    sel.innerHTML = (none !== undefined ? `<option value="">${esc(none)}</option>` : "") + items.map(([v, l]) => `<option value="${esc(v)}">${esc(l)}</option>`).join("");
    sel.value = current || "";
    if (sel.value !== (current || "")) sel.value = "";
  }
  function openHostDialog(h) {
    const f = $("#form-host"), st = R.view.store;
    f.reset();
    $("#host-dlg-title").textContent = h?.id ? t("rm.editHost") : t("rm.newHost");
    f.elements.id.value = h?.id || "";
    f.elements.label.value = h?.label || "";
    f.elements.address.value = h?.address || "";
    f.elements.port.value = h?.port || 22;
    f.elements.username.value = h?.username || "";
    f.elements.auth.value = h?.auth || "password";
    f.elements.tags.value = (h?.tags || []).join(", ");
    f.elements.env.value = (h?.env || []).join(", ");
    f.elements.notes.value = h?.notes || "";
    f.elements.keep_alive.checked = h ? !!h.keep_alive : true;
    f.elements.agent_forwarding.checked = !!h?.agent_forwarding;
    f.elements.color.value = h?.color || "";
    f.elements.password.value = ""; f.elements.password.placeholder = h?.has_password ? "•••••••• (saved)" : "";
    f.dataset.clearPw = "";
    fillSelect(f.elements.group_id, st.groups.map((g) => [g.id, g.name]), h?.group_id, t("rm.noGroup"));
    fillSelect(f.elements.key_id, st.keys.map((k) => [k.id, k.label]), h?.key_id, "—");
    fillSelect(f.elements.identity_id, st.identities.map((i) => [i.id, `${i.label} (${i.username})`]), h?.identity_id, "—");
    fillSelect(f.elements.jump_host_id, st.hosts.filter((x) => x.id !== h?.id).map((x) => [x.id, x.label]), h?.jump_host_id, t("rm.noJump"));
    fillSelect(f.elements.startup_snippet_id, st.snippets.map((s) => [s.id, s.label]), h?.startup_snippet_id, "—");
    $("#host-colors").innerHTML = COLORS.map((c) => `<button type="button" class="swatch ${(h?.color || "") === c ? "on" : ""}" data-color="${c}" style="${c ? `background:${c}` : ""}" title="${c || "none"}"></button>`).join("");
    applyAuthRows();
    $("#dlg-host").showModal();
    f.elements.label.focus();
  }
  function applyAuthRows() {
    const v = $("#form-host").elements.auth.value;
    $$("#form-host [data-auth]").forEach((el) => el.classList.toggle("hidden", el.dataset.auth !== v));
  }
  $("#form-host").elements.auth.onchange = applyAuthRows;
  $("#host-colors").onclick = (ev) => {
    const b = ev.target.closest(".swatch"); if (!b) return;
    $("#form-host").elements.color.value = b.dataset.color;
    $$("#host-colors .swatch").forEach((x) => x.classList.toggle("on", x === b));
  };
  $("#btn-host-clear-pw").onclick = () => { const f = $("#form-host"); f.elements.password.value = ""; f.elements.password.placeholder = ""; f.dataset.clearPw = "1"; };
  $("#form-host").onsubmit = async (ev) => {
    ev.preventDefault(); const f = ev.target, e = f.elements;
    const existing = e.id.value ? hostById(e.id.value) : null;
    const host = {
      ...(existing || {}),
      id: e.id.value, label: e.label.value.trim(), address: e.address.value.trim(), port: Number(e.port.value) || 22,
      username: e.username.value.trim(), group_id: e.group_id.value || null, auth: e.auth.value,
      key_id: e.key_id.value || null, identity_id: e.identity_id.value || null, jump_host_id: e.jump_host_id.value || null,
      startup_snippet_id: e.startup_snippet_id.value || null, tags: e.tags.value.split(",").map((x) => x.trim()).filter(Boolean),
      env: e.env.value.split(",").map((x) => x.trim()).filter((x) => x.includes("=")), notes: e.notes.value,
      keep_alive: e.keep_alive.checked, agent_forwarding: e.agent_forwarding.checked, color: e.color.value,
      has_password: existing?.has_password || false, os: existing?.os || "", created_at: existing?.created_at || 0, last_connected: existing?.last_connected || 0,
    };
    const password = e.password.value ? e.password.value : f.dataset.clearPw ? "" : null;
    try { await invoke("remote_save_host", { host, password }); $("#dlg-host").close(); toast(t("msg.saved2"), "ok"); } catch (x) { fail(x); }
    refresh();
  };
  function openGroupDialog(g) {
    const f = $("#form-group"); f.reset();
    f.elements.id.value = g?.id || ""; f.elements.name.value = g?.name || "";
    fillSelect(f.elements.parent_id, R.view.store.groups.filter((x) => x.id !== g?.id).map((x) => [x.id, x.name]), g?.parent_id, t("rm.noGroup"));
    fillSelect(f.elements.identity_id, R.view.store.identities.map((i) => [i.id, `${i.label} (${i.username})`]), g?.identity_id, "—");
    $("#dlg-group").showModal(); f.elements.name.focus();
  }
  $("#form-group").onsubmit = async (ev) => {
    ev.preventDefault(); const e = ev.target.elements;
    const group = { id: e.id.value, name: e.name.value.trim(), parent_id: e.parent_id.value || null, identity_id: e.identity_id.value || null, color: "" };
    try { await invoke("remote_save_group", { group }); $("#dlg-group").close(); } catch (x) { fail(x); }
    refresh();
  };
  $("#btn-rm-new-host").onclick = () => openHostDialog(null);
  $("#btn-rm-new-group").onclick = () => openGroupDialog(null);
  $("#rm-host-search").oninput = (ev) => { R.search = ev.target.value; renderHosts(); };
  $("#quick-connect").onsubmit = (ev) => {
    ev.preventDefault(); const v = ev.target.elements.target.value.trim(); if (!v) return;
    openTerminal({ quick: v, label: v }); ev.target.reset();
  };
  $("#rm-host-list").addEventListener("click", async (ev) => {
    const tag = ev.target.closest("[data-tag]"); if (tag) { R.tag = R.tag === tag.dataset.tag ? null : tag.dataset.tag; renderHosts(); return; }
    const ge = ev.target.closest("[data-group-edit]"); if (ge) { ev.preventDefault(); openGroupDialog(R.view.store.groups.find((g) => g.id === ge.dataset.groupEdit)); return; }
    const gd = ev.target.closest("[data-group-delete]");
    if (gd) { ev.preventDefault(); if (await askConfirm(t("rm.confirmDeleteGroup"))) { try { await invoke("remote_delete_group", { id: gd.dataset.groupDelete }); } catch (x) { fail(x); } refresh(); } return; }
    const row = ev.target.closest("[data-host]"); if (!row) return;
    const h = hostById(row.dataset.host); if (!h) return;
    const b = ev.target.closest("[data-rm]");
    if (!b) { if (ev.detail === 2) openTerminal({ hostId: h.id }); return; }
    switch (b.dataset.rm) {
      case "connect": openTerminal({ hostId: h.id }); break;
      case "sftp": showTab("sftp"); $("#sftp-host").value = h.id; sftpConnect(); break;
      case "edit": openHostDialog(h); break;
      case "duplicate": try { await invoke("remote_save_host", { host: { ...h, id: "", label: `${h.label} copy`, has_password: false }, password: null }); } catch (x) { fail(x); } refresh(); break;
      case "delete": if (await askConfirm(t("rm.confirmDeleteHost", { name: h.label }))) { try { await invoke("remote_delete_host", { id: h.id }); } catch (x) { fail(x); } refresh(); } break;
    }
  });
  $("#rm-tag-filter").onclick = (ev) => { const b = ev.target.closest("[data-tag]"); if (!b) return; R.tag = R.tag === b.dataset.tag ? null : b.dataset.tag; renderHosts(); };
  $("#rm-host-list").addEventListener("keydown", (ev) => { const row = ev.target.closest?.("[data-host]"); if (row && ev.key === "Enter") openTerminal({ hostId: row.dataset.host }); });

  /* ---------------- terminal themes ---------------- */
  const PALETTES = {
    dark: { black: "#1c2030", red: "#f87171", green: "#34d399", yellow: "#fbbf24", blue: "#6f8fff", magenta: "#c084fc", cyan: "#38c6ff", white: "#e8ebf2", brightBlack: "#5d667a", brightRed: "#fca5a5", brightGreen: "#6ee7b7", brightYellow: "#fde68a", brightBlue: "#a5b4fc", brightMagenta: "#d8b4fe", brightCyan: "#7dd3fc", brightWhite: "#ffffff", background: "#0f131b", foreground: "#e8ebf2", cursor: "#6f8fff", selectionBackground: "#2b3a66" },
    light: { black: "#0f1420", red: "#dc2626", green: "#16a34a", yellow: "#b45309", blue: "#3b6cf6", magenta: "#9333ea", cyan: "#0891b2", white: "#d9dde7", brightBlack: "#6b7384", brightRed: "#ef4444", brightGreen: "#22c55e", brightYellow: "#d97706", brightBlue: "#60a5fa", brightMagenta: "#a855f7", brightCyan: "#06b6d4", brightWhite: "#ffffff", background: "#ffffff", foreground: "#0f1420", cursor: "#3b6cf6", selectionBackground: "#dbe4ff" },
    dracula: { black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c", blue: "#bd93f9", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2", brightBlack: "#6272a4", brightRed: "#ff6e6e", brightGreen: "#69ff94", brightYellow: "#ffffa5", brightBlue: "#d6acff", brightMagenta: "#ff92df", brightCyan: "#a4ffff", brightWhite: "#ffffff", background: "#282a36", foreground: "#f8f8f2", cursor: "#f8f8f2", selectionBackground: "#44475a" },
    solarized: { black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5", brightBlack: "#586e75", brightRed: "#cb4b16", brightGreen: "#586e75", brightYellow: "#657b83", brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3", background: "#002b36", foreground: "#839496", cursor: "#93a1a1", selectionBackground: "#073642" },
    nord: { black: "#3b4252", red: "#bf616a", green: "#a3be8c", yellow: "#ebcb8b", blue: "#81a1c1", magenta: "#b48ead", cyan: "#88c0d0", white: "#e5e9f0", brightBlack: "#4c566a", brightRed: "#bf616a", brightGreen: "#a3be8c", brightYellow: "#ebcb8b", brightBlue: "#81a1c1", brightMagenta: "#b48ead", brightCyan: "#8fbcbb", brightWhite: "#eceff4", background: "#2e3440", foreground: "#d8dee9", cursor: "#d8dee9", selectionBackground: "#434c5e" },
  };
  const isDarkApp = () => document.documentElement.dataset.theme === "dark" || (!document.documentElement.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
  function xtermTheme() {
    const name = settings().theme || "auto";
    if (name !== "auto" && PALETTES[name]) return PALETTES[name];
    const base = { ...(isDarkApp() ? PALETTES.dark : PALETTES.light) };
    const css = getComputedStyle(document.documentElement);
    const surface = css.getPropertyValue("--surface").trim(); if (surface) base.background = surface;
    const text = css.getPropertyValue("--text").trim(); if (text) base.foreground = text;
    return base;
  }
  function xtermOptions() {
    const s = settings();
    return {
      theme: xtermTheme(),
      fontFamily: s.font_family || `"JetBrains Mono", "Cascadia Code", "SF Mono", Menlo, Consolas, ui-monospace, monospace`,
      fontSize: s.font_size || 13, cursorStyle: s.cursor_style || "block", cursorBlink: !!s.cursor_blink,
      scrollback: s.scrollback || 5000, allowProposedApi: true, convertEol: false, macOptionIsMeta: true,
      rightClickSelectsWord: false, allowTransparency: false, drawBoldTextInBrightColors: true,
    };
  }
  function applyThemeToTerms() {
    for (const x of R.terms.values()) { const o = xtermOptions(); x.term.options.theme = o.theme; x.term.options.fontFamily = o.fontFamily; x.term.options.fontSize = o.fontSize; x.term.options.cursorStyle = o.cursorStyle; x.term.options.cursorBlink = o.cursorBlink; x.term.options.scrollback = o.scrollback; try { x.fit.fit(); } catch (e) {} }
  }
  new MutationObserver(() => applyThemeToTerms()).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyThemeToTerms);

  /* ---------------- terminals ---------------- */
  function termTitle(x) { return x.title || x.info?.label || x.label || (x.local ? t("rm.local") : "…"); }
  function renderTabs() {
    const tabs = $("#term-tabs");
    tabs.innerHTML = [...R.terms.values()].map((x) => `<div class="term-tab ${x.id === R.active ? "active" : ""} ${x.closed ? "closed" : ""}" data-term="${x.id}" title="${esc(x.info?.target || "")}">
      <i class="tdot ${x.info?.state || "connecting"}"></i><span class="ttitle">${esc(termTitle(x))}</span><button class="tclose" data-term-close="${x.id}" title="${t("btn.close")}">${ICONS.x}</button></div>`).join("") +
      `<button class="term-add" id="term-add" title="${t("rm.localTerminal")}">${ICONS.plus}</button>`;
    $("#term-empty").classList.toggle("hidden", R.terms.size > 0);
    $("#term-stage").classList.toggle("hidden", R.terms.size === 0);
    renderCounts();
  }
  function barMarkup(x) {
    const snippets = R.view?.store.snippets || [];
    return `<div class="term-bar">
      <span class="tstate ${x.info?.state || "connecting"}"><i></i><span class="txt">${esc(stateText(x))}</span></span>
      <span class="ttarget mono">${esc(x.info?.target || "")}</span>
      <span class="spacer"></span>
      <select class="small term-snippet" title="${t("nav.snippets")}"><option value="">${t("rm.sendSnippet")}</option>${snippets.map((s) => `<option value="${esc(s.id)}">${esc(s.folder ? s.folder + " / " : "")}${esc(s.label)}</option>`).join("")}</select>
      <button class="icon-btn small" data-tact="search" title="${t("rm.search")} (Ctrl+Shift+F)">${ICONS.search}</button>
      <button class="icon-btn small" data-tact="clear" title="${t("btn.clear")}">${ICONS.trash}</button>
      <button class="icon-btn small" data-tact="reconnect" title="${t("rm.reconnect")}">${ICONS.refresh}</button>
      <button class="icon-btn small" data-tact="settings" title="${t("rm.terminalSettings")}">${ICONS.gear}</button>
      <button class="icon-btn small" data-tact="close" title="${t("btn.close")}">${ICONS.x}</button>
    </div>
    <div class="term-search hidden"><input placeholder="${t("rm.search")}"><button class="icon-btn small" data-tact="prev">${ICONS.up}</button><button class="icon-btn small" data-tact="next"><i class="ic ic-flip">${ICONS.up}</i></button><button class="icon-btn small" data-tact="search">${ICONS.x}</button></div>`;
  }
  const stateText = (x) => {
    const s = x.info?.state || "connecting";
    if (s === "connected") return t("status.connected");
    if (s === "closed") return `${t("rm.closed")}${x.info?.message ? " · " + x.info.message : ""}`;
    if (s === "error") return `${t("rm.failed")}: ${x.info?.message || ""}`;
    return t("rm.connecting");
  };
  function updateBar(x) {
    const bar = $(".term-bar", x.el); if (!bar) return;
    const st = $(".tstate", bar); st.className = `tstate ${x.info?.state || "connecting"}`; $(".txt", st).textContent = stateText(x);
    $(".ttarget", bar).textContent = x.info?.target || "";
  }
  async function openTerminal(opts) {
    const id = newId();
    const el = document.createElement("div"); el.className = "term-box"; el.dataset.term = id;
    el.innerHTML = `<div class="term-head"></div><div class="term-view"></div>`;
    $(".term-head", el).innerHTML = "";
    $("#term-stage").appendChild(el);
    const term = new Terminal(xtermOptions());
    const fit = new FitAddon.FitAddon(), search = new SearchAddon.SearchAddon();
    term.loadAddon(fit); term.loadAddon(search);
    term.loadAddon(new WebLinksAddon.WebLinksAddon((ev, uri) => invoke("open_url_external", { url: uri }).catch(fail)));
    try { term.loadAddon(new Unicode11Addon.Unicode11Addon()); term.unicode.activeVersion = "11"; } catch (e) {}
    const x = { id, term, fit, search, el, title: "", info: { state: "connecting", label: opts.label || hostById(opts.hostId)?.label || (opts.local ? t("rm.local") : ""), target: opts.quick || (hostById(opts.hostId) ? hostSummary(hostById(opts.hostId)) : "") }, hostId: opts.hostId || null, quick: opts.quick || null, local: !!opts.local, closed: false };
    R.terms.set(id, x);
    $(".term-head", el).innerHTML = barMarkup(x);
    term.open($(".term-view", el));
    term.onData((d) => { if (!x.closed) invoke("term_write", { id, data: d }).catch(() => {}); });
    term.onResize(({ cols, rows }) => { if (!x.closed) invoke("term_resize", { id, cols, rows }).catch(() => {}); });
    term.onTitleChange((title) => { x.title = title; renderTabs(); });
    term.onSelectionChange(() => { if (settings().copy_on_select && term.hasSelection()) navigator.clipboard.writeText(term.getSelection()).catch(() => {}); });
    term.onBell(() => { if (settings().bell) { try { const ctx = new AudioContext(); const o = ctx.createOscillator(); o.frequency.value = 880; o.connect(ctx.destination); o.start(); o.stop(ctx.currentTime + 0.08); } catch (e) {} } });
    term.attachCustomKeyEventHandler((ev) => {
      if (ev.type !== "keydown") return true;
      if (ev.ctrlKey && ev.shiftKey && ev.code === "KeyC") { if (term.hasSelection()) navigator.clipboard.writeText(term.getSelection()).catch(() => {}); return false; }
      if (ev.ctrlKey && ev.shiftKey && ev.code === "KeyV") { navigator.clipboard.readText().then((tx) => tx && term.paste(tx)).catch(() => {}); return false; }
      if (ev.ctrlKey && ev.shiftKey && ev.code === "KeyF") { toggleSearch(x, true); return false; }
      if (ev.ctrlKey && ev.shiftKey && ev.code === "KeyW") { closeTerminal(id); return false; }
      if (ev.ctrlKey && ev.shiftKey && ev.code === "KeyT") { openTerminal({ local: true }); return false; }
      return true;
    });
    $(".term-view", el).addEventListener("contextmenu", (ev) => {
      if (!settings().right_click_paste) return;
      ev.preventDefault();
      if (term.hasSelection()) { navigator.clipboard.writeText(term.getSelection()).catch(() => {}); term.clearSelection(); }
      else navigator.clipboard.readText().then((tx) => tx && term.paste(tx)).catch(() => {});
    });
    activateTerminal(id);
    showTab("terminal");
    await new Promise((r) => requestAnimationFrame(r));
    try { fit.fit(); } catch (e) {}
    term.focus();
    try {
      const info = await invoke("term_open", { req: { id, host_id: opts.hostId || null, quick: opts.quick || null, local: !!opts.local, cols: term.cols, rows: term.rows, cwd: opts.cwd || null } });
      x.info = { ...x.info, ...info };
    } catch (e) {
      x.info.state = "error"; x.info.message = String(e?.message || e); x.closed = true;
      term.write(`\r\n\x1b[31m${t("rm.failed")}: ${String(e?.message || e).replace(/\r?\n/g, "\r\n")}\x1b[0m\r\n`);
      toast(`${x.info.label}: ${e?.message || e}`, "bad");
    }
    updateBar(x); renderTabs();
    return id;
  }
  function activateTerminal(id) {
    R.active = id;
    for (const x of R.terms.values()) x.el.classList.toggle("active", x.id === id);
    renderTabs();
    const x = R.terms.get(id);
    if (x) requestAnimationFrame(() => { try { x.fit.fit(); } catch (e) {} x.term.focus(); });
  }
  async function closeTerminal(id, force) {
    const x = R.terms.get(id); if (!x) return;
    if (!x.closed && !force && settings().confirm_close && !(await askConfirm(t("rm.confirmCloseSession", { name: termTitle(x) }), t("btn.close")))) return;
    if (!x.closed) invoke("term_close", { id }).catch(() => {});
    x.term.dispose(); x.el.remove(); R.terms.delete(id);
    if (R.active === id) { const next = [...R.terms.keys()].pop(); R.active = null; if (next) activateTerminal(next); }
    renderTabs();
  }
  function toggleSearch(x, show) {
    const box = $(".term-search", x.el);
    const on = show ?? box.classList.contains("hidden");
    box.classList.toggle("hidden", !on);
    if (on) $("input", box).focus(); else { x.search.clearDecorations(); x.term.focus(); }
  }
  async function reconnect(x) {
    if (!x.closed) return;
    const opts = x.local ? { local: true } : x.hostId ? { hostId: x.hostId } : { quick: x.quick, label: x.info.label };
    await closeTerminal(x.id, true);
    openTerminal(opts);
  }
  $("#term-tabs").addEventListener("click", (ev) => {
    const c = ev.target.closest("[data-term-close]"); if (c) { closeTerminal(c.dataset.termClose); return; }
    if (ev.target.closest("#term-add")) { openTerminal({ local: true }); return; }
    const tab = ev.target.closest("[data-term]"); if (tab) activateTerminal(tab.dataset.term);
  });
  $("#term-tabs").addEventListener("auxclick", (ev) => { const tab = ev.target.closest("[data-term]"); if (tab && ev.button === 1) closeTerminal(tab.dataset.term); });
  $("#term-stage").addEventListener("click", async (ev) => {
    const box = ev.target.closest(".term-box"); if (!box) return;
    const x = R.terms.get(box.dataset.term); if (!x) return;
    const b = ev.target.closest("[data-tact]"); if (!b) return;
    switch (b.dataset.tact) {
      case "search": toggleSearch(x); break;
      case "next": x.search.findNext($(".term-search input", x.el).value); break;
      case "prev": x.search.findPrevious($(".term-search input", x.el).value); break;
      case "clear": x.term.clear(); x.term.focus(); break;
      case "reconnect": if (x.closed) reconnect(x); else toast(t("rm.stillConnected"), "info"); break;
      case "settings": openTermSettings(); break;
      case "close": closeTerminal(x.id); break;
    }
  });
  $("#term-stage").addEventListener("input", (ev) => {
    const inp = ev.target.closest(".term-search input"); if (!inp) return;
    const x = R.terms.get(ev.target.closest(".term-box").dataset.term);
    x.search.findNext(inp.value, { incremental: true });
  });
  $("#term-stage").addEventListener("keydown", (ev) => {
    const inp = ev.target.closest?.(".term-search input"); if (!inp) return;
    const x = R.terms.get(ev.target.closest(".term-box").dataset.term);
    if (ev.key === "Enter") { ev.preventDefault(); if (ev.shiftKey) x.search.findPrevious(inp.value); else x.search.findNext(inp.value); }
    if (ev.key === "Escape") toggleSearch(x, false);
  });
  $("#term-stage").addEventListener("change", async (ev) => {
    const sel = ev.target.closest(".term-snippet"); if (!sel || !sel.value) return;
    const x = R.terms.get(ev.target.closest(".term-box").dataset.term);
    const s = R.view.store.snippets.find((q) => q.id === sel.value); sel.value = "";
    if (s && x && !x.closed) { sendToTerminal(x.id, s.script); x.term.focus(); }
  });
  function sendToTerminal(id, text) {
    const x = R.terms.get(id); if (!x || x.closed) return false;
    invoke("term_write", { id, data: text.endsWith("\n") ? text : text + "\n" }).catch(fail);
    return true;
  }
  $("#btn-term-local").onclick = () => openTerminal({ local: true });
  new ResizeObserver(() => { const x = R.terms.get(R.active); if (x && $("#tab-terminal").classList.contains("active")) { try { x.fit.fit(); } catch (e) {} } }).observe($("#term-stage"));
  document.addEventListener("veranda:tab", (ev) => {
    if (ev.detail === "terminal") { const x = R.terms.get(R.active); if (x) requestAnimationFrame(() => { try { x.fit.fit(); } catch (e) {} x.term.focus(); }); }
    else if (["hosts", "forward", "snippets", "keychain", "history", "sftp"].includes(ev.detail)) refresh();
    if (ev.detail === "sftp" && !R.panes.local.path) loadPane("local", settings().sftp_local_root || "");
    if (ev.detail === "knownhosts") loadKnownHosts();
  });
  listen("term-out", (ev) => { const x = R.terms.get(ev.payload.id); if (x) x.term.write(b64(ev.payload.data)); });
  listen("term-state", (ev) => {
    const x = R.terms.get(ev.payload.id); if (!x) return;
    x.info = { ...x.info, ...ev.payload };
    if (ev.payload.state === "closed" || ev.payload.state === "error") {
      if (!x.closed) x.term.write(`\r\n\x1b[2m[${t("rm.sessionEnded")}${ev.payload.message ? ": " + ev.payload.message : ""}] ${t("rm.pressReconnect")}\x1b[0m\r\n`);
      x.closed = true;
    }
    updateBar(x); renderTabs();
  });
  listen("term-exit", (ev) => { const x = R.terms.get(ev.payload.id); if (x) { x.closed = true; renderTabs(); } });
  listen("remote-notice", (ev) => {
    const x = R.terms.get(ev.payload.session);
    if (x) x.term.write(`\x1b[2m${String(ev.payload.text).replace(/\r?\n/g, "\r\n")}\x1b[0m\r\n`); else toast(ev.payload.text, "info");
  });
  listen("remote-changed", () => refresh());

  /* ---------------- prompts (host key, password, passphrase, OTP) ---------------- */
  listen("remote-prompt", (ev) => { R.promptQueue.push(ev.payload); showNextPrompt(); });
  listen("remote-prompt-done", (ev) => {
    R.promptQueue = R.promptQueue.filter((p) => p.id !== ev.payload);
    if (R.promptOpen?.id === ev.payload) { R.promptOpen = null; $("#dlg-prompt").close(); showNextPrompt(); }
  });
  function showNextPrompt() {
    if (R.promptOpen || !R.promptQueue.length) return;
    const p = R.promptQueue.shift(); R.promptOpen = p;
    const f = $("#form-prompt"); f.reset();
    const fp = $("#prompt-fp");
    $("#prompt-title").textContent = p.kind === "hostkey" ? (p.status === "changed" ? t("rm.hostKeyChanged") : t("rm.hostKeyNew")) : p.title || "";
    if (p.kind === "hostkey") {
      $("#prompt-text").textContent = p.status === "changed" ? t("rm.hostKeyChangedText", { host: `${p.host}:${p.port}` }) : t("rm.hostKeyNewText", { host: `${p.host}:${p.port}` });
      fp.innerHTML = `<b>${esc(p.algorithm)}</b><code>${esc(p.fingerprint)}</code>`; fp.classList.remove("hidden"); fp.classList.toggle("warn", p.status === "changed");
      $("#prompt-input-row").classList.add("hidden"); $("#prompt-remember-row").classList.add("hidden");
      $("#btn-prompt-ok").textContent = p.status === "changed" ? t("rm.trustAnyway") : t("rm.trust");
      $("#btn-prompt-ok").classList.toggle("danger", p.status === "changed");
    } else {
      $("#prompt-text").textContent = ""; fp.classList.add("hidden");
      $("#prompt-input-row").classList.remove("hidden");
      $("#prompt-label").textContent = p.prompt || (p.kind === "passphrase" ? t("rm.passphrase") : t("db.password"));
      f.elements.value.type = p.echo ? "text" : "password";
      $("#prompt-remember-row").classList.toggle("hidden", !p.can_remember);
      $("#btn-prompt-ok").textContent = "OK"; $("#btn-prompt-ok").classList.remove("danger");
    }
    const dlg = $("#dlg-prompt"); if (!dlg.open) dlg.showModal();
    if (p.kind !== "hostkey") setTimeout(() => f.elements.value.focus(), 50);
  }
  async function answerPrompt(accept) {
    const p = R.promptOpen; if (!p) return;
    const f = $("#form-prompt");
    R.promptOpen = null; $("#dlg-prompt").close();
    try { await invoke("remote_prompt_answer", { id: p.id, accept, value: accept ? f.elements.value.value : null, remember: accept && f.elements.remember.checked }); } catch (e) {}
    showNextPrompt();
  }
  $("#form-prompt").onsubmit = (ev) => { ev.preventDefault(); answerPrompt(true); };
  $("#btn-prompt-cancel").onclick = () => answerPrompt(false);
  $("#dlg-prompt").oncancel = (ev) => { ev.preventDefault(); answerPrompt(false); };

  /* ---------------- terminal settings ---------------- */
  function openTermSettings() {
    const f = $("#form-term-settings"), s = settings();
    for (const el of f.elements) { if (!el.name) continue; if (el.type === "checkbox") el.checked = !!s[el.name]; else el.value = s[el.name] ?? ""; }
    $("#dlg-term-settings").showModal();
  }
  $("#form-term-settings").onsubmit = async (ev) => {
    ev.preventDefault(); const s = { ...settings() };
    for (const el of ev.target.elements) { if (!el.name) continue; s[el.name] = el.type === "checkbox" ? el.checked : el.type === "number" ? Number(el.value) : el.value; }
    try { await invoke("remote_save_settings", { settings: s }); $("#dlg-term-settings").close(); } catch (e) { fail(e); }
    await refresh(); applyThemeToTerms(); renderPane("local"); renderPane("remote");
  };

  /* ---------------- SFTP ---------------- */
  function renderSftpHosts() {
    const sel = $("#sftp-host"); const cur = sel.value;
    sel.innerHTML = hostsOf().map((h) => `<option value="${esc(h.id)}">${esc(h.label)} · ${esc(hostSummary(h))}</option>`).join("") || `<option value="">${t("rm.noHosts")}</option>`;
    if (cur) sel.value = cur;
  }
  async function sftpConnect() {
    const hostId = $("#sftp-host").value; if (!hostId) { toast(t("rm.noHosts"), "info"); return; }
    if (R.sftp) await sftpDisconnect();
    const b = $("#btn-sftp-connect"); b.disabled = true; b.classList.add("busy");
    try {
      R.sftp = await invoke("sftp_open", { hostId });
      $("#btn-sftp-disconnect").disabled = false; $("#btn-sftp-upload").disabled = false; $("#btn-sftp-download").disabled = false;
      await Promise.all([loadPane("remote", R.sftp.home), R.panes.local.path ? Promise.resolve() : loadPane("local", settings().sftp_local_root || "")]);
      toast(t("rm.sftpConnected", { name: R.sftp.label }), "ok");
    } catch (e) { fail(e); R.sftp = null; renderPane("remote"); }
    b.disabled = false; b.classList.remove("busy");
  }
  async function sftpDisconnect() {
    if (R.sftp) { try { await invoke("sftp_close", { id: R.sftp.id }); } catch (e) {} }
    R.sftp = null; R.panes.remote = { path: "", entries: [], sel: new Set() };
    $("#btn-sftp-disconnect").disabled = true; $("#btn-sftp-upload").disabled = true; $("#btn-sftp-download").disabled = true;
    renderPane("remote");
  }
  async function loadPane(side, path) {
    const pane = R.panes[side];
    try {
      const r = side === "local" ? await invoke("local_list", { path: path ?? pane.path }) : await invoke("sftp_list", { id: R.sftp.id, path: path ?? pane.path });
      pane.path = r.path; pane.entries = r.entries; pane.roots = r.roots; pane.sel = new Set();
    } catch (e) { fail(e); }
    renderPane(side);
  }
  function paneUp(side) {
    const p = R.panes[side].path; if (!p) return;
    if (side === "local") { const sep = p.includes("\\") && !p.includes("/") ? "\\" : "/"; const parts = p.split(sep).filter(Boolean); if (parts.length <= (sep === "\\" ? 1 : 0)) { loadPane(side, sep === "\\" ? parts[0] + "\\" : "/"); return; } parts.pop(); loadPane(side, sep === "\\" ? (parts.length === 1 ? parts[0] + "\\" : parts.join("\\")) : "/" + parts.join("/")); }
    else loadPane(side, p.replace(/\/+$/, "").split("/").slice(0, -1).join("/") || "/");
  }
  function renderPane(side) {
    const pane = R.panes[side], el = $(`#pane-${side}`), connected = side === "local" || !!R.sftp;
    const showHidden = !!settings().show_hidden;
    const entries = pane.entries.filter((e) => showHidden || !e.hidden);
    el.innerHTML = `<div class="pane-head">
        <span class="pane-title">${side === "local" ? ICONS.local + `<b>${t("rm.thisComputer")}</b>` : ICONS.host + `<b>${esc(R.sftp?.label || t("rm.remote"))}</b>`}</span>
        <span class="spacer"></span>
        <button class="icon-btn small" data-pact="up" title="${t("rm.up")}" ${connected ? "" : "disabled"}>${ICONS.up}</button>
        <button class="icon-btn small" data-pact="home" title="Home" ${connected ? "" : "disabled"}>${ICONS.home}</button>
        <button class="icon-btn small" data-pact="refresh" title="${t("btn.refresh")}" ${connected ? "" : "disabled"}>${ICONS.refresh}</button>
        <button class="icon-btn small" data-pact="mkdir" title="${t("rm.newFolder")}" ${connected ? "" : "disabled"}>${ICONS.folder}</button>
        ${side === "local" ? `<button class="icon-btn small" data-pact="open" title="${t("btn.open")}">${ICONS.open}</button>` : ""}
      </div>
      <form class="pane-path"><input name="path" value="${esc(pane.path)}" spellcheck="false" ${connected ? "" : "disabled"}></form>
      <div class="pane-list" ${connected ? "" : "data-off"}>${connected ? (entries.map((e) => `<div class="prow ${pane.sel.has(e.path) ? "sel" : ""} k-${e.kind}" data-path="${esc(e.path)}" data-kind="${e.kind}" draggable="true">
          <span class="pico">${e.kind === "dir" ? ICONS.folder : e.kind === "link" ? ICONS.link : ICONS.file}</span><span class="pname">${esc(e.name)}</span><span class="psize">${e.kind === "dir" ? "" : fmtSize(e.size)}</span><span class="ptime">${fmtTime(e.mtime)}</span>
          <span class="pact"><button class="icon-btn small" data-pact="rename" title="${t("rm.rename")}">${ICONS.edit}</button>${side === "remote" && e.kind === "file" ? `<button class="icon-btn small" data-pact="edit" title="${t("btn.edit")}">${ICONS.file}</button>` : ""}${side === "remote" ? `<button class="icon-btn small" data-pact="chmod" title="chmod">${ICONS.lock}</button>` : ""}<button class="icon-btn small danger" data-pact="delete" title="${t("btn.remove")}">${ICONS.trash}</button></span>
        </div>`).join("") || `<div class="empty small">${t("rm.emptyFolder")}</div>`) : `<div class="empty small">${t("rm.sftpOff")}</div>`}</div>
      <div class="pane-foot">${connected ? t("rm.items", { n: entries.length, sel: pane.sel.size }) : ""}</div>`;
  }
  async function paneAction(side, act, row) {
    const pane = R.panes[side], path = row?.dataset.path;
    const isLocal = side === "local";
    const name = path ? path.split(/[\\/]/).pop() : "";
    switch (act) {
      case "up": paneUp(side); break;
      case "home": loadPane(side, isLocal ? R.view.home : R.sftp.home); break;
      case "refresh": loadPane(side); break;
      case "open": invoke("open_target", { what: `path:${pane.path}` }).catch(fail); break;
      case "mkdir": {
        const n = await askText(t("rm.newFolder"), t("rm.name"), ""); if (!n) return;
        const full = joinPath(side, pane.path, n);
        try { if (isLocal) await invoke("local_mkdir", { path: full }); else await invoke("sftp_mkdir", { id: R.sftp.id, path: full }); } catch (e) { fail(e); }
        loadPane(side); break;
      }
      case "rename": {
        const n = await askText(t("rm.rename"), t("rm.name"), name); if (!n || n === name) return;
        const to = joinPath(side, pane.path, n);
        try { if (isLocal) await invoke("local_rename", { from: path, to }); else await invoke("sftp_rename", { id: R.sftp.id, from: path, to }); } catch (e) { fail(e); }
        loadPane(side); break;
      }
      case "delete": {
        if (!(await askConfirm(t("rm.confirmDeleteFile", { name })))) return;
        try { if (isLocal) await invoke("local_delete", { path }); else await invoke("sftp_delete", { id: R.sftp.id, path }); } catch (e) { fail(e); }
        loadPane(side); break;
      }
      case "chmod": {
        const e = pane.entries.find((x) => x.path === path);
        const n = await askText(`chmod ${name}`, t("rm.mode"), (e?.mode || 0).toString(8).padStart(3, "0")); if (!n) return;
        try { await invoke("sftp_chmod", { id: R.sftp.id, path, mode: parseInt(n, 8) }); } catch (x) { fail(x); }
        loadPane(side); break;
      }
      case "edit": {
        try {
          const text = await invoke("sftp_read", { id: R.sftp.id, path });
          const edited = await askEditor(name, text); if (edited === null) return;
          await invoke("sftp_write", { id: R.sftp.id, path, text: edited }); toast(t("msg.saved2"), "ok"); loadPane(side);
        } catch (e) { fail(e); }
        break;
      }
    }
  }
  const joinPath = (side, dir, name) => side === "local" ? (dir.endsWith("/") || dir.endsWith("\\") ? dir + name : dir + (dir.includes("\\") ? "\\" : "/") + name) : (dir.endsWith("/") ? dir + name : dir + "/" + name);
  function wirePane(side) {
    const el = $(`#pane-${side}`);
    el.addEventListener("click", (ev) => {
      const b = ev.target.closest("[data-pact]"); const row = ev.target.closest(".prow");
      if (b) { ev.stopPropagation(); paneAction(side, b.dataset.pact, row); return; }
      if (!row) return;
      const pane = R.panes[side];
      if (ev.detail === 2) { if (row.dataset.kind === "dir") loadPane(side, row.dataset.path); else if (side === "remote") paneAction(side, "edit", row); return; }
      if (ev.ctrlKey || ev.metaKey) { pane.sel.has(row.dataset.path) ? pane.sel.delete(row.dataset.path) : pane.sel.add(row.dataset.path); }
      else if (ev.shiftKey && pane.sel.size) { const rows = $$(".prow", el).map((r) => r.dataset.path); const last = [...pane.sel].pop(); const [a, c] = [rows.indexOf(last), rows.indexOf(row.dataset.path)].sort((p, q) => p - q); rows.slice(a, c + 1).forEach((p) => pane.sel.add(p)); }
      else { pane.sel = new Set([row.dataset.path]); }
      $$(".prow", el).forEach((r) => r.classList.toggle("sel", pane.sel.has(r.dataset.path)));
      $(".pane-foot", el).textContent = t("rm.items", { n: $$(".prow", el).length, sel: pane.sel.size });
    });
    el.addEventListener("submit", (ev) => { ev.preventDefault(); loadPane(side, ev.target.elements.path.value.trim()); });
    el.addEventListener("dragstart", (ev) => { const row = ev.target.closest(".prow"); if (!row) return; const pane = R.panes[side]; if (!pane.sel.has(row.dataset.path)) { pane.sel = new Set([row.dataset.path]); renderPane(side); } ev.dataTransfer.setData("text/plain", JSON.stringify({ side, paths: [...pane.sel] })); ev.dataTransfer.effectAllowed = "copy"; });
    el.addEventListener("dragover", (ev) => { ev.preventDefault(); el.classList.add("drop"); });
    el.addEventListener("dragleave", () => el.classList.remove("drop"));
    el.addEventListener("drop", (ev) => {
      ev.preventDefault(); el.classList.remove("drop");
      try { const d = JSON.parse(ev.dataTransfer.getData("text/plain")); if (d.side !== side) transfer(d.side === "local" ? "upload" : "download", d.paths); } catch (e) {}
    });
    el.addEventListener("keydown", (ev) => { if (ev.key === "Backspace") paneUp(side); if (ev.key === "F5") loadPane(side); });
  }
  wirePane("local"); wirePane("remote");
  $("#btn-sftp-connect").onclick = sftpConnect;
  $("#btn-sftp-disconnect").onclick = sftpDisconnect;
  $("#btn-sftp-upload").onclick = () => transfer("upload", [...R.panes.local.sel]);
  $("#btn-sftp-download").onclick = () => transfer("download", [...R.panes.remote.sel]);

  async function transfer(dir, paths) {
    if (!R.sftp) { toast(t("rm.sftpOff"), "info"); return; }
    if (!paths.length) { toast(t("rm.selectFirst"), "info"); return; }
    const srcSide = dir === "upload" ? "local" : "remote", dstSide = dir === "upload" ? "remote" : "local";
    const src = R.panes[srcSide], dst = R.panes[dstSide];
    const jobs = [];
    const walk = async (p, relDir) => {
      const e = src.entries.find((x) => x.path === p) || (await statViaList(srcSide, p));
      if (!e) return;
      if (e.kind === "dir") {
        const r = srcSide === "local" ? await invoke("local_list", { path: p }) : await invoke("sftp_list", { id: R.sftp.id, path: p });
        const sub = joinPath(dstSide, relDir, e.name);
        jobs.push({ mkdir: sub });
        for (const c of r.entries) await walkEntry(c, sub);
      } else jobs.push({ src: p, dst: joinPath(dstSide, relDir, e.name), name: e.name, size: e.size });
    };
    const walkEntry = async (e, relDir) => {
      if (e.kind === "dir") {
        const r = srcSide === "local" ? await invoke("local_list", { path: e.path }) : await invoke("sftp_list", { id: R.sftp.id, path: e.path });
        const sub = joinPath(dstSide, relDir, e.name); jobs.push({ mkdir: sub });
        for (const c of r.entries) await walkEntry(c, sub);
      } else jobs.push({ src: e.path, dst: joinPath(dstSide, relDir, e.name), name: e.name, size: e.size });
    };
    try { for (const p of paths) await walk(p, dst.path); } catch (e) { fail(e); return; }
    for (const j of jobs) {
      if (j.mkdir) { try { if (dstSide === "local") await invoke("local_mkdir", { path: j.mkdir }); else await invoke("sftp_mkdir", { id: R.sftp.id, path: j.mkdir }); } catch (e) {} continue; }
      const id = newId();
      const tr = { id, name: j.name, dir, done: 0, total: j.size, state: "running" };
      R.transfers.unshift(tr); renderTransfers();
      try {
        if (dir === "upload") await invoke("sftp_upload", { id: R.sftp.id, local: j.src, remote: j.dst, transfer: id });
        else await invoke("sftp_download", { id: R.sftp.id, remote: j.src, local: j.dst, transfer: id });
        tr.state = "done"; tr.done = tr.total;
      } catch (e) { tr.state = /cancel/i.test(String(e)) ? "cancelled" : "error"; tr.error = String(e?.message || e); if (tr.state === "error") toast(`${j.name}: ${tr.error}`, "bad"); }
      renderTransfers();
    }
    loadPane(dstSide);
  }
  async function statViaList(side, p) {
    const dir = side === "local" ? p.replace(/[\\/][^\\/]+$/, "") || "/" : p.split("/").slice(0, -1).join("/") || "/";
    try { const r = side === "local" ? await invoke("local_list", { path: dir }) : await invoke("sftp_list", { id: R.sftp.id, path: dir }); return r.entries.find((x) => x.path === p); } catch (e) { return null; }
  }
  function renderTransfers() {
    const box = $("#sftp-transfers");
    R.transfers = R.transfers.slice(0, 30);
    box.innerHTML = R.transfers.map((x) => `<div class="transfer ${x.state}"><span class="tico">${x.dir === "upload" ? ICONS.upload : ICONS.download}</span><span class="tname">${esc(x.name)}</span>
      <span class="tbar"><span class="fill" style="width:${x.total ? Math.min(100, 100 * x.done / x.total).toFixed(0) : (x.state === "running" ? 30 : 100)}%"></span></span>
      <span class="ttxt mono">${x.state === "running" ? `${fmtSize(x.done)} / ${fmtSize(x.total)}` : x.state === "done" ? t("pkg.done") : x.state === "cancelled" ? t("rm.cancelled") : esc(x.error || "error")}</span>
      ${x.state === "running" ? `<button class="icon-btn small" data-cancel-transfer="${x.id}" title="${t("btn.cancel")}">${ICONS.x}</button>` : ""}</div>`).join("");
  }
  $("#sftp-transfers").onclick = (ev) => { const b = ev.target.closest("[data-cancel-transfer]"); if (b) invoke("sftp_cancel", { transfer: b.dataset.cancelTransfer }).catch(() => {}); };
  listen("sftp-progress", (ev) => { const x = R.transfers.find((q) => q.id === ev.payload.transfer); if (!x) return; x.done = ev.payload.done; x.total = ev.payload.total || x.total; const row = $(`[data-cancel-transfer="${x.id}"]`)?.closest(".transfer"); if (row) { $(".fill", row).style.width = `${x.total ? Math.min(100, 100 * x.done / x.total).toFixed(0) : 30}%`; $(".ttxt", row).textContent = `${fmtSize(x.done)} / ${fmtSize(x.total)}`; } });

  /* ---------------- port forwarding ---------------- */
  function renderForwards() {
    if (!R.view) return;
    const active = Object.fromEntries((R.view.forwards_active || []).map((f) => [f.rule_id, f]));
    const rules = R.view.store.forwards;
    $("#fwd-list").innerHTML = rules.map((r) => {
      const h = hostById(r.host_id), a = active[r.id];
      const desc = r.kind === "dynamic" ? `SOCKS5 ${r.bind_address}:${r.bind_port}` : r.kind === "local" ? `${r.bind_address}:${r.bind_port} → ${r.dest_host}:${r.dest_port}` : `${t("rm.server")} ${r.bind_address}:${a?.bound_port || r.bind_port} → ${r.dest_host}:${r.dest_port}`;
      return `<div class="row ${a ? "on" : "off"}" data-fwd="${esc(r.id)}">
        <div class="ico">${ICONS.forward}</div>
        <div><div class="name">${esc(r.label)} <span class="pill">${t(`rm.fwd${r.kind[0].toUpperCase()}${r.kind.slice(1)}`)}</span></div>
          <div class="meta"><span class="mono">${esc(desc)}</span><span class="pill">${esc(h?.label || "?")}</span>${a ? `<span class="state on"><i></i>${t("rm.connections", { n: a.connections, total: a.total })}</span>` : `<span class="state"><i></i>${t("status.stopped")}</span>`}</div></div>
        <div class="act"><input type="checkbox" class="switch" data-fwd-switch="${esc(r.id)}" ${a ? "checked" : ""} ${h ? "" : "disabled"}>
          <button class="icon-btn" data-fwd-edit="${esc(r.id)}" title="${t("btn.edit")}">${ICONS.edit}</button>
          <button class="icon-btn danger" data-fwd-delete="${esc(r.id)}" title="${t("btn.remove")}">${ICONS.trash}</button></div></div>`;
    }).join("") || `<div class="empty">${ICONS.forward}<span>${t("rm.noRules")}</span></div>`;
  }
  function openForwardDialog(r) {
    const f = $("#form-forward"); f.reset();
    f.elements.id.value = r?.id || ""; f.elements.label.value = r?.label || ""; f.elements.kind.value = r?.kind || "local";
    fillSelect(f.elements.host_id, hostsOf().map((h) => [h.id, h.label]), r?.host_id);
    f.elements.bind_address.value = r?.bind_address || "127.0.0.1"; f.elements.bind_port.value = r?.bind_port || "";
    f.elements.dest_host.value = r?.dest_host || "127.0.0.1"; f.elements.dest_port.value = r?.dest_port || "";
    applyFwdKind(); $("#dlg-forward").showModal();
  }
  function applyFwdKind() {
    const k = $("#form-forward").elements.kind.value;
    $("#fwd-dest").classList.toggle("hidden", k === "dynamic");
    $("#fwd-explain").textContent = t(`rm.fwdExplain${k[0].toUpperCase()}${k.slice(1)}`);
  }
  $("#form-forward").elements.kind.onchange = applyFwdKind;
  $("#form-forward").onsubmit = async (ev) => {
    ev.preventDefault(); const e = ev.target.elements;
    const rule = { id: e.id.value, label: e.label.value.trim(), kind: e.kind.value, host_id: e.host_id.value, bind_address: e.bind_address.value.trim() || "127.0.0.1", bind_port: Number(e.bind_port.value) || 0, dest_host: e.dest_host.value.trim(), dest_port: Number(e.dest_port.value) || 0 };
    try { await invoke("remote_save_forward", { rule }); $("#dlg-forward").close(); } catch (x) { fail(x); }
    refresh();
  };
  $("#btn-fwd-new").onclick = () => openForwardDialog(null);
  $("#fwd-list").addEventListener("click", async (ev) => {
    const e = ev.target.closest("[data-fwd-edit]"); if (e) { openForwardDialog(R.view.store.forwards.find((r) => r.id === e.dataset.fwdEdit)); return; }
    const d = ev.target.closest("[data-fwd-delete]");
    if (d && (await askConfirm(t("rm.confirmDeleteRule")))) { try { await invoke("remote_delete_forward", { id: d.dataset.fwdDelete }); } catch (x) { fail(x); } refresh(); }
  });
  $("#fwd-list").addEventListener("change", async (ev) => {
    const sw = ev.target.closest("[data-fwd-switch]"); if (!sw) return;
    const id = sw.dataset.fwdSwitch; sw.disabled = true;
    try {
      if (sw.checked) { const s = await invoke("forward_start", { id }); toast(t("rm.forwardOn", { port: s.bound_port }), "ok"); }
      else { await invoke("forward_stop", { id }); toast(t("rm.forwardOff"), "ok"); }
    } catch (x) { fail(x); }
    refresh();
  });
  setInterval(() => { if ((R.view?.forwards_active || []).length && $("#tab-forward").classList.contains("active")) refresh(); }, 5000);

  /* ---------------- snippets ---------------- */
  function renderSnippets() {
    if (!R.view) return;
    const snippets = R.view.store.snippets;
    const folders = [...new Set(snippets.map((s) => s.folder || ""))].sort((a, b) => (a === "") - (b === "") || a.localeCompare(b));
    const canSend = !!(R.active && R.terms.get(R.active) && !R.terms.get(R.active).closed);
    $("#snip-list").innerHTML = folders.map((fo) => `${fo ? `<h3 class="rm-folder">${ICONS.group}${esc(fo)}</h3>` : ""}<div class="list">${snippets.filter((s) => (s.folder || "") === fo).map((s) => `<div class="row" data-snip="${esc(s.id)}">
        <div class="ico">${ICONS.snippet}</div>
        <div><div class="name">${esc(s.label)}</div><div class="meta"><code class="snip-prev">${esc(s.script.split("\n")[0].slice(0, 120))}${s.script.includes("\n") ? " …" : ""}</code></div></div>
        <div class="act"><button class="btn small ${canSend ? "primary" : ""}" data-snip-send="${esc(s.id)}" ${canSend ? "" : "disabled"} title="${t("rm.sendToTerminal")}">${ICONS.send}<span>${t("rm.send")}</span></button>
          <button class="btn small" data-snip-run="${esc(s.id)}">${ICONS.play}<span>${t("rm.runOnHosts")}</span></button>
          <button class="icon-btn" data-snip-copy="${esc(s.id)}" title="${t("msg.copied")}">${ICONS.copy}</button>
          <button class="icon-btn" data-snip-edit="${esc(s.id)}" title="${t("btn.edit")}">${ICONS.edit}</button>
          <button class="icon-btn danger" data-snip-delete="${esc(s.id)}" title="${t("btn.remove")}">${ICONS.trash}</button></div></div>`).join("")}</div>`).join("")
      || `<div class="empty">${ICONS.snippet}<span>${t("rm.noSnippets")}</span></div>`;
  }
  function openSnippetDialog(s) {
    const f = $("#form-snippet"); f.reset();
    f.elements.id.value = s?.id || ""; f.elements.label.value = s?.label || ""; f.elements.folder.value = s?.folder || ""; f.elements.script.value = s?.script || "";
    $("#dlg-snippet").showModal(); f.elements.label.focus();
  }
  $("#form-snippet").onsubmit = async (ev) => {
    ev.preventDefault(); const e = ev.target.elements;
    try { await invoke("remote_save_snippet", { snippet: { id: e.id.value, label: e.label.value.trim(), folder: e.folder.value.trim(), script: e.script.value } }); $("#dlg-snippet").close(); } catch (x) { fail(x); }
    refresh();
  };
  $("#btn-snip-new").onclick = () => openSnippetDialog(null);
  function pickHosts(title, preselect) {
    return new Promise((resolve) => {
      const dlg = $("#dlg-run-snippet"), f = $("#form-run-snippet");
      $("#run-snippet-title").textContent = title;
      $("#run-snippet-hosts").innerHTML = hostsOf().map((h) => `<label class="check"><input type="checkbox" name="h" value="${esc(h.id)}" ${preselect?.includes(h.id) ? "checked" : ""}><span>${esc(h.label)} <small class="mono">${esc(hostSummary(h))}</small></span></label>`).join("") || `<div class="empty small">${t("rm.noHosts")}</div>`;
      const done = (v) => { dlg.close(); resolve(v); };
      f.onsubmit = (ev) => { ev.preventDefault(); done($$("input[name=h]:checked", f).map((i) => i.value)); };
      dlg.oncancel = (ev) => { ev.preventDefault(); done(null); };
      $("[data-close]", f).onclick = () => done(null);
      dlg.showModal();
    });
  }
  $("#snip-list").addEventListener("click", async (ev) => {
    const b = ev.target.closest("button"); if (!b) return;
    const sid = b.dataset.snipSend || b.dataset.snipRun || b.dataset.snipCopy || b.dataset.snipEdit || b.dataset.snipDelete;
    const s = R.view.store.snippets.find((x) => x.id === sid); if (!s) return;
    if (b.dataset.snipSend) { if (sendToTerminal(R.active, s.script)) { showTab("terminal"); } else toast(t("rm.noTerminal"), "info"); }
    else if (b.dataset.snipCopy) { await navigator.clipboard.writeText(s.script); toast(t("msg.copied"), "ok"); }
    else if (b.dataset.snipEdit) openSnippetDialog(s);
    else if (b.dataset.snipDelete) { if (await askConfirm(t("rm.confirmDeleteSnippet", { name: s.label }))) { try { await invoke("remote_delete_snippet", { id: s.id }); } catch (x) { fail(x); } refresh(); } }
    else if (b.dataset.snipRun) {
      const ids = await pickHosts(`${t("rm.runOnHosts")}: ${s.label}`); if (!ids?.length) return;
      const out = $("#snip-output"); out.classList.remove("hidden");
      out.innerHTML = `<div class="activity-head"><span>${esc(s.label)}</span><span class="state busy">${ICONS.spinner}${t("rm.running", { n: ids.length })}</span></div>`;
      try {
        const results = await invoke("snippet_run", { snippetId: s.id, script: null, hostIds: ids });
        out.innerHTML = `<div class="activity-head"><span>${esc(s.label)}</span><button class="btn small ghost" id="btn-snip-out-close">${t("btn.close")}</button></div>` + results.map((r) => `<div class="snip-result ${r.error || (r.exit && r.exit !== 0) ? "bad" : "ok"}"><div class="srh"><b>${esc(r.label)}</b><span class="pill">${r.error ? esc(r.error) : `exit ${r.exit ?? "?"}`}</span></div><pre>${esc((r.stdout + (r.stderr ? "\n" + r.stderr : "")).trim() || "—")}</pre></div>`).join("");
        $("#btn-snip-out-close").onclick = () => out.classList.add("hidden");
      } catch (x) { fail(x); out.classList.add("hidden"); }
    }
  });

  /* ---------------- keychain ---------------- */
  function renderKeychain() {
    if (!R.view) return;
    const { keys, identities } = R.view.store;
    $("#keychain-note").textContent = R.view.secret_backend === "keyring" ? t("rm.secretsKeyring") : t("rm.secretsFile");
    $("#key-list").innerHTML = keys.map((k) => `<div class="row" data-key="${esc(k.id)}">
        <div class="ico">${ICONS.key}</div>
        <div><div class="name">${esc(k.label)} ${k.encrypted ? `<span class="pill" title="${t("rm.passphraseProtected")}">${ICONS.lock}${k.has_passphrase ? t("rm.saved") : ""}</span>` : ""}</div><div class="meta"><span class="pill">${esc(k.algorithm)}</span><span class="mono">${esc(k.fingerprint)}</span></div></div>
        <div class="act"><button class="btn small" data-key-copy="${esc(k.id)}">${ICONS.copy}<span>${t("rm.copyPublic")}</span></button>
          <button class="btn small" data-key-deploy="${esc(k.id)}" title="${t("rm.deployHint")}">${ICONS.upload}<span>${t("rm.deploy")}</span></button>
          <button class="icon-btn" data-key-export="${esc(k.id)}" title="${t("rm.exportPublic")}">${ICONS.download}</button>
          <button class="icon-btn" data-key-export-private="${esc(k.id)}" title="${t("rm.exportPrivate")}">${ICONS.file}</button>
          <button class="icon-btn" data-key-pass="${esc(k.id)}" title="${t("rm.changePassphrase")}">${k.encrypted ? ICONS.lock : ICONS.unlock}</button>
          <button class="icon-btn danger" data-key-delete="${esc(k.id)}" title="${t("btn.remove")}">${ICONS.trash}</button></div></div>`).join("")
      || `<div class="empty">${ICONS.key}<span>${t("rm.noKeys")}</span></div>`;
    $("#ident-list").innerHTML = identities.map((i) => `<div class="row" data-ident="${esc(i.id)}">
        <div class="ico">${ICONS.identity}</div>
        <div><div class="name">${esc(i.label)}</div><div class="meta"><span class="mono">${esc(i.username)}</span>${i.has_password ? `<span class="pill">${t("db.password")}</span>` : ""}${i.key_id ? `<span class="pill">${ICONS.key}${esc(keys.find((k) => k.id === i.key_id)?.label || "?")}</span>` : ""}</div></div>
        <div class="act"><button class="icon-btn" data-ident-edit="${esc(i.id)}" title="${t("btn.edit")}">${ICONS.edit}</button><button class="icon-btn danger" data-ident-delete="${esc(i.id)}" title="${t("btn.remove")}">${ICONS.trash}</button></div></div>`).join("")
      || `<div class="empty">${ICONS.identity}<span>${t("rm.noIdentities")}</span></div>`;
  }
  async function openKeyDialog(mode) {
    const f = $("#form-key"); f.reset(); f.elements.mode.value = mode;
    $("#key-dlg-title").textContent = mode === "generate" ? t("rm.newKey") : t("rm.importKey");
    $$("#form-key [data-key]").forEach((el) => el.classList.toggle("hidden", el.dataset.key !== mode));
    if (mode === "import") {
      let cands = []; try { cands = await invoke("remote_key_candidates"); } catch (e) {}
      fillSelect(f.elements.path, cands.map((p) => [p, p]), "", t("rm.pasteOrPick"));
    }
    $("#dlg-key").showModal(); f.elements.label.focus();
  }
  $("#btn-key-new").onclick = () => openKeyDialog("generate");
  $("#btn-key-import").onclick = () => openKeyDialog("import");
  $("#btn-key-browse").onclick = async () => {
    try { const p = await window.__TAURI__.dialog.open({ multiple: false, title: t("rm.keyFile") }); if (p) { const f = $("#form-key"); const v = typeof p === "string" ? p : p.path; fillSelect(f.elements.path, [[v, v]], v); } } catch (e) { toast(t("migrate.noPicker"), "info"); }
  };
  $("#form-key").onsubmit = async (ev) => {
    ev.preventDefault(); const e = ev.target.elements;
    const args = { label: e.label.value.trim(), passphrase: e.passphrase.value || null, savePassphrase: e.save_passphrase.checked };
    try {
      if (e.mode.value === "generate") await invoke("remote_key_generate", { ...args, kind: e.kind.value });
      else await invoke("remote_key_import", { ...args, text: e.text.value || null, path: e.path.value || null });
      $("#dlg-key").close(); toast(t("msg.saved2"), "ok");
    } catch (x) { fail(x); }
    refresh();
  };
  function openIdentityDialog(i) {
    const f = $("#form-identity"); f.reset();
    f.elements.id.value = i?.id || ""; f.elements.label.value = i?.label || ""; f.elements.username.value = i?.username || "";
    f.elements.password.placeholder = i?.has_password ? "•••••••• (saved)" : "";
    fillSelect(f.elements.key_id, R.view.store.keys.map((k) => [k.id, k.label]), i?.key_id, "—");
    $("#dlg-identity").showModal(); f.elements.label.focus();
  }
  $("#btn-ident-new").onclick = () => openIdentityDialog(null);
  $("#form-identity").onsubmit = async (ev) => {
    ev.preventDefault(); const e = ev.target.elements;
    const identity = { id: e.id.value, label: e.label.value.trim(), username: e.username.value.trim(), key_id: e.key_id.value || null, has_password: false };
    try { await invoke("remote_save_identity", { identity, password: e.password.value || null }); $("#dlg-identity").close(); } catch (x) { fail(x); }
    refresh();
  };
  $("#key-list").addEventListener("click", async (ev) => {
    const b = ev.target.closest("button"); if (!b) return;
    const kid = b.dataset.keyCopy || b.dataset.keyDeploy || b.dataset.keyExport || b.dataset.keyExportPrivate || b.dataset.keyPass || b.dataset.keyDelete;
    const k = R.view.store.keys.find((x) => x.id === kid); if (!k) return;
    try {
      if (b.dataset.keyCopy) { await navigator.clipboard.writeText(k.public_key); toast(t("msg.copied"), "ok"); }
      else if (b.dataset.keyExport || b.dataset.keyExportPrivate) {
        const priv = !!b.dataset.keyExportPrivate;
        if (priv && !(await askConfirm(t("rm.exportPrivateWarn"), t("rm.export")))) return;
        const path = await window.__TAURI__.dialog.save({ defaultPath: `${k.label.replace(/[^\w.-]+/g, "_")}${priv ? "" : ".pub"}`, title: priv ? t("rm.exportPrivate") : t("rm.exportPublic") });
        if (!path) return;
        await invoke("remote_key_export", { id: k.id, path, private: priv }); toast(t("rm.exported", { path }), "ok");
      }
      else if (b.dataset.keyDeploy) {
        const ids = await pickHosts(`${t("rm.deploy")}: ${k.label}`); if (!ids?.length) return;
        for (const hid of ids) { try { toast(await invoke("remote_key_deploy", { hostId: hid, keyId: k.id }), "ok"); } catch (x) { fail(x); } }
      }
      else if (b.dataset.keyPass) {
        const current = k.encrypted && !k.has_passphrase ? await askText(t("rm.changePassphrase"), t("rm.currentPassphrase"), "", true) : null;
        if (k.encrypted && !k.has_passphrase && current === null) return;
        const next = await askText(t("rm.changePassphrase"), t("rm.newPassphrase"), "", true); if (next === null) return;
        await invoke("remote_key_passphrase", { id: k.id, current, new: next, savePassphrase: !!next }); toast(t("msg.saved2"), "ok"); refresh();
      }
      else if (b.dataset.keyDelete) { if (await askConfirm(t("rm.confirmDeleteKey", { name: k.label }))) { await invoke("remote_key_delete", { id: k.id }); refresh(); } }
    } catch (x) { fail(x); }
  });
  $("#ident-list").addEventListener("click", async (ev) => {
    const b = ev.target.closest("button"); if (!b) return;
    const i = R.view.store.identities.find((x) => x.id === (b.dataset.identEdit || b.dataset.identDelete)); if (!i) return;
    if (b.dataset.identEdit) openIdentityDialog(i);
    else if (await askConfirm(t("rm.confirmDeleteIdentity", { name: i.label }))) { try { await invoke("remote_delete_identity", { id: i.id }); } catch (x) { fail(x); } refresh(); }
  });

  /* ---------------- known hosts & history ---------------- */
  async function loadKnownHosts() {
    try {
      const list = await invoke("remote_known_hosts");
      $("#kh-list").innerHTML = list.map((k) => `<div class="row"><div class="ico">${ICONS.knownhosts}</div>
        <div><div class="name mono">${esc(k.hosts.startsWith("|1|") ? t("rm.hashedHost") : k.hosts)}</div><div class="meta"><span class="pill">${esc(k.algorithm)}</span><span class="mono">${esc(k.fingerprint)}</span></div></div>
        <div class="act"><button class="icon-btn danger" data-kh-remove="${k.line}" title="${t("btn.remove")}">${ICONS.trash}</button></div></div>`).join("") || `<div class="empty">${ICONS.knownhosts}<span>${t("rm.noKnown")}</span></div>`;
    } catch (e) { fail(e); }
  }
  $("#btn-kh-refresh").onclick = loadKnownHosts;
  $("#kh-list").addEventListener("click", async (ev) => {
    const b = ev.target.closest("[data-kh-remove]"); if (!b) return;
    if (await askConfirm(t("rm.confirmForgetHost"))) { try { await invoke("remote_known_host_remove", { line: Number(b.dataset.khRemove) }); } catch (x) { fail(x); } loadKnownHosts(); }
  });
  function renderHistory() {
    if (!R.view) return;
    $("#hist-list").innerHTML = R.view.store.history.slice(0, 200).map((h) => `<div class="row ${h.ok ? "" : "missing"}" data-hist="${esc(h.host_id || "")}">
        <div class="ico">${h.kind === "sftp" ? ICONS.sftp : h.kind === "forward" ? ICONS.forward : h.kind === "snippet" ? ICONS.snippet : ICONS.terminal}</div>
        <div><div class="name">${esc(h.label)} <span class="pill">${esc(h.kind)}</span></div><div class="meta"><span class="mono">${esc(h.target)}</span><span>${fmtTime(h.started_at)}</span>${h.ended_at ? `<span>· ${Math.max(1, Math.round((h.ended_at - h.started_at) / 60))} min</span>` : ""}${h.ok ? "" : `<span class="state warn"><i></i>${esc(h.error.split("\n")[0].slice(0, 120))}</span>`}</div></div>
        <div class="act">${h.host_id && hostById(h.host_id) ? `<button class="icon-btn go" data-hist-connect="${esc(h.host_id)}" title="${t("rm.openTerminal")}">${ICONS.terminal}</button>` : ""}</div></div>`).join("")
      || `<div class="empty">${ICONS.history}<span>${t("rm.noHistory")}</span></div>`;
  }
  $("#btn-hist-clear").onclick = async () => { if (await askConfirm(t("rm.confirmClearHistory"), t("btn.clear"))) { try { await invoke("remote_history_clear"); } catch (x) { fail(x); } refresh(); } };
  $("#hist-list").addEventListener("click", (ev) => { const b = ev.target.closest("[data-hist-connect]"); if (b) openTerminal({ hostId: b.dataset.histConnect }); });

  /* ---------------- import ---------------- */
  $("#btn-rm-import").onclick = async () => {
    let hosts = [];
    try { hosts = await invoke("remote_import_detect"); } catch (e) { fail(e); return; }
    if (!hosts.length) { toast(t("rm.nothingToImport"), "info"); return; }
    $("#import-list").innerHTML = hosts.map((h, i) => `<label class="check"><input type="checkbox" name="h" value="${i}" checked><span>${esc(h.alias)} <small class="mono">${esc(h.user ? h.user + "@" : "")}${esc(h.hostname)}:${h.port}</small> <small>${esc(h.source)}</small></span></label>`).join("");
    const f = $("#form-import");
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      const chosen = $$("input[name=h]:checked", f).map((i) => hosts[Number(i.value)]);
      try { const r = await invoke("remote_import_apply", { hosts: chosen, importKeys: f.elements.import_keys.checked }); toast(t("rm.imported", { hosts: r.hosts, keys: r.keys }), "ok"); r.skipped.forEach((s) => toast(s, "info")); $("#dlg-import").close(); } catch (x) { fail(x); }
      refresh();
    };
    $("#dlg-import").showModal();
  };

  /* ---------------- small dialogs ---------------- */
  function askText(title, label, value, secret) {
    return new Promise((resolve) => {
      const dlg = $("#dlg-text"), f = $("#form-text");
      $("#text-dlg-title").textContent = title; $("#text-dlg-label").textContent = label;
      f.elements.value.type = secret ? "password" : "text"; f.elements.value.value = value || "";
      const done = (v) => { dlg.close(); resolve(v); };
      f.onsubmit = (ev) => { ev.preventDefault(); done(f.elements.value.value.trim()); };
      dlg.oncancel = (ev) => { ev.preventDefault(); done(null); };
      $("[data-close]", f).onclick = () => done(null);
      dlg.showModal(); f.elements.value.focus(); f.elements.value.select();
    });
  }
  function askEditor(title, text) {
    return new Promise((resolve) => {
      const dlg = $("#dlg-editor"), f = $("#form-editor");
      $("#editor-title").textContent = title; f.elements.text.value = text;
      const done = (v) => { dlg.close(); resolve(v); };
      f.onsubmit = (ev) => { ev.preventDefault(); done(f.elements.text.value); };
      dlg.oncancel = (ev) => { ev.preventDefault(); done(null); };
      $("[data-close]", f).onclick = () => done(null);
      dlg.showModal(); f.elements.text.focus();
    });
  }
  document.addEventListener("click", (ev) => { const b = ev.target.closest("[data-close]"); if (b && !b.onclick) { const d = document.getElementById(b.dataset.close); if (d?.open) d.close(); } });


  /* ---------------- scripted check (VERANDA_UI_AUTOTEST=remote[:tab], dev only) ---------------- */
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  async function waitFor(cond, ms) { const end = Date.now() + ms; while (Date.now() < end) { if (cond()) return true; await sleep(100); } return cond(); }
  function bufferText(id) { const b = R.terms.get(id)?.term.buffer.active; if (!b) return ""; const lines = []; for (let i = 0; i < b.length; i++) lines.push(b.getLine(i)?.translateToString(true) || ""); return lines.join("\n"); }
  async function autotest(name, report) {
    const [base, tab] = name.split(":");
    if (base !== "remote") return false;
    const out = [];
    try {
      await refresh();
      let host = hostsOf().find((h) => h.label === "Refused box");
      if (!host) {
        const gid = await invoke("remote_save_group", { group: { id: "", name: "Lab", parent_id: null, identity_id: null, color: "" } });
        const kmeta = await invoke("remote_key_generate", { label: "autotest key", kind: "ed25519", passphrase: null, savePassphrase: false });
        const iid = await invoke("remote_save_identity", { identity: { id: "", label: "deploy", username: "deploy", has_password: false, key_id: kmeta.id }, password: null });
        const hid = await invoke("remote_save_host", { host: { id: "", label: "Refused box", address: "127.0.0.1", port: 1, username: "root", group_id: gid, tags: ["lab", "test"], color: "#dc2626", auth: "identity", identity_id: iid, notes: "nothing listens on port 1" }, password: null });
        await invoke("remote_save_host", { host: { id: "", label: "Example", address: "example.com", port: 22, username: "deploy", tags: ["prod"], color: "#16a34a", auth: "key", key_id: kmeta.id }, password: null });
        await invoke("remote_save_snippet", { snippet: { id: "", label: "Disk usage", folder: "Maintenance", script: "df -h" } });
        await invoke("remote_save_snippet", { snippet: { id: "", label: "Update packages", folder: "Maintenance", script: "sudo apt update && sudo apt upgrade -y" } });
        await invoke("remote_save_forward", { rule: { id: "", label: "DB tunnel", kind: "local", host_id: hid, bind_address: "127.0.0.1", bind_port: 33061, dest_host: "127.0.0.1", dest_port: 3306 } });
        await refresh();
        host = hostById(hid);
        out.push(`created fp=${kmeta.fingerprint.slice(0, 16)}`);
      }
      out.push(`hosts=${hostsOf().length} groups=${R.view.store.groups.length} keys=${R.view.store.keys.length} idents=${R.view.store.identities.length} snippets=${R.view.store.snippets.length} rules=${R.view.store.forwards.length} backend=${R.view.secret_backend}`);
      const tid = await openTerminal({ local: true });
      const up = await waitFor(() => R.terms.get(tid)?.info.state === "connected", 8000);
      await sleep(600);
      invoke("term_write", { id: tid, data: "echo VERANDA_OK_$((40+2))\n" });
      const echoed = await waitFor(() => bufferText(tid).includes("VERANDA_OK_42"), 8000);
      out.push(`local=${up ? "connected" : R.terms.get(tid)?.info.state} echo=${echoed}`);
      const tid2 = await openTerminal({ hostId: host.id });
      await waitFor(() => ["error", "closed", "connected"].includes(R.terms.get(tid2)?.info.state), 15000);
      out.push(`ssh=${R.terms.get(tid2)?.info.state} msg="${(R.terms.get(tid2)?.info.message || "").slice(0, 90)}"`);
      const rule = R.view.store.forwards[0];
      try { await invoke("forward_start", { id: rule.id }); out.push("forward=started?!"); } catch (e) { out.push(`forward=refused-ok(${String(e?.message || e).slice(0, 40)})`); }
      const l = await invoke("local_list", { path: null }); out.push(`local_list=${l.entries.length}`);
      const kh = await invoke("remote_known_hosts"); await refresh(); out.push(`known=${kh.length} history=${R.view.store.history.length} ok=${R.view.store.history.filter((h) => h.ok).length}`);
      if (tab === "sftp") { await loadPane("local", ""); }
      if (tab) { showTab(tab); if (tab === "terminal") activateTerminal(tid); }
    } catch (e) { out.push(`ERROR ${e?.message || e}`); }
    report(out.join(" | "));
    return true;
  }

  /* ---------------- boot ---------------- */
  window.VerandaRemote = {
    init: async () => { await refresh(); renderPane("local"); renderPane("remote"); if (state?.ui_tab === "terminal:local") openTerminal({ local: true }); },
    openTerminal, openLocal: () => openTerminal({ local: true }), refresh, autotest,
  };
})();
