//! Directory layout of a Padu installation root.
//!
//! ```text
//! <root>/
//!   bin/<kind>/<version>/   downloaded runtimes (php, caddy, mariadb, ...)
//!   etc/                    generated configuration (padu.toml, Caddyfile, php.ini, ...)
//!   etc/ssl/                local CA and site certificates
//!   etc/secrets/            generated passwords (0600)
//!   etc/apps/               web tools served under http://localhost/<tool> (adminer, phpmyadmin)
//!   data/                   database data directories
//!   www/                    document root; each sub folder becomes <name>.test
//!   logs/                   service logs
//!   run/                    pid files
//!   tmp/                    downloads and scratch space
//!   backups/                database dumps
//!   usr/                    user customisations (extra services, startup scripts)
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub const ROOT_ENV: &str = "PADU_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default root: `<home>/padu` on every platform (no spaces, user-writable).
    pub fn default_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("padu")
    }

    /// Small pointer file in the per-user config dir that remembers a custom root.
    pub fn pointer_file() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("padu").join("root"))
    }

    /// Resolution order: `$PADU_HOME`, pointer file, default root.
    pub fn discover() -> Result<Self> {
        if let Ok(v) = std::env::var(ROOT_ENV)
            && !v.trim().is_empty()
        {
            return Ok(Self::new(PathBuf::from(v.trim())));
        }
        if let Some(p) = Self::pointer_file()
            && let Ok(s) = std::fs::read_to_string(&p)
        {
            let s = s.trim();
            if !s.is_empty() {
                return Ok(Self::new(PathBuf::from(s)));
            }
        }
        Ok(Self::new(Self::default_root()))
    }

    /// Persist a custom root so later invocations find it.
    pub fn remember(&self) -> Result<()> {
        if let Some(p) = Self::pointer_file() {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::fsutil::write_atomic(&p, self.root.to_string_lossy().as_bytes())?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn bin_kind(&self, kind: &str) -> PathBuf {
        self.bin().join(kind)
    }
    pub fn etc(&self) -> PathBuf {
        self.root.join("etc")
    }
    pub fn ssl(&self) -> PathBuf {
        self.etc().join("ssl")
    }
    pub fn secrets(&self) -> PathBuf {
        self.etc().join("secrets")
    }
    pub fn apps(&self) -> PathBuf {
        self.etc().join("apps")
    }
    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }
    pub fn www(&self) -> PathBuf {
        self.root.join("www")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn run(&self) -> PathBuf {
        self.root.join("run")
    }
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn downloads(&self) -> PathBuf {
        self.tmp().join("downloads")
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn usr(&self) -> PathBuf {
        self.root.join("usr")
    }
    pub fn config_file(&self) -> PathBuf {
        self.etc().join("padu.toml")
    }
    pub fn sites_file(&self) -> PathBuf {
        self.etc().join("sites.toml")
    }
    pub fn checksums_file(&self) -> PathBuf {
        self.etc().join("checksums.json")
    }
    pub fn installed_file(&self) -> PathBuf {
        self.etc().join("installed.json")
    }

    /// Create every directory; secrets/ssl are private.
    pub fn ensure(&self) -> Result<()> {
        for d in [
            self.bin(),
            self.etc(),
            self.apps(),
            self.data(),
            self.www(),
            self.logs(),
            self.run(),
            self.tmp(),
            self.downloads(),
            self.backups(),
            self.usr(),
        ] {
            std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
        }
        for d in [self.ssl(), self.secrets()] {
            std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
            crate::fsutil::make_private_dir(&d)?;
        }
        Ok(())
    }

    /// Paths of installed runtime binaries, for prefixing `PATH` in terminals and child processes.
    pub fn path_entries(&self, extra: &[PathBuf]) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = extra.to_vec();
        v.push(self.usr().join("bin"));
        v
    }

    /// Normalise a path into forward slashes (used inside generated config files).
    pub fn slashes(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }
}
