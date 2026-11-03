//! MariaDB / MySQL server lifecycle (initialisation, root password, start, stop).

use super::{Ctx, STOP_GRACE, ServiceId, ServiceStatus, render};
use crate::config::DbEngine;
use crate::db::DbCtx;
use crate::manifest::Kind;
use crate::paths::Layout;
use crate::process;
use anyhow::{Context, Result, bail};

const PID: &str = "db";
const MY_CNF: &str = include_str!("../templates/my.cnf.tpl");

pub fn kind(engine: DbEngine) -> Kind {
    match engine {
        DbEngine::MariaDb => Kind::MariaDb,
        DbEngine::MySql => Kind::MySql,
    }
}

pub fn conf_path(layout: &Layout, engine: DbEngine) -> std::path::PathBuf {
    layout.etc().join(engine.id()).join("my.cnf")
}

/// Unix socket path. `sun_path` is limited to ~107 bytes, so very long roots fall back to the
/// system temp directory with a name that includes the port to stay unique.
pub fn socket_path(layout: &Layout, db: &DbCtx) -> std::path::PathBuf {
    let preferred = layout.run().join(format!("{}.sock", db.engine.id()));
    if preferred.as_os_str().len() <= 100 {
        preferred
    } else {
        std::env::temp_dir().join(format!("veranda-{}-{}.sock", db.engine.id(), db.port))
    }
}

/// Marker written once the data directory is initialised *and* the root password is set.
fn ready_marker(db: &DbCtx) -> std::path::PathBuf {
    db.data_dir.join(".veranda-ready")
}

pub fn prepare(ctx: &Ctx) -> Result<()> {
    let Some(db) = DbCtx::from_app(ctx.app, ctx.reg) else {
        return Ok(());
    };
    let layout = &ctx.app.layout;
    let conf = conf_path(layout, db.engine);
    std::fs::create_dir_all(conf.parent().unwrap())?;
    std::fs::create_dir_all(&db.data_dir)?;
    let windows = ctx.app.platform().is_windows();
    let socket = socket_path(layout, &db);
    let (client_socket, server_socket) = if windows {
        (String::new(), String::new())
    } else {
        (
            format!("socket = \"{}\"", Layout::slashes(&socket)),
            format!("socket = \"{}\"", Layout::slashes(&socket)),
        )
    };
    let extra_security = match db.engine {
        DbEngine::MariaDb => "".to_string(),
        DbEngine::MySql => "mysqlx = OFF\nskip-log-bin".to_string(),
    };
    let text = render(
        MY_CNF,
        &[
            ("PORT", db.port.to_string()),
            ("CLIENT_SOCKET", client_socket),
            ("BASEDIR", Layout::slashes(&db.pkg.path)),
            ("DATADIR", Layout::slashes(&db.data_dir)),
            ("TMP_DIR", Layout::slashes(&layout.tmp())),
            ("SERVER_SOCKET", server_socket),
            (
                "PID_FILE",
                format!(
                    "pid-file = \"{}/{}.internal.pid\"",
                    Layout::slashes(&layout.run()),
                    db.engine.id()
                ),
            ),
            ("LOG_DIR", Layout::slashes(&layout.logs())),
            ("ENGINE", db.engine.id().to_string()),
            ("EXTRA_SECURITY", extra_security),
            (
                "FLUSH_METHOD",
                if windows {
                    "unbuffered".into()
                } else {
                    "fsync".into()
                },
            ),
        ],
    );
    crate::fsutil::write_if_changed(&conf, &text)?;
    Ok(())
}

pub fn status(ctx: &Ctx) -> ServiceStatus {
    let db = DbCtx::from_app(ctx.app, ctx.reg);
    let pid = process::running_pid(&ctx.app.layout, PID);
    let name = match ctx.app.config.db.engine {
        DbEngine::MariaDb => "MariaDB",
        DbEngine::MySql => "MySQL",
    };
    let running = pid.is_some()
        || (db.is_some() && process::port_accepting(ctx.app.config.db.port) && pid.is_some());
    ServiceStatus {
        id: ServiceId::Db,
        name: name.into(),
        enabled: true,
        installed: db.is_some(),
        version: db
            .as_ref()
            .map(|d| d.pkg.version.clone())
            .unwrap_or_default(),
        running,
        pids: pid.into_iter().collect(),
        ports: vec![ctx.app.config.db.port],
        message: if db.is_none() {
            "not installed".into()
        } else {
            String::new()
        },
    }
}

/// Generic Linux tarballs of MySQL/MariaDB link `libaio.so.1`. Debian/Ubuntu 24.04+ ship it as
/// `libaio.so.1t64`, which the loader does not match, so the server dies with exit 127.
/// The binaries carry `RUNPATH=$ORIGIN/../lib/private`; dropping a symlink there fixes it
/// without touching the system. Missing entirely => a clear install hint.
#[cfg(target_os = "linux")]
pub fn ensure_runtime_libs(pkg_dir: &std::path::Path) -> Result<()> {
    const DIRS: [&str; 6] = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib/x86_64-linux-gnu",
        "/lib64",
    ];
    let find = |name: &str| {
        DIRS.iter()
            .map(|d| std::path::Path::new(d).join(name))
            .find(|p| p.exists())
    };
    let private = pkg_dir.join("lib").join("private");
    let shim = private.join("libaio.so.1");
    if shim.exists() || find("libaio.so.1").is_some() {
        return Ok(());
    }
    match find("libaio.so.1t64") {
        Some(sys) => {
            std::fs::create_dir_all(&private)?;
            std::os::unix::fs::symlink(&sys, &shim)
                .with_context(|| format!("linking {} -> {}", shim.display(), sys.display()))?;
            Ok(())
        }
        None => bail!(
            "the database server needs the libaio library. Install it and try again: \
             `sudo apt install libaio1t64` (Debian/Ubuntu), `sudo dnf install libaio` (Fedora), \
             `sudo pacman -S libaio` (Arch)"
        ),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_runtime_libs(_pkg_dir: &std::path::Path) -> Result<()> {
    Ok(())
}

pub fn start(ctx: &Ctx) -> Result<()> {
    let db = DbCtx::from_app(ctx.app, ctx.reg)
        .ok_or_else(|| super::not_installed(kind(ctx.app.config.db.engine)))?;
    let layout = &ctx.app.layout;
    prepare(ctx)?;
    if process::running_pid(layout, PID).is_some() {
        return Ok(());
    }
    process::clear_pid(layout, PID);
    super::ensure_port_free(db.port, "the database")?;
    ensure_runtime_libs(&db.pkg.path)?;
    let needs_setup = !ready_marker(&db).exists();
    if !db.is_initialized() {
        crate::db::initialize(&db).context("initialising the database directory")?;
    }
    let server = db.server_binary();
    if !server.is_file() {
        bail!("database server binary not found: {}", server.display());
    }
    let mut cmd = process::command(&server, &ctx.path_entries());
    cmd.arg(format!(
        "--defaults-file={}",
        conf_path(layout, db.engine).display()
    ))
    .current_dir(&db.pkg.path);
    if ctx.app.platform().is_windows() {
        cmd.arg("--console");
    }
    process::spawn_detached(layout, PID, cmd)?;
    if !process::wait_for_port(db.port, std::time::Duration::from_secs(60)) {
        bail!(
            "database did not start listening on port {}. See logs/db.log and logs/{}.log",
            db.port,
            db.engine.id()
        );
    }
    if needs_setup {
        // A start that failed half-way (after init, before the password) is retried here:
        // `set_root_password` first tries an empty password, then the stored one.
        let pw = crate::db::ensure_root_password_file(layout)?;
        crate::db::set_root_password(&db, None, &pw)
            .context("setting the initial root password")?;
        std::fs::write(ready_marker(&db), b"ok\n")?;
    }
    Ok(())
}

pub fn stop(ctx: &Ctx) -> Result<()> {
    let layout = &ctx.app.layout;
    if let (Some(db), Some(pid)) = (
        DbCtx::from_app(ctx.app, ctx.reg),
        process::running_pid(layout, PID),
    ) {
        // Clean shutdown flushes InnoDB; fall back to a signal if the password is unknown.
        if let Ok(Some(pw)) = crate::db::root_password(layout) {
            let _ = crate::db::shutdown(&db, &pw);
            let start = std::time::Instant::now();
            while start.elapsed() < STOP_GRACE && process::is_pid_alive(pid) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
    process::stop_named(layout, PID, std::time::Duration::from_secs(12))
}
