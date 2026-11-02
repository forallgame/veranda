//! Installed runtimes under `bin/<kind>/<version>/` and the registry that tracks them.

use crate::archive;
use crate::config::{DbEngine, WebServer};
use crate::download::{self, ProgressFn};
use crate::fsutil;
use crate::manifest::{ArchiveType, Kind, Package};
use crate::paths::Layout;
use crate::platform::Platform;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub kind: Kind,
    pub version: String,
    pub path: PathBuf,
    pub source: String,
    pub installed_at: String,
    /// SHA-256 of each downloaded part, for audit.
    #[serde(default)]
    pub digests: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub items: Vec<Installed>,
}

impl Registry {
    pub fn load(layout: &Layout) -> Result<Self> {
        let mut reg: Registry = match std::fs::read_to_string(layout.installed_file()) {
            Ok(t) => serde_json::from_str(&t).unwrap_or_default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Registry::default(),
            Err(e) => return Err(e.into()),
        };
        // Drop entries whose folder disappeared (user deleted it by hand).
        reg.items.retain(|i| i.path.is_dir());
        Ok(reg)
    }

    pub fn save(&self, layout: &Layout) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        fsutil::write_atomic(&layout.installed_file(), text.as_bytes())
    }

    /// Installed versions of `kind`, newest first.
    pub fn list(&self, kind: Kind) -> Vec<&Installed> {
        let mut v: Vec<&Installed> = self.items.iter().filter(|i| i.kind == kind).collect();
        v.sort_by_key(|i| std::cmp::Reverse(crate::manifest::parse_version(&i.version)));
        v
    }

    pub fn find(&self, kind: Kind, version: &str) -> Option<&Installed> {
        self.items
            .iter()
            .find(|i| i.kind == kind && i.version == version)
    }

    pub fn newest(&self, kind: Kind) -> Option<&Installed> {
        self.list(kind).into_iter().next()
    }

    /// The version to use: configured one if installed, otherwise the newest installed.
    pub fn active(&self, kind: Kind, configured: &str) -> Option<&Installed> {
        if !configured.trim().is_empty()
            && let Some(i) = self.find(kind, configured.trim())
        {
            return Some(i);
        }
        self.newest(kind)
    }
}

pub fn package_dir(layout: &Layout, kind: Kind, version: &str) -> PathBuf {
    layout.bin_kind(kind.id()).join(version)
}

/// Download, verify, extract and register a package. Safe to call again (reinstalls).
pub async fn install(
    layout: &Layout,
    pkg: &Package,
    progress: Option<&ProgressFn>,
) -> Result<Installed> {
    if !pkg.matches(Platform::current()) {
        bail!(
            "package {} {} is for {} not {}",
            pkg.kind,
            pkg.version,
            pkg.platform(),
            Platform::current()
        );
    }
    if !fsutil::is_safe_name(&pkg.version.to_ascii_lowercase().replace('.', "-")) {
        bail!("refusing suspicious version string {:?}", pkg.version);
    }
    let client = download::client()?;
    let final_dir = package_dir(layout, pkg.kind, &pkg.version);
    let staging = layout
        .bin_kind(pkg.kind.id())
        .join(format!(".staging-{}", pkg.version));
    fsutil::remove_dir_all_if_exists(&staging)?;
    std::fs::create_dir_all(&staging)?;

    let mut digests = Vec::new();
    let result: Result<()> = async {
        for part in &pkg.parts {
            let file_name = url_file_name(&part.url)?;
            let dl = layout
                .downloads()
                .join(format!("{}-{}-{}", pkg.kind, pkg.version, file_name));
            let digest =
                download::download(&client, layout, &part.url, &dl, &part.checksum, progress)
                    .await?;
            digests.push(digest);
            let dest = if part.dest.is_empty() {
                staging.clone()
            } else {
                staging.join(&part.dest)
            };
            std::fs::create_dir_all(&dest)?;
            match part.archive {
                ArchiveType::File => {
                    let name = if part.file_name.is_empty() {
                        file_name.clone()
                    } else {
                        part.file_name.clone()
                    };
                    if !fsutil::is_safe_name(&name.to_ascii_lowercase().replace('.', "-")) {
                        bail!("unsafe file name {name:?}");
                    }
                    std::fs::copy(&dl, dest.join(&name))?;
                }
                t => archive::extract(&dl, t, &dest, part.strip as usize as u32)?,
            }
            let _ = std::fs::remove_file(&dl);
        }
        post_install(pkg.kind, &staging)?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        let _ = fsutil::remove_dir_all_if_exists(&staging);
        return Err(e);
    }

    fsutil::remove_dir_all_if_exists(&final_dir)?;
    std::fs::rename(&staging, &final_dir)
        .with_context(|| format!("moving into {}", final_dir.display()))?;

    let installed = Installed {
        kind: pkg.kind,
        version: pkg.version.clone(),
        path: final_dir,
        source: pkg.source.clone(),
        installed_at: fsutil::timestamp(),
        digests,
    };
    let mut reg = Registry::load(layout)?;
    reg.items
        .retain(|i| !(i.kind == pkg.kind && i.version == pkg.version));
    reg.items.push(installed.clone());
    reg.save(layout)?;
    Ok(installed)
}

pub fn remove(layout: &Layout, kind: Kind, version: &str) -> Result<()> {
    let mut reg = Registry::load(layout)?;
    let Some(item) = reg.find(kind, version).cloned() else {
        bail!("{kind} {version} is not installed")
    };
    if !item.path.starts_with(layout.bin()) {
        bail!("refusing to delete {} outside bin/", item.path.display());
    }
    fsutil::remove_dir_all_if_exists(&item.path)?;
    reg.items
        .retain(|i| !(i.kind == kind && i.version == version));
    reg.save(layout)
}

fn url_file_name(url: &str) -> Result<String> {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("").to_string();
    if name.is_empty() || name.contains("..") || name.contains('\\') {
        bail!("cannot derive a file name from {url}");
    }
    Ok(name)
}

/// Kind specific fix-ups after extraction (executable bits, layout normalisation).
fn post_install(kind: Kind, dir: &Path) -> Result<()> {
    match kind {
        Kind::Php => {
            for n in ["php", "php-fpm", "php-cgi"] {
                let p = dir.join(n);
                if p.is_file() {
                    fsutil::make_executable(&p)?;
                }
            }
        }
        Kind::Caddy => {
            let p = dir.join("caddy");
            if p.is_file() {
                fsutil::make_executable(&p)?;
            }
        }
        Kind::Mailpit => {
            let p = dir.join("mailpit");
            if p.is_file() {
                fsutil::make_executable(&p)?;
            }
        }
        Kind::Composer => {
            // Tiny wrapper so `composer` works from a terminal.
            #[cfg(unix)]
            {
                let sh = dir.join("composer");
                std::fs::write(
                    &sh,
                    "#!/bin/sh\nexec php \"$(dirname \"$0\")/composer.phar\" \"$@\"\n",
                )?;
                fsutil::make_executable(&sh)?;
            }
            #[cfg(windows)]
            {
                std::fs::write(
                    dir.join("composer.bat"),
                    "@echo off\r\nphp \"%~dp0composer.phar\" %*\r\n",
                )?;
            }
        }
        Kind::PhpMyAdmin => {
            // Generated config: connect to 127.0.0.1 only, cookie auth, random blowfish secret.
            let secret: String = crate::db::random_token(32);
            let cfg = format!(
                "<?php\n$cfg['blowfish_secret'] = '{secret}';\n$i = 1;\n$cfg['Servers'][$i]['auth_type'] = 'cookie';\n$cfg['Servers'][$i]['host'] = '127.0.0.1';\n$cfg['Servers'][$i]['AllowNoPassword'] = false;\n$cfg['TempDir'] = __DIR__ . '/tmp';\n$cfg['CheckConfigurationPermissions'] = false;\n"
            );
            std::fs::write(dir.join("config.inc.php"), cfg)?;
            std::fs::create_dir_all(dir.join("tmp"))?;
        }
        _ => {}
    }
    Ok(())
}

/// Resolve the main executables of an installed package.
pub struct Binaries;

impl Binaries {
    fn exe(p: &Path, name: &str) -> PathBuf {
        p.join(Platform::current().exe(name))
    }

    pub fn php(i: &Installed) -> PathBuf {
        Self::exe(&i.path, "php")
    }
    /// FastCGI server: `php-fpm` on unix, `php-cgi.exe` on Windows.
    pub fn php_fcgi(i: &Installed) -> PathBuf {
        if Platform::current().is_windows() {
            Self::exe(&i.path, "php-cgi")
        } else {
            i.path.join("php-fpm")
        }
    }
    pub fn caddy(i: &Installed) -> PathBuf {
        Self::exe(&i.path, "caddy")
    }
    pub fn mailpit(i: &Installed) -> PathBuf {
        Self::exe(&i.path, "mailpit")
    }
    pub fn nginx(i: &Installed) -> PathBuf {
        Self::exe(&i.path, "nginx")
    }
    pub fn httpd(i: &Installed) -> PathBuf {
        Self::exe(&i.path.join("bin"), "httpd")
    }
    pub fn db_server(i: &Installed, engine: DbEngine) -> PathBuf {
        let bin = i.path.join("bin");
        match engine {
            DbEngine::MariaDb => {
                let p = Self::exe(&bin, "mariadbd");
                if p.exists() {
                    p
                } else {
                    Self::exe(&bin, "mysqld")
                }
            }
            DbEngine::MySql => Self::exe(&bin, "mysqld"),
        }
    }
    pub fn db_client(i: &Installed, engine: DbEngine) -> PathBuf {
        let bin = i.path.join("bin");
        match engine {
            DbEngine::MariaDb => {
                let p = Self::exe(&bin, "mariadb");
                if p.exists() {
                    p
                } else {
                    Self::exe(&bin, "mysql")
                }
            }
            DbEngine::MySql => Self::exe(&bin, "mysql"),
        }
    }
    pub fn db_dump(i: &Installed, engine: DbEngine) -> PathBuf {
        let bin = i.path.join("bin");
        match engine {
            DbEngine::MariaDb => {
                let p = Self::exe(&bin, "mariadb-dump");
                if p.exists() {
                    p
                } else {
                    Self::exe(&bin, "mysqldump")
                }
            }
            DbEngine::MySql => Self::exe(&bin, "mysqldump"),
        }
    }
    pub fn db_admin(i: &Installed, engine: DbEngine) -> PathBuf {
        let bin = i.path.join("bin");
        match engine {
            DbEngine::MariaDb => {
                let p = Self::exe(&bin, "mariadb-admin");
                if p.exists() {
                    p
                } else {
                    Self::exe(&bin, "mysqladmin")
                }
            }
            DbEngine::MySql => Self::exe(&bin, "mysqladmin"),
        }
    }
    /// MariaDB's data directory initialiser (unix: scripts/, windows: bin/).
    pub fn mariadb_install_db(i: &Installed) -> Option<PathBuf> {
        let candidates = [
            i.path.join("scripts").join("mariadb-install-db"),
            i.path.join("scripts").join("mysql_install_db"),
            i.path.join("bin").join("mariadb-install-db.exe"),
            i.path.join("bin").join("mysql_install_db.exe"),
        ];
        candidates.into_iter().find(|p| p.exists())
    }
    pub fn cache_server(i: &Installed) -> PathBuf {
        for n in ["valkey-server", "redis-server"] {
            for dir in [i.path.join("bin"), i.path.clone()] {
                let p = Self::exe(&dir, n);
                if p.exists() {
                    return p;
                }
            }
        }
        Self::exe(&i.path.join("bin"), "valkey-server")
    }
    pub fn cache_cli(i: &Installed) -> PathBuf {
        for n in ["valkey-cli", "redis-cli"] {
            for dir in [i.path.join("bin"), i.path.clone()] {
                let p = Self::exe(&dir, n);
                if p.exists() {
                    return p;
                }
            }
        }
        Self::exe(&i.path.join("bin"), "valkey-cli")
    }
    pub fn node_bin_dir(i: &Installed) -> PathBuf {
        if Platform::current().is_windows() {
            i.path.clone()
        } else {
            i.path.join("bin")
        }
    }
}

/// Directories to prepend to PATH for terminals and child processes.
pub fn runtime_path(layout: &Layout, reg: &Registry, cfg: &crate::Config) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(i) = reg.active(Kind::Php, &cfg.php.version) {
        v.push(i.path.clone());
    }
    if let Some(i) = reg.newest(Kind::Composer) {
        v.push(i.path.clone());
    }
    if let Some(i) = reg.newest(Kind::Node) {
        v.push(Binaries::node_bin_dir(i));
    }
    let db_kind = match cfg.db.engine {
        DbEngine::MariaDb => Kind::MariaDb,
        DbEngine::MySql => Kind::MySql,
    };
    if let Some(i) = reg.active(db_kind, &cfg.db.version) {
        v.push(i.path.join("bin"));
    }
    let web_kind = match cfg.web.server {
        WebServer::Caddy => Kind::Caddy,
        WebServer::Nginx => Kind::Nginx,
        WebServer::Apache => Kind::Apache,
    };
    if let Some(i) = reg.active(web_kind, &cfg.web.version) {
        v.push(if web_kind == Kind::Apache {
            i.path.join("bin")
        } else {
            i.path.clone()
        });
    }
    if let Some(i) = reg.newest(Kind::Cache) {
        v.push(
            Binaries::cache_cli(i)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(i.path.clone()),
        );
    }
    if let Some(i) = reg.newest(Kind::Mailpit) {
        v.push(i.path.clone());
    }
    v.push(layout.usr().join("bin"));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        assert_eq!(url_file_name("https://x/y/php.zip?x=1").unwrap(), "php.zip");
        assert!(url_file_name("https://x/").is_err());
    }
}
