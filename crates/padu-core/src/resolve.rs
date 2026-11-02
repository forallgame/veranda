//! Discover upstream versions at run time.
//!
//! `versions(kind, platform)` returns every version Padu can install from official sources,
//! newest first; `latest` is its first element. Every resolver is best-effort: a failure only
//! means the built-in manifest is used. Sources with real checksums are preferred; otherwise
//! HTTPS + trust-on-first-use.

use crate::download::{client, fetch_text};
use crate::manifest::{ArchiveType, Checksum, Kind, Manifest, Package, Part, parse_version};
use crate::platform::{Arch, Os, Platform};
use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Upper bound of versions returned per kind (keeps the UI list readable).
const MAX_VERSIONS: usize = 24;

fn pkg(kind: Kind, version: &str, p: Platform, source: &str, parts: Vec<Part>) -> Package {
    Package {
        kind,
        version: version.to_string(),
        os: p.os,
        arch: p.arch,
        parts,
        source: source.into(),
        notes: String::new(),
    }
}

fn part(url: String, checksum: Checksum, strip: u32) -> Part {
    Part {
        archive: ArchiveType::guess(&url),
        url,
        checksum,
        strip,
        dest: String::new(),
        file_name: String::new(),
    }
}

fn file_part(url: String, checksum: Checksum, file_name: &str) -> Part {
    Part {
        archive: ArchiveType::File,
        url,
        checksum,
        strip: 0,
        dest: String::new(),
        file_name: file_name.into(),
    }
}

/// Sort newest first, drop duplicates, cap the list.
fn finish(mut v: Vec<Package>) -> Vec<Package> {
    v.sort_by_key(|p| std::cmp::Reverse(parse_version(&p.version)));
    v.dedup_by(|a, b| a.version == b.version);
    v.truncate(MAX_VERSIONS);
    v
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

async fn github_releases(c: &reqwest::Client, repo: &str, n: usize) -> Result<Vec<GhRelease>> {
    let text = fetch_text(
        c,
        &format!("https://api.github.com/repos/{repo}/releases?per_page={n}"),
    )
    .await?;
    let rels: Vec<GhRelease> = serde_json::from_str(&text).context("parsing GitHub releases")?;
    Ok(rels
        .into_iter()
        .filter(|r| !r.prerelease && !r.draft)
        .collect())
}

fn gh_checksum(asset: &GhAsset) -> Checksum {
    match asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        Some(h) if h.len() == 64 => Checksum::Sha256 { hex: h.to_string() },
        _ => Checksum::Tofu,
    }
}

/// Every installable version of `kind` for `p`, newest first.
pub async fn versions(kind: Kind, p: Platform) -> Result<Vec<Package>> {
    let c = client()?;
    let v = match kind {
        Kind::Php => php(&c, p).await?,
        Kind::Caddy => caddy(&c, p).await?,
        Kind::Mailpit => mailpit(&c, p).await?,
        Kind::MariaDb => mariadb(&c, p).await?,
        Kind::MySql => mysql(&c, p).await?,
        Kind::Node => node(&c, p).await?,
        Kind::Composer => composer(&c, p).await?,
        Kind::Adminer => adminer(&c, p).await?,
        Kind::PhpMyAdmin => phpmyadmin(&c, p).await?,
        Kind::Nginx => nginx(&c, p).await?,
        Kind::Apache => apache(&c, p).await?,
        Kind::Cache => cache(&c, p).await?,
        Kind::CaBundle => vec![cabundle(&c, p).await?],
    };
    if v.is_empty() {
        bail!("no {kind} versions found upstream");
    }
    Ok(finish(v))
}

/// Newest package of `kind` for `p`, from upstream.
pub async fn latest(kind: Kind, p: Platform) -> Result<Package> {
    versions(kind, p)
        .await?
        .into_iter()
        .next()
        .context("empty version list")
}

/// Every version of every kind; failures are logged and skipped.
pub async fn versions_all(p: Platform) -> Manifest {
    let mut m = Manifest::default();
    for kind in Kind::ALL {
        match versions(kind, p).await {
            Ok(v) => m.packages.extend(v),
            Err(e) => tracing::warn!("resolve {kind}: {e:#}"),
        }
    }
    m
}

/// Newest of every kind; failures are logged and skipped.
pub async fn latest_all(p: Platform) -> Manifest {
    let mut m = versions_all(p).await;
    let mut seen = std::collections::HashSet::new();
    m.packages.retain(|pk| seen.insert(pk.kind));
    m
}

// ---------------------------------------------------------------- PHP

#[derive(Deserialize)]
struct WinBuild {
    zip: Option<WinFile>,
}
#[derive(Deserialize)]
struct WinFile {
    path: String,
    sha256: String,
}

async fn php(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    match p.os {
        Os::Windows => {
            let text = fetch_text(
                c,
                "https://downloads.php.net/~windows/releases/releases.json",
            )
            .await?;
            let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)?;
            let mut out = Vec::new();
            for (branch, entry) in &map {
                // 8.1 and newer only; older branches are end-of-life.
                if parse_version(branch) < parse_version("8.1") {
                    continue;
                }
                let Some(version) = entry.get("version").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(obj) = entry.as_object() else {
                    continue;
                };
                let Some(key) = obj
                    .keys()
                    .find(|k| k.starts_with("nts-vs") && k.ends_with("-x64"))
                else {
                    continue;
                };
                let Ok(build) = serde_json::from_value::<WinBuild>(entry[key].clone()) else {
                    continue;
                };
                let Some(zip) = build.zip else { continue };
                let url = format!("https://downloads.php.net/~windows/releases/{}", zip.path);
                out.push(pkg(
                    Kind::Php,
                    version,
                    p,
                    "php.net",
                    vec![part(url, Checksum::Sha256 { hex: zip.sha256 }, 0)],
                ));
            }
            Ok(out)
        }
        _ => {
            let text = fetch_text(
                c,
                "https://dl.static-php.dev/static-php-cli/common/?format=json",
            )
            .await?;
            #[derive(Deserialize)]
            struct Entry {
                name: String,
            }
            let entries: Vec<Entry> = serde_json::from_str(&text)?;
            let os = if p.os == Os::Macos { "macos" } else { "linux" };
            let arch = match p.arch {
                Arch::X86_64 => "x86_64",
                Arch::Aarch64 => "aarch64",
            };
            let re = regex::Regex::new(&format!(
                r"^php-(\d+\.\d+\.\d+)-(cli|fpm)-{os}-{arch}\.tar\.gz$"
            ))?;
            let mut have: std::collections::HashMap<String, u8> = Default::default();
            for e in &entries {
                if let Some(m) = re.captures(&e.name) {
                    *have.entry(m[1].to_string()).or_default() += 1;
                }
            }
            let base = "https://dl.static-php.dev/static-php-cli/common";
            let mut out: Vec<Package> = have
                .into_iter()
                .filter(|(v, n)| *n == 2 && parse_version(v) >= parse_version("8.1"))
                .map(|(v, _)| {
                    pkg(
                        Kind::Php,
                        &v,
                        p,
                        "static-php.dev",
                        vec![
                            part(
                                format!("{base}/php-{v}-cli-{os}-{arch}.tar.gz"),
                                Checksum::Tofu,
                                0,
                            ),
                            part(
                                format!("{base}/php-{v}-fpm-{os}-{arch}.tar.gz"),
                                Checksum::Tofu,
                                0,
                            ),
                        ],
                    )
                })
                .collect();
            // Keep the newest 4 patch releases of every minor branch.
            out.sort_by_key(|x| std::cmp::Reverse(parse_version(&x.version)));
            let mut per_branch: std::collections::HashMap<String, usize> = Default::default();
            out.retain(|x| {
                let b = crate::manifest::version_branch(&x.version);
                let n = per_branch.entry(b).or_default();
                *n += 1;
                *n <= 4
            });
            Ok(out)
        }
    }
}

// ---------------------------------------------------------------- GitHub hosted

async fn caddy(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let suffix = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "linux_amd64.tar.gz",
        (Os::Linux, Arch::Aarch64) => "linux_arm64.tar.gz",
        (Os::Macos, Arch::X86_64) => "mac_amd64.tar.gz",
        (Os::Macos, Arch::Aarch64) => "mac_arm64.tar.gz",
        (Os::Windows, Arch::X86_64) => "windows_amd64.zip",
        (Os::Windows, Arch::Aarch64) => "windows_arm64.zip",
    };
    let mut out = Vec::new();
    for rel in github_releases(c, "caddyserver/caddy", 12).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        let name = format!("caddy_{v}_{suffix}");
        let Some(asset) = rel.assets.iter().find(|a| a.name == name) else {
            continue;
        };
        let checksum = match rel
            .assets
            .iter()
            .find(|a| a.name.ends_with("_checksums.txt"))
        {
            Some(s) => Checksum::Remote {
                url: s.browser_download_url.clone(),
                name: name.clone(),
            },
            None => gh_checksum(asset),
        };
        out.push(pkg(
            Kind::Caddy,
            &v,
            p,
            "github",
            vec![part(asset.browser_download_url.clone(), checksum, 0)],
        ));
    }
    Ok(out)
}

async fn mailpit(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let name = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "mailpit-linux-amd64.tar.gz",
        (Os::Linux, Arch::Aarch64) => "mailpit-linux-arm64.tar.gz",
        (Os::Macos, Arch::X86_64) => "mailpit-darwin-amd64.tar.gz",
        (Os::Macos, Arch::Aarch64) => "mailpit-darwin-arm64.tar.gz",
        (Os::Windows, Arch::X86_64) => "mailpit-windows-amd64.zip",
        (Os::Windows, Arch::Aarch64) => "mailpit-windows-arm64.zip",
    };
    let mut out = Vec::new();
    for rel in github_releases(c, "axllent/mailpit", 10).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        if let Some(asset) = rel.assets.iter().find(|a| a.name == name) {
            out.push(pkg(
                Kind::Mailpit,
                &v,
                p,
                "github",
                vec![part(
                    asset.browser_download_url.clone(),
                    gh_checksum(asset),
                    0,
                )],
            ));
        }
    }
    Ok(out)
}

async fn adminer(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let mut out = Vec::new();
    for rel in github_releases(c, "vrana/adminer", 8).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        let name = format!("adminer-{v}.php");
        if let Some(asset) = rel.assets.iter().find(|a| a.name == name) {
            out.push(pkg(
                Kind::Adminer,
                &v,
                p,
                "github",
                vec![file_part(
                    asset.browser_download_url.clone(),
                    gh_checksum(asset),
                    "index.php",
                )],
            ));
        }
    }
    Ok(out)
}

async fn cache(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    match p.os {
        Os::Windows => {
            let mut out = Vec::new();
            for rel in github_releases(c, "redis-windows/redis-windows", 10).await? {
                let v = rel.tag_name.trim_start_matches('v').to_string();
                if let Some(asset) = rel
                    .assets
                    .iter()
                    .find(|a| a.name.ends_with("-Windows-x64-msys2.zip"))
                {
                    out.push(pkg(
                        Kind::Cache,
                        &v,
                        p,
                        "github:redis-windows",
                        vec![part(
                            asset.browser_download_url.clone(),
                            gh_checksum(asset),
                            0,
                        )],
                    ));
                }
            }
            Ok(out)
        }
        Os::Linux => {
            // Valkey publishes tarballs + .sha256 per file for Ubuntu LTS builds ("jammy").
            let arch = match p.arch {
                Arch::X86_64 => "x86_64",
                Arch::Aarch64 => "arm64",
            };
            let mut out = Vec::new();
            for r in github_releases(c, "valkey-io/valkey", 12)
                .await?
                .into_iter()
                .take(8)
            {
                let v = r.tag_name.trim_start_matches('v').to_string();
                let name = format!("valkey-{v}-jammy-{arch}.tar.gz");
                let url = format!("https://download.valkey.io/releases/{name}");
                let sum = format!("{url}.sha256");
                if let Ok(s) = fetch_text(c, &sum).await
                    && crate::download::find_in_checksum_file(&s, &name).is_some()
                {
                    out.push(pkg(
                        Kind::Cache,
                        &v,
                        p,
                        "valkey.io",
                        vec![part(url, Checksum::Remote { url: sum, name }, 1)],
                    ));
                }
            }
            Ok(out)
        }
        Os::Macos => bail!("no official Valkey binaries for macOS; use `brew install valkey`"),
    }
}

// ---------------------------------------------------------------- MariaDB / MySQL

async fn mariadb(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Majors {
        major_releases: Vec<Major>,
    }
    #[derive(Deserialize)]
    struct Major {
        release_id: String,
        release_status: String,
    }
    #[derive(Deserialize)]
    struct Latest {
        releases: std::collections::BTreeMap<String, Rel>,
    }
    #[derive(Deserialize)]
    struct Rel {
        release_id: String,
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        file_name: String,
        file_download_url: String,
        os: Option<String>,
        cpu: Option<String>,
        checksum: Option<Sum>,
    }
    #[derive(Deserialize)]
    struct Sum {
        sha256sum: Option<String>,
    }
    let (os, cpu, suffix) = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => ("Linux", "x86_64", "-x86_64.tar.gz"),
        (Os::Linux, Arch::Aarch64) => ("Linux", "aarch64", "-aarch64.tar.gz"),
        (Os::Windows, Arch::X86_64) => ("Windows", "x86_64", "-winx64.zip"),
        _ => bail!("MariaDB has no official binaries for {p}; use MySQL or `brew install mariadb`"),
    };
    let text = fetch_text(c, "https://downloads.mariadb.org/rest-api/mariadb/").await?;
    let majors: Majors = serde_json::from_str(&text)?;
    let mut out = Vec::new();
    for major in majors
        .major_releases
        .iter()
        .filter(|m| m.release_status == "Stable")
    {
        let Ok(text) = fetch_text(
            c,
            &format!(
                "https://downloads.mariadb.org/rest-api/mariadb/{}/latest/",
                major.release_id
            ),
        )
        .await
        else {
            continue;
        };
        let Ok(latest) = serde_json::from_str::<Latest>(&text) else {
            continue;
        };
        let Some(rel) = latest.releases.values().next() else {
            continue;
        };
        let f = rel
            .files
            .iter()
            .filter(|f| f.os.as_deref() == Some(os) && f.cpu.as_deref() == Some(cpu))
            .filter(|f| f.file_name.ends_with(suffix) && !f.file_name.contains("debug"))
            .find(|f| f.file_name.contains("systemd") || os == "Windows")
            .or_else(|| {
                rel.files.iter().find(|f| {
                    f.os.as_deref() == Some(os)
                        && f.cpu.as_deref() == Some(cpu)
                        && f.file_name.ends_with(suffix)
                })
            });
        let Some(f) = f else { continue };
        let url = f.file_download_url.replacen("http://", "https://", 1);
        let checksum = match f.checksum.as_ref().and_then(|s| s.sha256sum.clone()) {
            Some(h) => Checksum::Sha256 { hex: h },
            None => Checksum::Tofu,
        };
        out.push(pkg(
            Kind::MariaDb,
            &rel.release_id,
            p,
            "mariadb.org",
            vec![part(url, checksum, 1)],
        ));
    }
    Ok(out)
}

/// MySQL has no version API; probe the CDN for newer patch releases of each supported branch.
async fn mysql(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    // Seed = newest patch known at release time; the probe looks for later ones.
    const SEEDS: [(&str, &str); 3] = [("9.7", "9.7.4"), ("8.4", "8.4.13"), ("8.0", "8.0.46")];
    let file = |v: &str| -> String {
        match (p.os, p.arch) {
            (Os::Windows, _) => format!("mysql-{v}-winx64.zip"),
            (Os::Linux, Arch::X86_64) => format!("mysql-{v}-linux-glibc2.28-x86_64.tar.xz"),
            (Os::Linux, Arch::Aarch64) => format!("mysql-{v}-linux-glibc2.28-aarch64.tar.xz"),
            (Os::Macos, Arch::Aarch64) => format!("mysql-{v}-macos15-arm64.tar.gz"),
            (Os::Macos, Arch::X86_64) => format!("mysql-{v}-macos15-x86_64.tar.gz"),
        }
    };
    let mut out = Vec::new();
    for (branch, seed) in SEEDS {
        let key = parse_version(seed);
        let (major, minor, patch) = match key.0.as_slice() {
            [a, b, c, ..] => (*a, *b, *c),
            _ => continue,
        };
        let mut best: Option<Package> = None;
        for next in patch..=(patch + 6) {
            let v = format!("{major}.{minor}.{next}");
            let name = file(&v);
            let url = format!("https://cdn.mysql.com/Downloads/MySQL-{branch}/{name}");
            match fetch_text(c, &format!("{url}.md5")).await {
                Ok(text) => {
                    if let Some(h) = crate::download::find_in_checksum_file(&text, &name) {
                        best = Some(pkg(
                            Kind::MySql,
                            &v,
                            p,
                            "mysql.com",
                            vec![part(url, Checksum::Md5 { hex: h }, 1)],
                        ));
                    }
                }
                Err(_) => {
                    if next > patch {
                        break;
                    }
                }
            }
        }
        if let Some(b) = best {
            out.push(b);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- Node / Composer / phpMyAdmin / nginx / Apache / CA bundle

async fn node(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Entry {
        version: String,
        lts: serde_json::Value,
    }
    let text = fetch_text(c, "https://nodejs.org/dist/index.json").await?;
    let entries: Vec<Entry> = serde_json::from_str(&text)?;
    let name = |v: &str| match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => format!("node-v{v}-linux-x64.tar.xz"),
        (Os::Linux, Arch::Aarch64) => format!("node-v{v}-linux-arm64.tar.xz"),
        (Os::Macos, Arch::X86_64) => format!("node-v{v}-darwin-x64.tar.gz"),
        (Os::Macos, Arch::Aarch64) => format!("node-v{v}-darwin-arm64.tar.gz"),
        (Os::Windows, Arch::X86_64) => format!("node-v{v}-win-x64.zip"),
        (Os::Windows, Arch::Aarch64) => format!("node-v{v}-win-arm64.zip"),
    };
    // Newest release of each LTS major (index.json is newest first).
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in entries
        .iter()
        .filter(|e| e.lts.as_bool() != Some(false) && !e.version.contains('-'))
    {
        let v = e.version.trim_start_matches('v').to_string();
        let major = parse_version(&v).0.first().copied().unwrap_or(0);
        if !seen.insert(major) {
            continue;
        }
        let n = name(&v);
        out.push(pkg(
            Kind::Node,
            &v,
            p,
            "nodejs.org",
            vec![part(
                format!("https://nodejs.org/dist/v{v}/{n}"),
                Checksum::Remote {
                    url: format!("https://nodejs.org/dist/v{v}/SHASUMS256.txt"),
                    name: n,
                },
                1,
            )],
        ));
        if out.len() >= 4 {
            break;
        }
    }
    Ok(out)
}

async fn composer(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Versions {
        stable: Vec<Chan>,
    }
    #[derive(Deserialize)]
    struct Chan {
        version: String,
        path: String,
    }
    let text = fetch_text(c, "https://getcomposer.org/versions").await?;
    let v: Versions = serde_json::from_str(&text)?;
    Ok(v.stable
        .iter()
        .map(|ch| {
            let url = format!("https://getcomposer.org{}", ch.path);
            pkg(
                Kind::Composer,
                &ch.version,
                p,
                "getcomposer.org",
                vec![file_part(
                    url.clone(),
                    Checksum::Remote {
                        url: format!("{url}.sha256sum"),
                        name: "composer.phar".into(),
                    },
                    "composer.phar",
                )],
            )
        })
        .collect())
}

async fn phpmyadmin(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct V {
        releases: Vec<R>,
    }
    #[derive(Deserialize)]
    struct R {
        version: String,
    }
    let text = fetch_text(c, "https://www.phpmyadmin.net/home_page/version.json").await?;
    let v: V = serde_json::from_str(&text)?;
    Ok(v.releases
        .iter()
        .map(|r| {
            let name = format!("phpMyAdmin-{}-all-languages.zip", r.version);
            let url = format!(
                "https://files.phpmyadmin.net/phpMyAdmin/{}/{name}",
                r.version
            );
            pkg(
                Kind::PhpMyAdmin,
                &r.version,
                p,
                "phpmyadmin.net",
                vec![part(
                    url.clone(),
                    Checksum::Remote {
                        url: format!("{url}.sha256"),
                        name,
                    },
                    1,
                )],
            )
        })
        .collect())
}

async fn nginx(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    if p.os != Os::Windows {
        bail!("nginx on Linux/macOS comes from the system package manager (apt/brew)");
    }
    let text = fetch_text(c, "https://nginx.org/en/download.html").await?;
    let re = regex::Regex::new(r"/download/nginx-(\d+\.\d+\.\d+)\.zip")?;
    let mut seen = std::collections::HashSet::new();
    Ok(re
        .captures_iter(&text)
        .map(|m| m[1].to_string())
        .filter(|v| seen.insert(v.clone()))
        .map(|v| {
            pkg(
                Kind::Nginx,
                &v,
                p,
                "nginx.org",
                vec![part(
                    format!("https://nginx.org/download/nginx-{v}.zip"),
                    Checksum::Tofu,
                    1,
                )],
            )
        })
        .collect())
}

async fn apache(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    if p.os != Os::Windows {
        bail!("Apache on Linux/macOS comes from the system package manager (apt/brew)");
    }
    let text = fetch_text(c, "https://www.apachelounge.com/download/").await?;
    let re = regex::Regex::new(
        r"/download/(VS\d+)/binaries/(httpd-(\d+\.\d+\.\d+-\d+)-Win64-VS\d+\.zip)",
    )?;
    let mut seen = std::collections::HashSet::new();
    Ok(re
        .captures_iter(&text)
        .map(|m| (m[1].to_string(), m[2].to_string(), m[3].to_string()))
        .filter(|(_, _, v)| seen.insert(v.clone()))
        .map(|(vs, name, ver)| {
            let url = format!("https://www.apachelounge.com/download/{vs}/binaries/{name}");
            pkg(
                Kind::Apache,
                &ver,
                p,
                "apachelounge.com",
                vec![part(
                    url.clone(),
                    Checksum::Remote {
                        url: format!("{url}.txt"),
                        name,
                    },
                    1,
                )],
            )
        })
        .collect())
}

async fn cabundle(c: &reqwest::Client, p: Platform) -> Result<Package> {
    let head = fetch_text(c, "https://curl.se/ca/cacert.pem.sha256").await?;
    let _ = crate::download::find_in_checksum_file(&head, "cacert.pem")
        .context("no sha256 for cacert.pem")?;
    let now = time::OffsetDateTime::now_utc();
    let v = format!(
        "{}.{:02}.{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    );
    Ok(pkg(
        Kind::CaBundle,
        &v,
        p,
        "curl.se",
        vec![file_part(
            "https://curl.se/ca/cacert.pem".into(),
            Checksum::Remote {
                url: "https://curl.se/ca/cacert.pem.sha256".into(),
                name: "cacert.pem".into(),
            },
            "cacert.pem",
        )],
    ))
}
