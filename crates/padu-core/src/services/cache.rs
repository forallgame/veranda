//! Valkey (Linux) / Redis for Windows / system valkey or redis on macOS.

use super::{Ctx, STOP_GRACE, ServiceId, ServiceStatus, render};
use crate::manifest::Kind;
use crate::packages::Binaries;
use crate::paths::Layout;
use crate::process;
use anyhow::{Result, bail};
use std::path::PathBuf;

const PID: &str = "cache";
const TPL: &str = include_str!("../templates/cache.conf.tpl");

fn binary(ctx: &Ctx) -> Option<(PathBuf, String)> {
    if let Some(i) = ctx.reg.active(Kind::Cache, &ctx.app.config.cache.version) {
        return Some((Binaries::cache_server(i), i.version.clone()));
    }
    if !ctx.app.platform().is_windows() {
        for n in ["valkey-server", "redis-server"] {
            if let Ok(p) = which::which(n) {
                return Some((p, "system".into()));
            }
            for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
                let p = PathBuf::from(dir).join(n);
                if p.is_file() {
                    return Some((p, "system".into()));
                }
            }
        }
    }
    None
}

pub fn conf_path(layout: &Layout) -> PathBuf {
    layout.etc().join("cache").join("cache.conf")
}

pub fn prepare(ctx: &Ctx) -> Result<()> {
    let layout = &ctx.app.layout;
    let conf = conf_path(layout);
    std::fs::create_dir_all(conf.parent().unwrap())?;
    let data = layout.data().join("cache");
    std::fs::create_dir_all(&data)?;
    let text = render(
        TPL,
        &[
            ("PORT", ctx.app.config.cache.port.to_string()),
            ("DATA_DIR", Layout::slashes(&data)),
            (
                "LOG_FILE",
                Layout::slashes(&layout.logs().join("cache-server.log")),
            ),
        ],
    );
    crate::fsutil::write_if_changed(&conf, &text)?;
    Ok(())
}

pub fn status(ctx: &Ctx) -> ServiceStatus {
    let bin = binary(ctx);
    let pid = process::running_pid(&ctx.app.layout, PID);
    let msg = if bin.is_none() {
        if ctx.app.platform().is_macos() {
            "not installed (brew install valkey)".into()
        } else {
            "not installed".into()
        }
    } else {
        String::new()
    };
    ServiceStatus {
        id: ServiceId::Cache,
        name: if ctx.app.platform().is_windows() {
            "Redis".into()
        } else {
            "Valkey".into()
        },
        enabled: true,
        installed: bin.is_some(),
        version: bin.map(|b| b.1).unwrap_or_default(),
        running: pid.is_some(),
        pids: pid.into_iter().collect(),
        ports: vec![ctx.app.config.cache.port],
        message: msg,
    }
}

pub fn start(ctx: &Ctx) -> Result<()> {
    let layout = &ctx.app.layout;
    let Some((bin, _)) = binary(ctx) else {
        if ctx.app.platform().is_macos() {
            bail!("Valkey is not installed. Run: brew install valkey");
        }
        return Err(super::not_installed(Kind::Cache));
    };
    prepare(ctx)?;
    if process::running_pid(layout, PID).is_some() {
        return Ok(());
    }
    super::ensure_port_free(ctx.app.config.cache.port, "the cache server")?;
    let mut cmd = process::command(&bin, &ctx.path_entries());
    cmd.arg(conf_path(layout))
        .current_dir(layout.data().join("cache"));
    process::spawn_detached(layout, PID, cmd)?;
    if !process::wait_for_port(
        ctx.app.config.cache.port,
        std::time::Duration::from_secs(10),
    ) {
        bail!("cache server did not start. See logs/cache.log");
    }
    Ok(())
}

pub fn stop(ctx: &Ctx) -> Result<()> {
    process::stop_named(&ctx.app.layout, PID, STOP_GRACE)
}
