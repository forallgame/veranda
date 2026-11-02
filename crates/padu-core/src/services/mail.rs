//! Mailpit: catches every email sent by PHP and shows it in a web UI.

use super::{Ctx, STOP_GRACE, ServiceId, ServiceStatus};
use crate::manifest::Kind;
use crate::packages::Binaries;
use crate::process;
use anyhow::{Result, bail};

const PID: &str = "mail";

pub fn status(ctx: &Ctx) -> ServiceStatus {
    let pkg = ctx.reg.active(Kind::Mailpit, &ctx.app.config.mail.version);
    let pid = process::running_pid(&ctx.app.layout, PID);
    ServiceStatus {
        id: ServiceId::Mail,
        name: "Mailpit".into(),
        enabled: true,
        installed: pkg.is_some(),
        version: pkg.map(|p| p.version.clone()).unwrap_or_default(),
        running: pid.is_some(),
        pids: pid.into_iter().collect(),
        ports: vec![ctx.app.config.mail.smtp_port, ctx.app.config.mail.http_port],
        message: if pkg.is_none() {
            "not installed".into()
        } else {
            String::new()
        },
    }
}

pub fn start(ctx: &Ctx) -> Result<()> {
    let layout = &ctx.app.layout;
    let cfg = &ctx.app.config.mail;
    let pkg = ctx
        .reg
        .active(Kind::Mailpit, &cfg.version)
        .ok_or_else(|| super::not_installed(Kind::Mailpit))?;
    if process::running_pid(layout, PID).is_some() {
        return Ok(());
    }
    super::ensure_port_free(cfg.smtp_port, "Mailpit SMTP")?;
    super::ensure_port_free(cfg.http_port, "Mailpit web UI")?;
    let data = layout.data().join("mailpit");
    std::fs::create_dir_all(&data)?;
    let mut cmd = process::command(&Binaries::mailpit(pkg), &[]);
    cmd.args(["--listen", &format!("127.0.0.1:{}", cfg.http_port)])
        .args(["--smtp", &format!("127.0.0.1:{}", cfg.smtp_port)])
        .arg("--database")
        .arg(data.join("mailpit.db"))
        .args(["--max", "2000"])
        .current_dir(&data);
    process::spawn_detached(layout, PID, cmd)?;
    if !process::wait_for_port(cfg.http_port, std::time::Duration::from_secs(10)) {
        bail!("Mailpit did not start. See logs/mail.log");
    }
    Ok(())
}

pub fn stop(ctx: &Ctx) -> Result<()> {
    process::stop_named(&ctx.app.layout, PID, STOP_GRACE)
}
