//! User configuration (`etc/padu.toml`). Every field has a safe default so a missing
//! or partial file still loads.

use crate::paths::Layout;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
#[derive(Default)]
pub struct Config {
    pub general: General,
    pub web: Web,
    pub php: Php,
    pub db: Db,
    pub cache: Cache,
    pub mail: Mail,
    pub backup: Backup,
    pub update: Update,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Reserved TLD (RFC 6761) used for auto virtual hosts.
    pub tld: String,
    /// `{name}` is replaced by the project folder name, `{tld}` by `tld`.
    pub hostname_format: String,
    /// Empty = `<root>/www`.
    pub document_root: String,
    /// Scan the document root and create a virtual host per folder.
    pub auto_vhosts: bool,
    /// If a project has `public/index.php`, serve `public/` (Laravel, Symfony...).
    pub smart_public_dir: bool,
    /// Start enabled services when the app launches.
    pub auto_start: bool,
    /// Start the app minimized to the tray.
    pub start_minimized: bool,
    /// Closing the window hides to the tray (true) or quits the app (false).
    pub close_to_tray: bool,
    /// Command for "Edit" actions, e.g. `code`. Empty = system default.
    pub editor: String,
    /// Command for "Open" actions. Empty = system default browser.
    pub browser: String,
    /// UI language code (`en`, `id`, ...).
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Web {
    pub enabled: bool,
    /// `caddy` (default, all platforms), `nginx`, `apache`.
    pub server: WebServer,
    pub http_port: u16,
    pub https_port: u16,
    /// Serve HTTPS with the local CA.
    pub ssl: bool,
    /// Installed version to use; empty = newest installed.
    pub version: String,
    /// Allow directory listings for folders without an index file (localhost only).
    pub autoindex: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WebServer {
    Caddy,
    Nginx,
    Apache,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Php {
    pub enabled: bool,
    /// Installed version to use; empty = newest installed.
    pub version: String,
    /// First FastCGI port; Windows spawns `workers` php-cgi processes on consecutive ports.
    pub fcgi_port: u16,
    pub workers: u16,
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub max_execution_time: u32,
    pub timezone: String,
    pub xdebug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Db {
    pub enabled: bool,
    /// `mariadb` or `mysql`.
    pub engine: DbEngine,
    pub version: String,
    pub port: u16,
    /// Empty = `<root>/data/<engine>`.
    pub data_dir: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DbEngine {
    MariaDb,
    MySql,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Cache {
    pub enabled: bool,
    pub version: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Mail {
    pub enabled: bool,
    pub version: String,
    pub smtp_port: u16,
    pub http_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Backup {
    pub enabled: bool,
    pub interval_hours: u32,
    pub keep: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Update {
    /// Check GitHub releases of this repository (`owner/repo`) for new app versions.
    pub repo: String,
    pub check_on_start: bool,
    /// Also look for newer PHP / server versions on start (network access).
    pub check_packages: bool,
    /// Install app updates automatically when found (otherwise just notify).
    pub auto_install: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            tld: "test".into(),
            hostname_format: "{name}.{tld}".into(),
            document_root: String::new(),
            auto_vhosts: true,
            smart_public_dir: true,
            auto_start: false,
            start_minimized: false,
            close_to_tray: true,
            editor: String::new(),
            browser: String::new(),
            language: "en".into(),
        }
    }
}

impl Default for Web {
    fn default() -> Self {
        Self {
            enabled: true,
            server: WebServer::Caddy,
            http_port: 80,
            https_port: 443,
            ssl: true,
            version: String::new(),
            autoindex: true,
        }
    }
}

impl Default for Php {
    fn default() -> Self {
        Self {
            enabled: true,
            version: String::new(),
            fcgi_port: 9000,
            workers: 4,
            memory_limit: "512M".into(),
            upload_max_filesize: "256M".into(),
            max_execution_time: 300,
            timezone: "UTC".into(),
            xdebug: false,
        }
    }
}

impl Default for Db {
    fn default() -> Self {
        Self {
            enabled: true,
            engine: DbEngine::MariaDb,
            version: String::new(),
            port: 3306,
            data_dir: String::new(),
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            enabled: false,
            version: String::new(),
            port: 6379,
        }
    }
}

impl Default for Mail {
    fn default() -> Self {
        Self {
            enabled: true,
            version: String::new(),
            smtp_port: 1025,
            http_port: 8025,
        }
    }
}

impl Default for Backup {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 8,
            keep: 5,
        }
    }
}

impl Default for Update {
    fn default() -> Self {
        Self {
            repo: String::new(),
            check_on_start: true,
            check_packages: false,
            auto_install: false,
        }
    }
}

impl Config {
    pub fn load_or_default(layout: &Layout) -> Result<Self> {
        let path = layout.config_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let cfg: Config =
                    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                cfg.validate()?;
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Config::default();
                cfg.save(layout)?;
                Ok(cfg)
            }
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self, layout: &Layout) -> Result<()> {
        self.validate()?;
        let text = toml::to_string_pretty(self)?;
        let header = "# Padu configuration. Edit and reload; unknown keys are rejected.\n\n";
        crate::fsutil::write_atomic(&layout.config_file(), format!("{header}{text}").as_bytes())
    }

    pub fn validate(&self) -> Result<()> {
        let tld = &self.general.tld;
        if tld.is_empty()
            || !tld
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            anyhow::bail!("general.tld must be lowercase letters/digits (e.g. `test`)");
        }
        if !self.general.hostname_format.contains("{name}") {
            anyhow::bail!("general.hostname_format must contain {{name}}");
        }
        if self.web.http_port == self.web.https_port {
            anyhow::bail!("web.http_port and web.https_port must differ");
        }
        if self.php.workers == 0 || self.php.workers > 64 {
            anyhow::bail!("php.workers must be between 1 and 64");
        }
        if self.backup.keep == 0 {
            anyhow::bail!("backup.keep must be at least 1");
        }
        if !self.update.repo.is_empty() && !is_repo_slug(&self.update.repo) {
            anyhow::bail!("update.repo must look like owner/repo");
        }
        Ok(())
    }

    pub fn document_root(&self, layout: &Layout) -> PathBuf {
        if self.general.document_root.trim().is_empty() {
            layout.www()
        } else {
            PathBuf::from(self.general.document_root.trim())
        }
    }

    pub fn db_data_dir(&self, layout: &Layout) -> PathBuf {
        if self.db.data_dir.trim().is_empty() {
            layout.data().join(self.db.engine.id())
        } else {
            PathBuf::from(self.db.data_dir.trim())
        }
    }

    /// Hostname for a project folder, e.g. `my-app` -> `my-app.test`.
    pub fn hostname_for(&self, name: &str) -> String {
        self.general
            .hostname_format
            .replace("{name}", name)
            .replace("{tld}", &self.general.tld)
    }
}

impl WebServer {
    pub const fn id(&self) -> &'static str {
        match self {
            WebServer::Caddy => "caddy",
            WebServer::Nginx => "nginx",
            WebServer::Apache => "apache",
        }
    }
}

impl DbEngine {
    pub const fn id(&self) -> &'static str {
        match self {
            DbEngine::MariaDb => "mariadb",
            DbEngine::MySql => "mysql",
        }
    }
}

pub fn is_repo_slug(s: &str) -> bool {
    let mut parts = s.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), None) => {
            let ok = |x: &str| {
                !x.is_empty()
                    && x.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            };
            ok(a) && ok(b)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_defaults() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
        assert_eq!(cfg.hostname_for("blog"), "blog.test");
    }

    #[test]
    fn partial_file_loads() {
        let cfg: Config = toml::from_str("[web]\nhttp_port = 8080\n").unwrap();
        assert_eq!(cfg.web.http_port, 8080);
        assert_eq!(cfg.web.https_port, 443);
    }

    #[test]
    fn unknown_key_rejected() {
        assert!(toml::from_str::<Config>("[web]\nfoo = 1\n").is_err());
    }
}
