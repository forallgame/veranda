//! Service lifecycle: web server, PHP FastCGI, database, cache and mail catcher.

pub mod cache;
pub mod db;
pub mod mail;
pub mod php;
pub mod web;

use crate::Padu;
use crate::packages::Registry;
use crate::sites::Site;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceId {
    Web,
    Php,
    Db,
    Cache,
    Mail,
}

impl ServiceId {
    pub const ALL: [ServiceId; 5] = [
        ServiceId::Web,
        ServiceId::Php,
        ServiceId::Db,
        ServiceId::Cache,
        ServiceId::Mail,
    ];

    pub const fn id(&self) -> &'static str {
        match self {
            ServiceId::Web => "web",
            ServiceId::Php => "php",
            ServiceId::Db => "db",
            ServiceId::Cache => "cache",
            ServiceId::Mail => "mail",
        }
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.id())
    }
}

impl std::str::FromStr for ServiceId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "web" | "caddy" | "nginx" | "apache" | "httpd" => Ok(ServiceId::Web),
            "php" | "php-fpm" => Ok(ServiceId::Php),
            "db" | "mysql" | "mariadb" | "database" => Ok(ServiceId::Db),
            "cache" | "redis" | "valkey" => Ok(ServiceId::Cache),
            "mail" | "mailpit" => Ok(ServiceId::Mail),
            other => bail!("unknown service `{other}` (web, php, db, cache, mail)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub id: ServiceId,
    /// Human name including the engine, e.g. `Caddy`, `MariaDB`.
    pub name: String,
    pub enabled: bool,
    pub installed: bool,
    pub version: String,
    pub running: bool,
    pub pids: Vec<u32>,
    pub ports: Vec<u16>,
    /// Hint for the UI (missing package, port conflict...).
    pub message: String,
}

/// Everything a service needs to prepare and start.
pub struct Ctx<'a> {
    pub app: &'a Padu,
    pub reg: &'a Registry,
    pub sites: &'a [Site],
}

impl<'a> Ctx<'a> {
    pub fn path_entries(&self) -> Vec<std::path::PathBuf> {
        crate::packages::runtime_path(&self.app.layout, self.reg, &self.app.config)
    }
}

/// Replace `{{KEY}}` placeholders. Unknown keys are left as-is (visible in the output).
pub fn render(tpl: &str, vars: &[(&str, String)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

pub const STOP_GRACE: Duration = Duration::from_secs(5);

fn enabled(app: &Padu, id: ServiceId) -> bool {
    let c = &app.config;
    if c.general.hidden_services.iter().any(|h| h == id.id()) {
        return false;
    }
    match id {
        ServiceId::Web => c.web.enabled,
        ServiceId::Php => c.php.enabled,
        ServiceId::Db => c.db.enabled,
        ServiceId::Cache => c.cache.enabled,
        ServiceId::Mail => c.mail.enabled,
    }
}

pub fn status(ctx: &Ctx, id: ServiceId) -> ServiceStatus {
    let mut st = match id {
        ServiceId::Web => web::status(ctx),
        ServiceId::Php => php::status(ctx),
        ServiceId::Db => db::status(ctx),
        ServiceId::Cache => cache::status(ctx),
        ServiceId::Mail => mail::status(ctx),
    };
    st.enabled = enabled(ctx.app, id);
    st
}

pub fn status_all(app: &Padu) -> Result<Vec<ServiceStatus>> {
    let reg = Registry::load(&app.layout)?;
    let sites = crate::sites::discover(&app.layout, &app.config)?;
    let ctx = Ctx {
        app,
        reg: &reg,
        sites: &sites,
    };
    Ok(ServiceId::ALL.iter().map(|id| status(&ctx, *id)).collect())
}

/// Regenerate every config file, the site certificate and (if possible) the hosts file.
/// Returns the list of hostnames that are missing from the hosts file.
pub fn prepare_all(app: &Padu) -> Result<Vec<String>> {
    let reg = Registry::load(&app.layout)?;
    let sites = crate::sites::discover(&app.layout, &app.config)?;
    let ctx = Ctx {
        app,
        reg: &reg,
        sites: &sites,
    };
    if app.config.web.ssl {
        crate::tls::ensure_site_cert(&app.layout, &crate::sites::hostnames(&sites))?;
    }
    php::prepare(&ctx)?;
    web::prepare(&ctx)?;
    db::prepare(&ctx)?;
    cache::prepare(&ctx)?;
    let hosts = crate::sites::hostnames(&sites);
    let hosts_file = app.platform().hosts_file();
    if crate::hosts::is_in_sync(&hosts_file, &hosts) {
        Ok(vec![])
    } else {
        Ok(hosts)
    }
}

pub fn start(app: &Padu, id: ServiceId) -> Result<()> {
    let reg = Registry::load(&app.layout)?;
    let sites = crate::sites::discover(&app.layout, &app.config)?;
    let ctx = Ctx {
        app,
        reg: &reg,
        sites: &sites,
    };
    if app.config.web.ssl && matches!(id, ServiceId::Web) {
        crate::tls::ensure_site_cert(&app.layout, &crate::sites::hostnames(&sites))?;
    }
    match id {
        ServiceId::Web => web::start(&ctx),
        ServiceId::Php => php::start(&ctx),
        ServiceId::Db => db::start(&ctx),
        ServiceId::Cache => cache::start(&ctx),
        ServiceId::Mail => mail::start(&ctx),
    }
}

pub fn stop(app: &Padu, id: ServiceId) -> Result<()> {
    let reg = Registry::load(&app.layout)?;
    let ctx = Ctx {
        app,
        reg: &reg,
        sites: &[],
    };
    match id {
        ServiceId::Web => web::stop(&ctx),
        ServiceId::Php => php::stop(&ctx),
        ServiceId::Db => db::stop(&ctx),
        ServiceId::Cache => cache::stop(&ctx),
        ServiceId::Mail => mail::stop(&ctx),
    }
}

pub fn restart(app: &Padu, id: ServiceId) -> Result<()> {
    stop(app, id)?;
    start(app, id)
}

/// Reload configuration (web server picks up new sites). Falls back to restart.
pub fn reload(app: &Padu, id: ServiceId) -> Result<()> {
    match id {
        ServiceId::Web => {
            let reg = Registry::load(&app.layout)?;
            let sites = crate::sites::discover(&app.layout, &app.config)?;
            let ctx = Ctx {
                app,
                reg: &reg,
                sites: &sites,
            };
            if app.config.web.ssl {
                crate::tls::ensure_site_cert(&app.layout, &crate::sites::hostnames(&sites))?;
            }
            web::reload(&ctx)
        }
        other => restart(app, other),
    }
}

/// Start every enabled service in dependency order (db, cache, mail, php, web).
/// Collects errors instead of stopping at the first one.
pub fn start_all(app: &Padu) -> Vec<(ServiceId, Result<()>)> {
    let order = [
        ServiceId::Db,
        ServiceId::Cache,
        ServiceId::Mail,
        ServiceId::Php,
        ServiceId::Web,
    ];
    order
        .iter()
        .filter(|id| enabled(app, **id))
        .map(|id| (*id, start(app, *id)))
        .collect()
}

pub fn stop_all(app: &Padu) -> Vec<(ServiceId, Result<()>)> {
    let order = [
        ServiceId::Web,
        ServiceId::Php,
        ServiceId::Mail,
        ServiceId::Cache,
        ServiceId::Db,
    ];
    order.iter().map(|id| (*id, stop(app, *id))).collect()
}

/// Error for a missing runtime with the exact command that fixes it.
pub fn not_installed(kind: crate::manifest::Kind) -> anyhow::Error {
    anyhow::anyhow!(
        "{} is not installed. Run: padu install {}",
        kind.display(),
        kind.id()
    )
}

/// Fail early with a useful message when a port is taken by someone else.
pub fn ensure_port_free(port: u16, what: &str) -> Result<()> {
    if crate::process::port_in_use(port) {
        let owner = crate::process::port_owner(port).unwrap_or_default();
        bail!("port {port} needed by {what} is already in use. {owner}");
    }
    Ok(())
}
