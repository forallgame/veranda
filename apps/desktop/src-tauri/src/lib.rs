//! Tauri shell around `padu-core`: commands for the web UI, tray icon, background jobs.

use padu_core::manifest::{Kind, Manifest};
use padu_core::packages::{Installed, Registry};
use padu_core::services::{self, ServiceId, ServiceStatus};
use padu_core::sites::Site;
use padu_core::{Config, Padu, Platform};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

pub struct AppState {
    pub app: Mutex<Padu>,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

fn open_app(state: &State<'_, AppState>) -> CmdResult<Padu> {
    Ok(state
        .app
        .lock()
        .map_err(|_| "state poisoned".to_string())?
        .clone())
}

#[derive(Serialize)]
struct UiState {
    version: String,
    platform: String,
    root: String,
    config: Config,
    services: Vec<ServiceStatus>,
    sites: Vec<Site>,
    installed: Vec<Installed>,
    hosts_in_sync: bool,
    db_password_known: bool,
    scheme: String,
    base_url: String,
    templates: Vec<(String, String)>,
    /// Optional UI hints for automated screenshots: PADU_UI_TAB / PADU_UI_THEME.
    ui_tab: Option<String>,
    ui_theme: Option<String>,
}

#[tauri::command]
fn get_state(state: State<'_, AppState>) -> CmdResult<UiState> {
    let app = open_app(&state)?;
    let services = services::status_all(&app).map_err(err)?;
    let sites = padu_core::sites::discover(&app.layout, &app.config).map_err(err)?;
    let reg = Registry::load(&app.layout).map_err(err)?;
    let hosts_in_sync = padu_core::hosts::is_in_sync(
        &Platform::current().hosts_file(),
        &padu_core::sites::hostnames(&sites),
    );
    let db_password_known = padu_core::db::root_password(&app.layout)
        .ok()
        .flatten()
        .is_some();
    let scheme = if app.config.web.ssl { "https" } else { "http" };
    let port = if app.config.web.ssl {
        app.config.web.https_port
    } else {
        app.config.web.http_port
    };
    let default = (scheme == "https" && port == 443) || (scheme == "http" && port == 80);
    let base_url = if default {
        format!("{scheme}://localhost")
    } else {
        format!("{scheme}://localhost:{port}")
    };
    Ok(UiState {
        version: padu_core::VERSION.into(),
        platform: Platform::current().to_string(),
        root: app.layout.root().display().to_string(),
        config: app.config.clone(),
        services,
        sites,
        installed: reg.items,
        hosts_in_sync,
        db_password_known,
        scheme: scheme.into(),
        base_url,
        templates: padu_core::projects::TEMPLATES
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        ui_tab: std::env::var("PADU_UI_TAB").ok(),
        ui_theme: std::env::var("PADU_UI_THEME").ok(),
    })
}

#[tauri::command]
async fn service_action(state: State<'_, AppState>, id: String, action: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    let sid: ServiceId = id.parse().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "start" => services::start(&app, sid),
        "stop" => services::stop(&app, sid),
        "restart" => services::restart(&app, sid),
        "reload" => services::reload(&app, sid),
        other => anyhow::bail!("unknown action {other}"),
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[derive(Serialize)]
struct ActionReport {
    id: String,
    ok: bool,
    error: String,
}

fn to_report(results: Vec<(ServiceId, anyhow::Result<()>)>) -> Vec<ActionReport> {
    results
        .into_iter()
        .map(|(id, r)| ActionReport {
            id: id.to_string(),
            ok: r.is_ok(),
            error: r.err().map(|e| format!("{e:#}")).unwrap_or_default(),
        })
        .collect()
}

#[tauri::command]
async fn start_all(state: State<'_, AppState>) -> CmdResult<Vec<ActionReport>> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _ = services::prepare_all(&app);
        to_report(services::start_all(&app))
    })
    .await
    .map_err(err)
}

#[tauri::command]
async fn stop_all(state: State<'_, AppState>) -> CmdResult<Vec<ActionReport>> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || to_report(services::stop_all(&app)))
        .await
        .map_err(err)
}

#[derive(Clone, Serialize)]
struct InstallProgress {
    kind: String,
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
async fn install_package(
    handle: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    version: Option<String>,
    latest: bool,
) -> CmdResult<Installed> {
    let app = open_app(&state)?;
    let kind: Kind = kind.parse().map_err(err)?;
    let p = Platform::current();
    let builtin = Manifest::builtin().map_err(err)?;
    let version = version.filter(|v| !v.trim().is_empty());
    let pkg = match version {
        // Explicit version: built-in list first, then the upstream resolver (newer than built-in).
        Some(v) => match builtin
            .versions(kind, p)
            .into_iter()
            .find(|x| x.version == v)
            .cloned()
        {
            Some(pk) => pk,
            None => padu_core::resolve::versions(kind, p)
                .await
                .map_err(err)?
                .into_iter()
                .find(|x| x.version == v)
                .ok_or_else(|| {
                    format!(
                        "{} {v} is not available (built-in or upstream)",
                        kind.display()
                    )
                })?,
        },
        None if latest => padu_core::resolve::latest(kind, p).await.map_err(err)?,
        None => builtin
            .latest(kind, p)
            .cloned()
            .ok_or_else(|| format!("no {kind} package for {p}"))?,
    };
    let kind_id = kind.id().to_string();
    let h = handle.clone();
    let progress = move |pr: padu_core::download::Progress| {
        let _ = h.emit(
            "install-progress",
            InstallProgress {
                kind: kind_id.clone(),
                downloaded: pr.downloaded,
                total: pr.total,
            },
        );
    };
    padu_core::packages::install(&app.layout, &pkg, Some(&progress))
        .await
        .map_err(err)
}

#[tauri::command]
async fn upstream_versions() -> CmdResult<Vec<padu_core::manifest::Package>> {
    Ok(padu_core::resolve::versions_all(Platform::current())
        .await
        .packages)
}

#[tauri::command]
fn builtin_versions() -> CmdResult<Vec<padu_core::manifest::Package>> {
    let m = Manifest::builtin().map_err(err)?;
    let p = Platform::current();
    Ok(m.for_platform(p).cloned().collect())
}

#[tauri::command]
fn remove_package(state: State<'_, AppState>, kind: String, version: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    let kind: Kind = kind.parse().map_err(err)?;
    // Refuse to delete a version that a running service is using right now.
    let statuses = services::status_all(&app).map_err(err)?;
    let uses_kind = |id: ServiceId| -> bool {
        use padu_core::config::{DbEngine, WebServer};
        match id {
            ServiceId::Web => matches!(
                (app.config.web.server, kind),
                (WebServer::Caddy, Kind::Caddy)
                    | (WebServer::Nginx, Kind::Nginx)
                    | (WebServer::Apache, Kind::Apache)
            ),
            ServiceId::Php => kind == Kind::Php,
            ServiceId::Db => matches!(
                (app.config.db.engine, kind),
                (DbEngine::MariaDb, Kind::MariaDb) | (DbEngine::MySql, Kind::MySql)
            ),
            ServiceId::Cache => kind == Kind::Cache,
            ServiceId::Mail => kind == Kind::Mailpit,
        }
    };
    if let Some(st) = statuses
        .iter()
        .find(|st| st.running && st.version == version && uses_kind(st.id))
    {
        return Err(format!(
            "{} {} is running. Stop it first.",
            st.name, version
        ));
    }
    padu_core::packages::remove(&app.layout, kind, &version).map_err(err)
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, config: Config) -> CmdResult<()> {
    let mut guard = state.app.lock().map_err(|_| "state poisoned".to_string())?;
    config.validate().map_err(err)?;
    guard.config = config;
    guard.save_config().map_err(err)
}

#[derive(Serialize)]
struct SyncResult {
    missing_hosts: Vec<String>,
}

#[tauri::command]
async fn sync_sites(state: State<'_, AppState>) -> CmdResult<SyncResult> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<SyncResult> {
        let missing = services::prepare_all(&app)?;
        if services::status_all(&app)?
            .iter()
            .any(|s| s.id == ServiceId::Web && s.running)
        {
            services::reload(&app, ServiceId::Web)?;
        }
        Ok(SyncResult {
            missing_hosts: missing,
        })
    })
    .await
    .map_err(err)?
    .map_err(err)
}

/// Write the hosts file through an elevated copy of this executable.
#[tauri::command]
async fn hosts_sync(state: State<'_, AppState>) -> CmdResult<()> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        let sites = padu_core::sites::discover(&app.layout, &app.config)?;
        let hosts = padu_core::sites::hostnames(&sites);
        let hosts_file = Platform::current().hosts_file();
        if padu_core::hosts::is_in_sync(&hosts_file, &hosts) {
            return Ok(());
        }
        let payload = app.layout.tmp().join("hosts.payload");
        padu_core::hosts::write_payload(&payload, &hosts, &app.config.general.tld)?;
        let exe = padu_core::tools::current_exe()?;
        padu_core::tools::run_elevated(
            &exe,
            &[
                "--hosts-apply".into(),
                payload.to_string_lossy().into_owned(),
            ],
        )?;
        if !padu_core::hosts::is_in_sync(&hosts_file, &hosts) {
            anyhow::bail!("hosts file was not updated");
        }
        Ok(())
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
fn add_site(
    state: State<'_, AppState>,
    hostname: String,
    path: String,
    public: String,
) -> CmdResult<()> {
    let app = open_app(&state)?;
    padu_core::sites::add_manual(
        &app.layout,
        &app.config,
        &hostname,
        std::path::Path::new(&path),
        &public,
    )
    .map_err(err)
}

#[tauri::command]
fn remove_site(state: State<'_, AppState>, hostname: String) -> CmdResult<bool> {
    let app = open_app(&state)?;
    padu_core::sites::remove_manual(&app.layout, &hostname).map_err(err)
}

#[derive(Deserialize)]
struct NewProject {
    name: String,
    template: String,
    arg: String,
    create_db: bool,
}

#[tauri::command]
async fn create_project(state: State<'_, AppState>, req: NewProject) -> CmdResult<String> {
    let app = open_app(&state)?;
    use padu_core::projects::Template;
    let t = match req.template.as_str() {
        "blank" => Template::Blank,
        "wordpress" => Template::WordPress,
        "laravel" => Template::Laravel,
        "symfony" => Template::Symfony,
        "composer" => Template::Composer {
            package: req.arg.clone(),
        },
        "git" => Template::Git {
            url: req.arg.clone(),
        },
        other => return Err(format!("unknown template {other}")),
    };
    let path = padu_core::projects::create(&app, &req.name, &t, None)
        .await
        .map_err(err)?;
    if req.create_db {
        let reg = Registry::load(&app.layout).map_err(err)?;
        if let Some(db) = padu_core::db::DbCtx::from_app(&app, &reg) {
            let _ = padu_core::db::create_database(&db, &req.name.replace('-', "_"));
        }
    }
    Ok(path.display().to_string())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    padu_core::projects::delete(&app, &name).map_err(err)
}

fn db_ctx(app: &Padu) -> CmdResult<padu_core::db::DbCtx> {
    let reg = Registry::load(&app.layout).map_err(err)?;
    padu_core::db::DbCtx::from_app(app, &reg).ok_or_else(|| "database is not installed".to_string())
}

#[tauri::command]
async fn db_create(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    let db = db_ctx(&app)?;
    tauri::async_runtime::spawn_blocking(move || padu_core::db::create_database(&db, &name))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
async fn db_list(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let app = open_app(&state)?;
    let db = db_ctx(&app)?;
    tauri::async_runtime::spawn_blocking(move || padu_core::db::list_databases(&db))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
fn db_password(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let app = open_app(&state)?;
    padu_core::db::root_password(&app.layout).map_err(err)
}

#[tauri::command]
async fn db_set_password(state: State<'_, AppState>, new: Option<String>) -> CmdResult<String> {
    let app = open_app(&state)?;
    let db = db_ctx(&app)?;
    let current = padu_core::db::root_password(&app.layout).map_err(err)?;
    let new = new
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| padu_core::db::random_token(24));
    let n2 = new.clone();
    tauri::async_runtime::spawn_blocking(move || {
        padu_core::db::set_root_password(&db, current.as_deref(), &n2)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    Ok(new)
}

#[tauri::command]
async fn db_backup(state: State<'_, AppState>) -> CmdResult<String> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || padu_core::backup::run(&app))
        .await
        .map_err(err)?
        .map(|p| p.display().to_string())
        .map_err(err)
}

#[derive(Serialize)]
struct TrustReport {
    description: String,
    ok: bool,
    error: String,
}

#[tauri::command]
async fn ssl_trust(state: State<'_, AppState>) -> CmdResult<Vec<TrustReport>> {
    let app = open_app(&state)?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<TrustReport>> {
        padu_core::tls::ensure_ca(&app.layout)?;
        let mut out = Vec::new();
        for step in padu_core::tls::trust_commands(&app.layout) {
            let r = if step.needs_admin {
                padu_core::tools::run_elevated(std::path::Path::new(&step.program), &step.args)
            } else {
                std::process::Command::new(&step.program)
                    .args(&step.args)
                    .status()
                    .map_err(anyhow::Error::from)
                    .and_then(|s| {
                        if s.success() {
                            Ok(())
                        } else {
                            anyhow::bail!("exit {s}")
                        }
                    })
            };
            out.push(TrustReport {
                description: step.description,
                ok: r.is_ok(),
                error: r.err().map(|e| format!("{e:#}")).unwrap_or_default(),
            });
        }
        Ok(out)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
fn open_target(state: State<'_, AppState>, what: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    let cfg = &app.config;
    let st = get_state(state.clone())?;
    let base = st.base_url;
    match what.as_str() {
        "web" => padu_core::tools::open_url(cfg, &format!("{base}/")),
        "adminer" => padu_core::tools::open_url(cfg, &format!("{base}/adminer/")),
        "phpmyadmin" => padu_core::tools::open_url(cfg, &format!("{base}/phpmyadmin/")),
        "mail" => {
            padu_core::tools::open_url(cfg, &format!("http://127.0.0.1:{}/", cfg.mail.http_port))
        }
        "root" => padu_core::tools::open_path(app.layout.root()),
        "www" => padu_core::tools::open_path(&cfg.document_root(&app.layout)),
        "logs" => padu_core::tools::open_path(&app.layout.logs()),
        "config" => padu_core::tools::open_editor(cfg, &app.layout.config_file()),
        "phpini" => {
            padu_core::tools::open_editor(cfg, &padu_core::services::php::ini_path(&app.layout))
        }
        "terminal" => {
            let reg = Registry::load(&app.layout).map_err(err)?;
            let path = padu_core::packages::runtime_path(&app.layout, &reg, cfg);
            padu_core::tools::open_terminal(&cfg.document_root(&app.layout), &path)
        }
        s if s.starts_with("site:") => padu_core::tools::open_url(
            cfg,
            &format!(
                "{}://{}{}/",
                st.scheme,
                &s[5..],
                base.rsplit_once(':')
                    .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()))
                    .map(|(_, p)| format!(":{p}"))
                    .unwrap_or_default()
            ),
        ),
        s if s.starts_with("path:") => padu_core::tools::open_path(std::path::Path::new(&s[5..])),
        s if s.starts_with("edit:") => {
            padu_core::tools::open_editor(cfg, std::path::Path::new(&s[5..]))
        }
        other => Err(anyhow::anyhow!("unknown target {other}")),
    }
    .map_err(err)
}

#[tauri::command]
fn open_url_external(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    if !url.starts_with("https://github.com/") {
        return Err("only GitHub release pages are opened from here".into());
    }
    padu_core::tools::open_url(&app.config, &url).map_err(err)
}

#[tauri::command]
async fn check_update(state: State<'_, AppState>) -> CmdResult<Option<padu_core::update::Release>> {
    let app = open_app(&state)?;
    padu_core::update::check(&app.config.update.repo)
        .await
        .map_err(err)
}

#[tauri::command]
fn change_root(state: State<'_, AppState>, root: String) -> CmdResult<()> {
    let app = Padu::open_at(std::path::PathBuf::from(root)).map_err(err)?;
    app.layout.remember().map_err(err)?;
    let mut guard = state.app.lock().map_err(|_| "state poisoned".to_string())?;
    *guard = app;
    Ok(())
}

#[derive(Serialize)]
struct LogFile {
    name: String,
    size: u64,
    modified: String,
}

fn log_path(app: &Padu, name: &str) -> CmdResult<std::path::PathBuf> {
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name.starts_with('.')
        || !name.ends_with(".log")
    {
        return Err("invalid log name".into());
    }
    let p = app.layout.logs().join(name);
    if !p.is_file() {
        return Err(format!("{name} does not exist"));
    }
    Ok(p)
}

#[tauri::command]
fn list_logs(state: State<'_, AppState>) -> CmdResult<Vec<LogFile>> {
    let app = open_app(&state)?;
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(app.layout.logs()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".log") {
                continue;
            }
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default();
            v.push(LogFile {
                name,
                size: meta.len(),
                modified,
            });
        }
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(v)
}

/// Last `lines` lines of a log file (capped at 256 KiB read from the end).
#[tauri::command]
fn read_log(state: State<'_, AppState>, name: String, lines: usize) -> CmdResult<String> {
    use std::io::{Read, Seek, SeekFrom};
    let app = open_app(&state)?;
    let path = log_path(&app, &name)?;
    let mut f = std::fs::File::open(&path).map_err(err)?;
    let len = f.metadata().map_err(err)?.len();
    let cap: u64 = 256 * 1024;
    let start = len.saturating_sub(cap);
    f.seek(SeekFrom::Start(start)).map_err(err)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(err)?;
    let text = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = text.lines().collect();
    let n = lines.clamp(10, 2000);
    let tail = if all.len() > n {
        &all[all.len() - n..]
    } else {
        &all[..]
    };
    Ok(tail.join("\n"))
}

#[tauri::command]
fn clear_log(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let app = open_app(&state)?;
    let path = log_path(&app, &name)?;
    std::fs::write(path, b"").map_err(err)
}

#[derive(Serialize, Clone)]
struct UpdateInfo {
    version: String,
    notes: String,
    date: String,
    /// true when the updater can install it in place (signed bundle available for this platform)
    installable: bool,
    /// release page, for the manual fallback
    url: String,
}

fn updater_for(app: &AppHandle, core: &Padu) -> CmdResult<tauri_plugin_updater::Updater> {
    let repo = core.config.update.repo.trim();
    if !padu_core::config::is_repo_slug(repo) {
        return Err("update.repo is not configured (Settings > GitHub repo)".into());
    }
    let url: tauri::Url = format!("https://github.com/{repo}/releases/latest/download/latest.json")
        .parse()
        .map_err(err)?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(err)?
        .build()
        .map_err(err)
}

/// Check for a newer Padu. Tries the signed updater feed first, then the GitHub API as a
/// read-only fallback (e.g. Linux .deb installs, which cannot self-update).
#[tauri::command]
async fn update_check(
    handle: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<UpdateInfo>> {
    let core = open_app(&state)?;
    let repo = core.config.update.repo.trim().to_string();
    let page = format!("https://github.com/{repo}/releases/latest");
    if let Ok(updater) = updater_for(&handle, &core)
        && let Ok(found) = updater.check().await
    {
        return Ok(found.map(|u| UpdateInfo {
            version: u.version.clone(),
            notes: u.body.clone().unwrap_or_default(),
            date: u.date.map(|d| d.to_string()).unwrap_or_default(),
            installable: true,
            url: page.clone(),
        }));
    }
    match padu_core::update::check(&repo).await.map_err(err)? {
        Some(r) => Ok(Some(UpdateInfo {
            version: r.version,
            notes: r.notes,
            date: r.published_at,
            installable: false,
            url: r.url,
        })),
        None => Ok(None),
    }
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
    done: bool,
}

/// Download, verify (signature), install and restart.
#[tauri::command]
async fn update_install(handle: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let core = open_app(&state)?;
    let updater = updater_for(&handle, &core)?;
    let Some(update) = updater.check().await.map_err(err)? else {
        return Err("already up to date".into());
    };
    let h = handle.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = h.emit(
                    "update-progress",
                    UpdateProgress {
                        downloaded,
                        total,
                        done: false,
                    },
                );
            },
            || {},
        )
        .await
        .map_err(err)?;
    let _ = handle.emit(
        "update-progress",
        UpdateProgress {
            downloaded,
            total: Some(downloaded),
            done: true,
        },
    );
    // Stop services before the binary is swapped and the app relaunches.
    let _ = services::stop_all(&core);
    handle.restart();
}

/// Answer from the "hide or quit?" dialog. `remember` stores the choice in the config.
#[tauri::command]
fn close_decision(
    handle: AppHandle,
    state: State<'_, AppState>,
    action: String,
    remember: bool,
) -> CmdResult<()> {
    use padu_core::config::CloseAction;
    let chosen = match action.as_str() {
        "tray" => CloseAction::Tray,
        "quit" => CloseAction::Quit,
        other => return Err(format!("unknown close action {other}")),
    };
    if remember {
        let mut guard = state.app.lock().map_err(|_| "state poisoned".to_string())?;
        guard.config.general.close_action = chosen;
        guard.save_config().map_err(err)?;
    }
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.hide();
    }
    if chosen == CloseAction::Quit {
        quit_app(handle);
    }
    Ok(())
}

/// Stop services on a worker thread (never blocks the UI), then exit. Hard deadline of 15 s.
fn quit_app(app: AppHandle) {
    let state = app.state::<AppState>();
    let core = open_app(&state).ok();
    std::thread::spawn(move || {
        if let Some(core) = core {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = services::stop_all(&core);
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(std::time::Duration::from_secs(15));
        }
        app.exit(0);
    });
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .init();
    let core = Padu::open().expect("cannot open the Padu root");
    let _ = padu_core::tls::ensure_ca(&core.layout);
    let auto_start = core.config.general.auto_start;
    let start_minimized = core.config.general.start_minimized;

    tauri::Builder::default()
        // Must be registered first: a second launch focuses the running window instead of
        // starting another process (and another tray icon).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            app: Mutex::new(core),
        })
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Show Padu", true, None::<&str>)?;
            let start = MenuItem::with_id(app, "start_all", "Start All", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, "stop_all", "Stop All", true, None::<&str>)?;
            let web = MenuItem::with_id(app, "open_web", "Open localhost", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit (stops services)", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &show,
                    &PredefinedMenuItem::separator(app)?,
                    &start,
                    &stop,
                    &web,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("icon"))
                .tooltip("Padu")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let state = app.state::<AppState>();
                    match event.id.as_ref() {
                        "show" => show_main(app),
                        "start_all" => {
                            if let Ok(core) = open_app(&state) {
                                std::thread::spawn(move || {
                                    let _ = services::prepare_all(&core);
                                    let _ = services::start_all(&core);
                                });
                            }
                        }
                        "stop_all" => {
                            if let Ok(core) = open_app(&state) {
                                std::thread::spawn(move || services::stop_all(&core));
                            }
                        }
                        "open_web" => {
                            if let Ok(core) = open_app(&state) {
                                let scheme = if core.config.web.ssl { "https" } else { "http" };
                                let _ = padu_core::tools::open_url(
                                    &core.config,
                                    &format!("{scheme}://localhost/"),
                                );
                            }
                        }
                        "quit" => {
                            quit_app(app.clone());
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if start_minimized && let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
            let handle = app.handle().clone();
            if auto_start {
                let state = handle.state::<AppState>();
                if let Ok(core) = open_app(&state) {
                    std::thread::spawn(move || {
                        let _ = services::prepare_all(&core);
                        let _ = services::start_all(&core);
                        let _ = handle.emit("services-changed", ());
                    });
                }
            }
            // Background: update check on start (signed feed, falls back to the GitHub API).
            let handle_u = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                let state = handle_u.state::<AppState>();
                let Ok(core) = open_app(&state) else { return };
                if !core.config.update.check_on_start || core.config.update.repo.trim().is_empty() {
                    return;
                }
                if let Ok(Some(info)) = update_check(handle_u.clone(), state.clone()).await {
                    let auto = core.config.update.auto_install && info.installable;
                    let _ = handle_u.emit("update-available", info);
                    if auto {
                        let _ = handle_u.emit("update-auto", ());
                        if let Err(e) = update_install(handle_u.clone(), state.clone()).await {
                            let _ = handle_u.emit("update-error", e);
                        }
                    }
                }
            });
            // Background: periodic database backups.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                    let core = {
                        let state = handle.state::<AppState>();
                        open_app(&state).ok()
                    };
                    if let Some(core) = core
                        && padu_core::backup::is_due(&core)
                    {
                        let res = tauri::async_runtime::spawn_blocking(move || {
                            padu_core::backup::run(&core)
                        })
                        .await;
                        if let Ok(Ok(p)) = res {
                            let _ = handle.emit("backup-done", p.display().to_string());
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use padu_core::config::CloseAction;
                let action = window
                    .state::<AppState>()
                    .app
                    .lock()
                    .map(|a| a.config.general.close_action)
                    .unwrap_or(CloseAction::Ask);
                api.prevent_close();
                match action {
                    CloseAction::Tray => {
                        let _ = window.hide();
                    }
                    CloseAction::Quit => {
                        let _ = window.hide();
                        quit_app(window.app_handle().clone());
                    }
                    CloseAction::Ask => {
                        // The UI shows a dialog and answers through `close_decision`.
                        let _ = window.emit("close-requested", ());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            service_action,
            start_all,
            stop_all,
            install_package,
            upstream_versions,
            builtin_versions,
            remove_package,
            save_config,
            sync_sites,
            hosts_sync,
            add_site,
            remove_site,
            create_project,
            delete_project,
            db_create,
            db_list,
            db_password,
            db_set_password,
            db_backup,
            ssl_trust,
            open_target,
            check_update,
            change_root,
            list_logs,
            read_log,
            clear_log,
            update_check,
            update_install,
            open_url_external,
            close_decision
        ])
        .run(tauri::generate_context!())
        .expect("error while running Padu");
}
