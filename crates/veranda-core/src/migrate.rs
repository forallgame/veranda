//! One-click import from other local stacks (Laragon, XAMPP, FlyEnv, or any folder of projects).
//!
//! Sites: every project folder becomes a Veranda site, either linked in place (manual site that
//! points at the existing folder) or copied into the document root (auto site).
//! Databases: optional; the source server must be running. Each user database is streamed with
//! the installed `mysqldump` into Veranda's own server, so no data directory is ever touched.

use crate::Veranda;
use crate::config::DbEngine;
use crate::db::DbCtx;
use crate::packages::{Binaries, Registry};
use crate::{fsutil, sites};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub platform: String,
    pub label: String,
    /// Projects folder (Laragon `www`, XAMPP `htdocs`) or FlyEnv's `host.json`.
    pub path: Option<String>,
    pub found: bool,
    /// The application is installed here but holds no sites yet (FlyEnv without hosts).
    pub installed_at: Option<String>,
    pub db_port: u16,
    pub db_user: String,
    pub db_password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub platform: String,
    pub path: String,
    /// `link` (keep folders where they are) or `copy` (into the document root).
    pub mode: String,
    pub databases: bool,
    #[serde(default)]
    pub db_host: String,
    #[serde(default)]
    pub db_port: u16,
    #[serde(default)]
    pub db_user: String,
    #[serde(default)]
    pub db_password: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub sites: Vec<String>,
    pub databases: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

/// Folders XAMPP ships inside `htdocs` that are not user projects.
const XAMPP_BUILTIN: &[&str] = &["dashboard", "img", "webalizer", "xampp", "restricted"];
/// Server-side schemas that must never be copied between servers.
const SYSTEM_DBS: &[&str] = &["information_schema", "performance_schema", "mysql", "sys"];

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Known stacks with their default locations on this OS.
pub fn detect() -> Vec<Source> {
    let h = home();
    // Laragon (Windows only) and XAMPP: every drive letter plus the usual folders.
    let mut laragon: Vec<PathBuf> = Vec::new();
    let mut xampp: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for letter in b'C'..=b'Z' {
            let d = format!("{}:\\", letter as char);
            laragon.push(PathBuf::from(format!("{d}laragon\\www")));
            xampp.push(PathBuf::from(format!("{d}xampp\\htdocs")));
        }
        for base in [
            "C:\\Program Files",
            "C:\\Program Files (x86)",
            "C:\\ProgramData",
        ] {
            laragon.push(PathBuf::from(format!("{base}\\laragon\\www")));
            xampp.push(PathBuf::from(format!("{base}\\xampp\\htdocs")));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            laragon.push(PathBuf::from(&pf).join("laragon").join("www"));
            xampp.push(PathBuf::from(&pf).join("xampp").join("htdocs"));
        }
    }
    laragon.push(h.join("laragon").join("www"));
    laragon.push(h.join("Laragon").join("www"));
    xampp.push(PathBuf::from("/opt/lampp/htdocs"));
    xampp.push(h.join("lampp").join("htdocs"));
    xampp.push(h.join("xampp").join("htdocs"));
    xampp.push(PathBuf::from("/Applications/XAMPP/xamppfiles/htdocs"));
    xampp.push(PathBuf::from("/Applications/XAMPP/htdocs"));
    let flyenv_dirs = flyenv_config_dirs(&h);
    let flyenv_installed = flyenv_dirs.iter().find(|d| d.is_dir()).cloned();
    let flyenv = flyenv_installed
        .as_ref()
        .and_then(|d| flyenv_site_source(d));
    let mk = |platform: &str, label: &str, found: Option<PathBuf>, user: &str, pw: &str| Source {
        platform: platform.into(),
        label: label.into(),
        found: found.is_some(),
        path: found.map(|p| p.display().to_string()),
        installed_at: None,
        db_port: 3306,
        db_user: user.into(),
        db_password: pw.into(),
    };
    let mut fly = mk("flyenv", "FlyEnv", flyenv, "root", "root");
    fly.installed_at = flyenv_installed.map(|p| p.display().to_string());
    vec![
        mk("laragon", "Laragon", first_existing(&laragon), "root", ""),
        mk("xampp", "XAMPP", first_existing(&xampp), "root", ""),
        fly,
        mk("folder", "Other folder of projects", None, "root", ""),
    ]
}

/// Where FlyEnv (formerly PhpWebStudy) keeps its data on each platform.
fn flyenv_config_dirs(h: &Path) -> Vec<PathBuf> {
    let mut v = vec![
        h.join(".config").join("FlyEnv"),
        h.join("Library").join("FlyEnv"),
        h.join("Library").join("PhpWebStudy"),
        PathBuf::from("C:\\FlyEnv-Data"),
        PathBuf::from("C:\\Program Files\\FlyEnv-Data"),
    ];
    if let Some(d) = dirs::config_dir() {
        v.push(d.join("FlyEnv"));
    }
    v
}

/// The site list inside a FlyEnv folder: `host.json` when it exists, else the vhost folder when
/// it holds at least one `.conf`. `None` = FlyEnv has no sites yet.
fn flyenv_site_source(dir: &Path) -> Option<PathBuf> {
    for p in [dir.join("server").join("host.json"), dir.join("host.json")] {
        if p.is_file() {
            return Some(p);
        }
    }
    let vhost = dir.join("server").join("vhost");
    let has_conf = |d: &Path| {
        std::fs::read_dir(d)
            .map(|rd| {
                rd.flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "conf"))
            })
            .unwrap_or(false)
    };
    if has_conf(&vhost) || has_conf(&vhost.join("apache")) || has_conf(&vhost.join("nginx")) {
        return Some(vhost);
    }
    None
}

/// A project found in the source.
#[derive(Debug, Clone)]
struct Project {
    name: String,
    path: PathBuf,
    /// Hostname the source used, when it already fits Veranda's TLD.
    hostname: Option<String>,
}

fn projects_in_folder(dir: &Path, platform: &str) -> Result<Vec<Project>> {
    let mut out = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let Some(raw) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if raw.starts_with('.') || raw.starts_with('_') {
            continue;
        }
        if platform == "xampp" && XAMPP_BUILTIN.contains(&raw) {
            continue;
        }
        out.push(Project {
            name: raw.to_string(),
            path: p,
            hostname: None,
        });
    }
    Ok(out)
}

/// FlyEnv keeps its sites in `host.json`: `[{ "name": "demo.test", "root": "/path", ... }]`.
/// Accepts that file, the FlyEnv data folder, or its `server/vhost` folder (Apache/Nginx confs).
fn projects_in_flyenv(path: &Path) -> Result<Vec<Project>> {
    if path.is_dir() {
        let Some(src) = flyenv_site_source(path).or_else(|| {
            // a vhost folder itself
            path.file_name()
                .is_some_and(|n| n == "vhost")
                .then(|| path.to_path_buf())
        }) else {
            bail!(
                "FlyEnv at {} has no sites yet (no host.json and no vhost files); add hosts in FlyEnv first, or choose \"Other folder of projects\"",
                path.display()
            );
        };
        if src.is_dir() {
            return projects_in_vhost_confs(&src);
        }
        return projects_in_flyenv(&src);
    }
    let host_json = path;
    let text = std::fs::read_to_string(host_json)
        .with_context(|| format!("reading {}", host_json.display()))?;
    let hosts: Vec<serde_json::Value> = serde_json::from_str(text.trim()).context("host.json")?;
    let mut out = Vec::new();
    for h in hosts {
        let name = h.get("name").and_then(|v| v.as_str()).unwrap_or("").trim();
        let root = h.get("root").and_then(|v| v.as_str()).unwrap_or("").trim();
        if name.is_empty() || root.is_empty() {
            continue;
        }
        let path = PathBuf::from(root);
        // FlyEnv roots may point at `project/public`; the project itself is the parent then.
        let (path, name_hint) = if path.file_name().is_some_and(|f| f == "public")
            && path.parent().is_some_and(|p| p.is_dir())
        {
            (path.parent().unwrap().to_path_buf(), name)
        } else {
            (path, name)
        };
        out.push(Project {
            name: name_hint.split('.').next().unwrap_or(name_hint).to_string(),
            path,
            hostname: Some(name.to_ascii_lowercase()),
        });
    }
    Ok(out)
}

/// Sites from Apache (`ServerName` + `DocumentRoot`) or Nginx (`server_name` + `root`) vhost
/// files, one site per file, searched one level deep (`vhost/apache/*.conf`, `vhost/nginx/*.conf`).
fn projects_in_vhost_confs(dir: &Path) -> Result<Vec<Project>> {
    let mut files = Vec::new();
    for d in [dir.to_path_buf(), dir.join("apache"), dir.join("nginx")] {
        if let Ok(rd) = std::fs::read_dir(&d) {
            files.extend(
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "conf")),
            );
        }
    }
    files.sort();
    let mut out: Vec<Project> = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let mut name = None;
        let mut root = None;
        for line in text.lines() {
            let l = line.trim().trim_end_matches(';');
            let mut parts = l.split_whitespace();
            match parts.next() {
                Some("ServerName") | Some("server_name") if name.is_none() => {
                    name = parts
                        .next()
                        .map(|n| n.trim_matches('"').to_ascii_lowercase());
                }
                Some("DocumentRoot") | Some("root") if root.is_none() => {
                    root = parts.next().map(|r| r.trim_matches('"').to_string());
                }
                _ => {}
            }
        }
        let (Some(name), Some(root)) = (name, root) else {
            continue;
        };
        if name.is_empty()
            || root.is_empty()
            || out.iter().any(|p| p.hostname.as_deref() == Some(&name))
        {
            continue;
        }
        let path = PathBuf::from(&root);
        let path = if path.file_name().is_some_and(|f| f == "public")
            && path.parent().is_some_and(|p| p.is_dir())
        {
            path.parent().unwrap().to_path_buf()
        } else {
            path
        };
        out.push(Project {
            name: name.split('.').next().unwrap_or(&name).to_string(),
            path,
            hostname: Some(name),
        });
    }
    Ok(out)
}

fn copy_dir(src: &Path, dst: &Path, progress: &dyn Fn(String), count: &mut usize) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ty = e.file_type()?;
        let to = dst.join(e.file_name());
        if ty.is_symlink() {
            continue; // links are not followed: they usually point outside the project
        }
        if ty.is_dir() {
            copy_dir(&e.path(), &to, progress, count)?;
        } else {
            std::fs::copy(e.path(), &to)?;
            *count += 1;
            if (*count).is_multiple_of(250) {
                progress(format!("{} files copied…", count));
            }
        }
    }
    Ok(())
}

fn import_sites(
    app: &Veranda,
    req: &Request,
    report: &mut Report,
    progress: &dyn Fn(String),
) -> Result<()> {
    let src = PathBuf::from(req.path.trim());
    let projects = if req.platform == "flyenv" {
        projects_in_flyenv(&src)?
    } else {
        projects_in_folder(&src, &req.platform)?
    };
    if projects.is_empty() {
        bail!("no project folders found in {}", src.display());
    }
    let cfg = &app.config;
    let www = cfg.document_root(&app.layout);
    for p in projects {
        if !p.path.is_dir() {
            report.skipped.push(format!(
                "{}: folder not found ({})",
                p.name,
                p.path.display()
            ));
            continue;
        }
        let slug = fsutil::slugify(&p.name);
        if slug.is_empty() {
            report.skipped.push(format!("{}: unusable name", p.name));
            continue;
        }
        let hostname = match &p.hostname {
            Some(h) if crate::hosts::is_valid_hostname(h, &cfg.general.tld) => h.clone(),
            _ => cfg.hostname_for(&slug),
        };
        if req.mode == "copy" {
            let dest = www.join(&slug);
            if dest.exists() {
                report
                    .skipped
                    .push(format!("{}: already exists in {}", slug, www.display()));
                continue;
            }
            if p.path.starts_with(&www) {
                report
                    .skipped
                    .push(format!("{}: already inside the document root", slug));
                continue;
            }
            progress(format!("Copying {}…", p.name));
            let mut n = 0usize;
            match copy_dir(&p.path, &dest, progress, &mut n) {
                Ok(()) => report.sites.push(format!("{hostname} ({n} files)")),
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&dest);
                    report.errors.push(format!("{}: {e:#}", p.name));
                }
            }
        } else {
            progress(format!("Linking {}…", p.name));
            match sites::add_manual(&app.layout, cfg, &hostname, &p.path, "") {
                Ok(()) => report.sites.push(hostname),
                Err(e) => report.errors.push(format!("{}: {e:#}", p.name)),
            }
        }
    }
    Ok(())
}

fn source_client_args(engine: DbEngine, req: &Request) -> Vec<String> {
    let mut v = vec!["--no-defaults".to_string()];
    match engine {
        DbEngine::MariaDb => v.push("--skip-ssl".into()),
        DbEngine::MySql => v.push("--ssl-mode=DISABLED".into()),
    }
    let host = if req.db_host.trim().is_empty() {
        "127.0.0.1"
    } else {
        req.db_host.trim()
    };
    let port = if req.db_port == 0 { 3306 } else { req.db_port };
    let user = if req.db_user.trim().is_empty() {
        "root"
    } else {
        req.db_user.trim()
    };
    v.extend([
        "--protocol=tcp".into(),
        "-h".into(),
        host.into(),
        "-P".into(),
        port.to_string(),
        "-u".into(),
        user.into(),
    ]);
    v
}

fn list_source_databases(db: &DbCtx, req: &Request) -> Result<Vec<String>> {
    let mut cmd = Command::new(Binaries::db_client(&db.pkg, db.engine));
    cmd.args(source_client_args(db.engine, req))
        .args(["-N", "-B", "-e", "SHOW DATABASES"])
        .env("MYSQL_PWD", &req.db_password)
        .stdin(Stdio::null());
    let out = cmd.output().context("running the database client")?;
    if !out.status.success() {
        bail!(
            "cannot connect to the source database: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !SYSTEM_DBS.contains(&l.as_str()))
        .collect())
}

/// Stream one database from the source server straight into Veranda's server.
fn transfer_database(db: &DbCtx, req: &Request, name: &str, root_pw: &str) -> Result<()> {
    let mut dump = Command::new(Binaries::db_dump(&db.pkg, db.engine));
    dump.args(source_client_args(db.engine, req))
        .args([
            "--databases",
            name,
            "--single-transaction",
            "--skip-lock-tables",
            "--routines",
            "--events",
            "--triggers",
        ])
        .env("MYSQL_PWD", &req.db_password)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut dump = dump.spawn().context("starting mysqldump")?;
    let pipe = dump.stdout.take().context("dump pipe")?;
    let mut load = Command::new(Binaries::db_client(&db.pkg, db.engine));
    load.args(db.client_base_args())
        .args([
            "--protocol=tcp",
            "-h",
            "127.0.0.1",
            "-P",
            &db.port.to_string(),
            "-u",
            "root",
        ])
        .env("MYSQL_PWD", root_pw)
        .stdin(Stdio::from(pipe))
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let load = load.spawn().context("starting the database client")?;
    let load_out = load.wait_with_output()?;
    let dump_out = dump.wait_with_output()?;
    if !dump_out.status.success() {
        bail!(
            "dump failed: {}",
            String::from_utf8_lossy(&dump_out.stderr).trim()
        );
    }
    if !load_out.status.success() {
        bail!(
            "import failed: {}",
            String::from_utf8_lossy(&load_out.stderr).trim()
        );
    }
    Ok(())
}

fn import_databases(
    app: &Veranda,
    req: &Request,
    report: &mut Report,
    progress: &dyn Fn(String),
) -> Result<()> {
    let reg = Registry::load(&app.layout)?;
    let db = DbCtx::from_app(app, &reg).context("install and start Veranda's database first")?;
    if !crate::process::port_accepting(db.port) {
        bail!("start Veranda's database first (it receives the imported data)");
    }
    let host = req.db_host.trim();
    if (host.is_empty() || host == "127.0.0.1" || host == "localhost" || host == "::1")
        && req.db_port == db.port
    {
        bail!(
            "{host}:{} is Veranda's own database, not the source. Start the other stack's database server and enter its port here; if both use port {}, move Veranda's database to another port in Settings first",
            db.port,
            db.port
        );
    }
    let root_pw = crate::db::root_password(&app.layout)?.context("root password unknown")?;
    let names = list_source_databases(&db, req)?;
    if names.is_empty() {
        report
            .skipped
            .push("no user databases on the source server".into());
        return Ok(());
    }
    for name in names {
        progress(format!("Importing database {name}…"));
        match transfer_database(&db, req, &name, &root_pw) {
            Ok(()) => report.databases.push(name),
            Err(e) => report.errors.push(format!("{name}: {e:#}")),
        }
    }
    Ok(())
}

/// Run an import. Site errors never abort the database step and vice versa; everything is
/// collected in the report.
pub fn run(app: &Veranda, req: &Request, progress: &dyn Fn(String)) -> Result<Report> {
    if req.path.trim().is_empty() {
        bail!("choose the source folder first");
    }
    if !matches!(req.mode.as_str(), "link" | "copy") {
        bail!("mode must be link or copy");
    }
    let mut report = Report::default();
    if let Err(e) = import_sites(app, req, &mut report, progress) {
        report.errors.push(format!("sites: {e:#}"));
    }
    if req.databases
        && let Err(e) = import_databases(app, req, &mut report, progress)
    {
        report.errors.push(format!("databases: {e:#}"));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_projects_skip_hidden_and_builtin() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["blog", ".git", "_tmp", "dashboard", "shop"] {
            std::fs::create_dir_all(dir.path().join(n)).unwrap();
        }
        std::fs::write(dir.path().join("index.php"), "<?php").unwrap();
        let names: Vec<String> = projects_in_folder(dir.path(), "xampp")
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["blog", "shop"]);
        let all = projects_in_folder(dir.path(), "laragon").unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn flyenv_hosts_parse() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("demo");
        std::fs::create_dir_all(proj.join("public")).unwrap();
        // Built with serde so Windows paths (backslashes) are escaped correctly.
        let json = serde_json::json!([
            { "id": 1, "name": "demo.test", "root": proj.join("public").display().to_string(), "alias": "" },
            { "id": 2, "name": "", "root": "" }
        ]);
        let f = dir.path().join("host.json");
        std::fs::write(&f, serde_json::to_string(&json).unwrap()).unwrap();
        let p = projects_in_flyenv(&f).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "demo");
        assert_eq!(p[0].path, proj);
        assert_eq!(p[0].hostname.as_deref(), Some("demo.test"));
    }

    #[test]
    fn flyenv_dir_and_vhost_confs() {
        let dir = tempfile::tempdir().unwrap();
        let fly = dir.path().join("FlyEnv");
        let proj = dir.path().join("shop");
        std::fs::create_dir_all(proj.join("public")).unwrap();
        // installed but empty: a clear error, no sites
        std::fs::create_dir_all(fly.join("server").join("vhost").join("apache")).unwrap();
        assert!(flyenv_site_source(&fly).is_none());
        let e = projects_in_flyenv(&fly).unwrap_err().to_string();
        assert!(e.contains("no sites yet"), "{e}");
        // an apache vhost appears
        std::fs::write(
            fly.join("server/vhost/apache/shop.test.conf"),
            format!("<VirtualHost *:80>\n  ServerName shop.test\n  DocumentRoot \"{}\"\n</VirtualHost>\n", proj.join("public").display()),
        )
        .unwrap();
        // and an nginx one for the same host must not duplicate it
        std::fs::create_dir_all(fly.join("server/vhost/nginx")).unwrap();
        std::fs::write(
            fly.join("server/vhost/nginx/shop.test.conf"),
            format!(
                "server {{\n  server_name shop.test;\n  root {};\n}}\n",
                proj.join("public").display()
            ),
        )
        .unwrap();
        assert_eq!(
            flyenv_site_source(&fly).unwrap(),
            fly.join("server").join("vhost")
        );
        let p = projects_in_flyenv(&fly).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "shop");
        assert_eq!(p[0].path, proj);
        assert_eq!(p[0].hostname.as_deref(), Some("shop.test"));
        // host.json wins when present
        std::fs::write(fly.join("server/host.json"), "[]").unwrap();
        assert_eq!(
            flyenv_site_source(&fly).unwrap(),
            fly.join("server").join("host.json")
        );
    }

    #[test]
    fn link_import_adds_manual_sites() {
        let root = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("My App")).unwrap();
        let layout = crate::Layout::new(root.path());
        layout.ensure().unwrap();
        let app = Veranda {
            layout,
            config: crate::Config::default(),
        };
        let req = Request {
            platform: "folder".into(),
            path: src.path().display().to_string(),
            mode: "link".into(),
            databases: false,
            db_host: String::new(),
            db_port: 0,
            db_user: String::new(),
            db_password: String::new(),
        };
        let rep = run(&app, &req, &|_| {}).unwrap();
        assert_eq!(rep.sites, vec!["my-app.test"]);
        assert!(rep.errors.is_empty());
        let sites = sites::discover(&app.layout, &app.config).unwrap();
        assert!(sites.iter().any(|s| s.hostname == "my-app.test" && !s.auto));
    }
}
