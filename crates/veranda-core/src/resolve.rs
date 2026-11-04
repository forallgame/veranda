//! Discover upstream versions at run time.
//!
//! `versions(kind, platform)` returns every version Veranda can install from official sources,
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
        Kind::Postgres => postgres(&c, p).await?,
        Kind::Python => python(&c, p).await?,
        Kind::Go => go(&c, p).await?,
        Kind::Bun => bun(&c, p).await?,
        Kind::MongoDb => mongodb(&c, p).await?,
        Kind::Meilisearch => meilisearch(&c, p).await?,
        Kind::Qdrant => qdrant(&c, p).await?,
        Kind::Java => java(&c, p).await?,
        Kind::Deno => deno(&c, p).await?,
        Kind::Zig => zig(&c, p).await?,
        Kind::DotNet => dotnet(&c, p).await?,
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

// ---------------------------------------------------------------- PostgreSQL / Python / Go / Bun

/// zonky.io publishes the official PostgreSQL binaries as Maven artifacts (jar = zip holding one
/// `.txz`), with a `.sha256` next to every jar.
async fn postgres(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let artifact = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "linux-amd64",
        (Os::Linux, Arch::Aarch64) => "linux-arm64v8",
        (Os::Macos, Arch::X86_64) => "darwin-amd64",
        (Os::Macos, Arch::Aarch64) => "darwin-arm64v8",
        (Os::Windows, Arch::X86_64) => "windows-amd64",
        (Os::Windows, Arch::Aarch64) => bail!("no PostgreSQL build for Windows on ARM"),
    };
    let base = format!(
        "https://repo1.maven.org/maven2/io/zonky/test/postgres/embedded-postgres-binaries-{artifact}"
    );
    let text = fetch_text(c, &format!("{base}/maven-metadata.xml")).await?;
    let re = regex::Regex::new(r"<version>(\d+)\.(\d+)\.(\d+)</version>")?;
    let mut versions: Vec<String> = re
        .captures_iter(&text)
        .filter(|m| m[1].parse::<u32>().unwrap_or(0) >= 13)
        .map(|m| format!("{}.{}.{}", &m[1], &m[2], &m[3]))
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(parse_version(v)));
    versions.dedup();
    Ok(versions
        .into_iter()
        .take(MAX_VERSIONS)
        .map(|v| {
            let url = format!("{base}/{v}/embedded-postgres-binaries-{artifact}-{v}.jar");
            let mut pt = part(
                url.clone(),
                Checksum::Remote {
                    url: format!("{url}.sha256"),
                    name: String::new(),
                },
                0,
            );
            pt.archive = ArchiveType::Zip;
            pkg(Kind::Postgres, &v, p, "zonky.io / maven central", vec![pt])
        })
        .collect())
}

/// python-build-standalone: one release holds every CPython version; SHA256SUMS lists them all.
async fn python(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let triple = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "x86_64-unknown-linux-gnu",
        (Os::Linux, Arch::Aarch64) => "aarch64-unknown-linux-gnu",
        (Os::Macos, Arch::X86_64) => "x86_64-apple-darwin",
        (Os::Macos, Arch::Aarch64) => "aarch64-apple-darwin",
        (Os::Windows, Arch::X86_64) => "x86_64-pc-windows-msvc",
        (Os::Windows, Arch::Aarch64) => "aarch64-pc-windows-msvc",
    };
    let mut out = Vec::new();
    let re = regex::Regex::new(&format!(
        r"^cpython-(\d+\.\d+\.\d+)\+(\d+)-{}-install_only_stripped\.tar\.gz$",
        regex::escape(triple)
    ))?;
    for rel in github_releases(c, "astral-sh/python-build-standalone", 2).await? {
        let Some(sums) = rel.assets.iter().find(|a| a.name == "SHA256SUMS") else {
            continue;
        };
        for a in &rel.assets {
            let Some(m) = re.captures(&a.name) else {
                continue;
            };
            out.push(pkg(
                Kind::Python,
                &m[1],
                p,
                "python-build-standalone",
                vec![part(
                    a.browser_download_url.clone(),
                    Checksum::Remote {
                        url: sums.browser_download_url.clone(),
                        name: a.name.clone(),
                    },
                    1,
                )],
            ));
        }
        if !out.is_empty() {
            break;
        }
    }
    Ok(out)
}

/// go.dev publishes a JSON index with a sha256 for every archive.
async fn go(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Release {
        version: String,
        stable: bool,
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        filename: String,
        os: String,
        arch: String,
        sha256: String,
        kind: String,
    }
    let (os, arch) = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => ("linux", "amd64"),
        (Os::Linux, Arch::Aarch64) => ("linux", "arm64"),
        (Os::Macos, Arch::X86_64) => ("darwin", "amd64"),
        (Os::Macos, Arch::Aarch64) => ("darwin", "arm64"),
        (Os::Windows, Arch::X86_64) => ("windows", "amd64"),
        (Os::Windows, Arch::Aarch64) => ("windows", "arm64"),
    };
    let text = fetch_text(c, "https://go.dev/dl/?mode=json&include=all").await?;
    let rels: Vec<Release> = serde_json::from_str(&text).context("parsing go.dev index")?;
    Ok(rels
        .into_iter()
        .filter(|r| r.stable)
        .take(16)
        .filter_map(|r| {
            let f = r
                .files
                .into_iter()
                .find(|f| f.kind == "archive" && f.os == os && f.arch == arch)?;
            let v = r.version.trim_start_matches("go").to_string();
            Some(pkg(
                Kind::Go,
                &v,
                p,
                "go.dev",
                vec![part(
                    format!("https://go.dev/dl/{}", f.filename),
                    Checksum::Sha256 { hex: f.sha256 },
                    1,
                )],
            ))
        })
        .collect())
}

async fn bun(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let name = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "bun-linux-x64.zip",
        (Os::Linux, Arch::Aarch64) => "bun-linux-aarch64.zip",
        (Os::Macos, Arch::X86_64) => "bun-darwin-x64.zip",
        (Os::Macos, Arch::Aarch64) => "bun-darwin-aarch64.zip",
        (Os::Windows, Arch::X86_64) => "bun-windows-x64.zip",
        (Os::Windows, Arch::Aarch64) => bail!("no Bun build for Windows on ARM"),
    };
    let mut out = Vec::new();
    for rel in github_releases(c, "oven-sh/bun", 12).await? {
        let v = rel.tag_name.trim_start_matches("bun-v").to_string();
        let Some(asset) = rel.assets.iter().find(|a| a.name == name) else {
            continue;
        };
        let checksum = match rel.assets.iter().find(|a| a.name == "SHASUMS256.txt") {
            Some(s) => Checksum::Remote {
                url: s.browser_download_url.clone(),
                name: name.to_string(),
            },
            None => gh_checksum(asset),
        };
        out.push(pkg(
            Kind::Bun,
            &v,
            p,
            "github",
            vec![part(asset.browser_download_url.clone(), checksum, 1)],
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------- MongoDB / Meilisearch / Qdrant / Java / Deno / Zig / .NET

/// MongoDB publishes `current.json` with every production release and a sha256 per archive.
async fn mongodb(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Index {
        versions: Vec<Ver>,
    }
    #[derive(Deserialize)]
    struct Ver {
        version: String,
        #[serde(default)]
        production_release: bool,
        downloads: Vec<Dl>,
    }
    #[derive(Deserialize)]
    struct Dl {
        #[serde(default)]
        target: String,
        #[serde(default)]
        arch: String,
        #[serde(default)]
        edition: String,
        archive: Archive,
    }
    #[derive(Deserialize)]
    struct Archive {
        url: String,
        #[serde(default)]
        sha256: String,
    }
    // (target, arch, edition) MongoDB uses for this platform
    let want: (&str, &str, &str) = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => ("ubuntu2404", "x86_64", "targeted"),
        (Os::Linux, Arch::Aarch64) => ("ubuntu2404", "aarch64", "targeted"),
        (Os::Macos, Arch::X86_64) => ("macos", "x86_64", "base"),
        (Os::Macos, Arch::Aarch64) => ("macos", "arm64", "base"),
        (Os::Windows, Arch::X86_64) => ("windows", "x86_64", "base"),
        (Os::Windows, Arch::Aarch64) => bail!("no MongoDB build for Windows on ARM"),
    };
    let text = fetch_text(c, "https://downloads.mongodb.org/current.json").await?;
    let idx: Index = serde_json::from_str(&text).context("parsing current.json")?;
    Ok(idx
        .versions
        .into_iter()
        .filter(|v| v.production_release)
        .filter_map(|v| {
            let dl = v
                .downloads
                .into_iter()
                .find(|d| d.target == want.0 && d.arch == want.1 && d.edition == want.2)?;
            let checksum = if dl.archive.sha256.len() == 64 {
                Checksum::Sha256 {
                    hex: dl.archive.sha256,
                }
            } else {
                Checksum::Tofu
            };
            Some(pkg(
                Kind::MongoDb,
                &v.version,
                p,
                "mongodb.com",
                vec![part(dl.archive.url, checksum, 1)],
            ))
        })
        .collect())
}

async fn meilisearch(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let (name, file) = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => ("meilisearch-linux-amd64", "meilisearch"),
        (Os::Linux, Arch::Aarch64) => ("meilisearch-linux-aarch64", "meilisearch"),
        (Os::Macos, Arch::X86_64) => ("meilisearch-macos-amd64", "meilisearch"),
        (Os::Macos, Arch::Aarch64) => ("meilisearch-macos-apple-silicon", "meilisearch"),
        (Os::Windows, Arch::X86_64) => ("meilisearch-windows-amd64.exe", "meilisearch.exe"),
        (Os::Windows, Arch::Aarch64) => bail!("no Meilisearch build for Windows on ARM"),
    };
    let mut out = Vec::new();
    for rel in github_releases(c, "meilisearch/meilisearch", 12).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        if let Some(a) = rel.assets.iter().find(|a| a.name == name) {
            out.push(pkg(
                Kind::Meilisearch,
                &v,
                p,
                "github",
                vec![file_part(
                    a.browser_download_url.clone(),
                    gh_checksum(a),
                    file,
                )],
            ));
        }
    }
    Ok(out)
}

async fn qdrant(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let name = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "qdrant-x86_64-unknown-linux-musl.tar.gz",
        (Os::Linux, Arch::Aarch64) => "qdrant-aarch64-unknown-linux-musl.tar.gz",
        (Os::Macos, Arch::X86_64) => "qdrant-x86_64-apple-darwin.tar.gz",
        (Os::Macos, Arch::Aarch64) => "qdrant-aarch64-apple-darwin.tar.gz",
        (Os::Windows, Arch::X86_64) => "qdrant-x86_64-pc-windows-msvc.zip",
        (Os::Windows, Arch::Aarch64) => bail!("no Qdrant build for Windows on ARM"),
    };
    let mut out = Vec::new();
    for rel in github_releases(c, "qdrant/qdrant", 12).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        if let Some(a) = rel.assets.iter().find(|a| a.name == name) {
            out.push(pkg(
                Kind::Qdrant,
                &v,
                p,
                "github",
                vec![part(a.browser_download_url.clone(), gh_checksum(a), 0)],
            ));
        }
    }
    Ok(out)
}

/// Eclipse Temurin via the Adoptium API: one call per LTS line, sha256 included.
async fn java(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Rel {
        binaries: Vec<Bin>,
        version_data: VersionData,
    }
    #[derive(Deserialize)]
    struct Bin {
        package: Pkg,
    }
    #[derive(Deserialize)]
    struct Pkg {
        link: String,
        checksum: String,
    }
    #[derive(Deserialize)]
    struct VersionData {
        major: u32,
        minor: u32,
        security: u32,
    }
    let (os, arch) = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => ("linux", "x64"),
        (Os::Linux, Arch::Aarch64) => ("linux", "aarch64"),
        (Os::Macos, Arch::X86_64) => ("mac", "x64"),
        (Os::Macos, Arch::Aarch64) => ("mac", "aarch64"),
        (Os::Windows, Arch::X86_64) => ("windows", "x64"),
        (Os::Windows, Arch::Aarch64) => ("windows", "aarch64"),
    };
    let mut out = Vec::new();
    for lts in [25u32, 21, 17, 11, 8] {
        let url = format!(
            "https://api.adoptium.net/v3/assets/feature_releases/{lts}/ga?architecture={arch}&os={os}&image_type=jdk&jvm_impl=hotspot&vendor=eclipse&page_size=1"
        );
        let Ok(text) = fetch_text(c, &url).await else {
            continue;
        };
        let Ok(rels) = serde_json::from_str::<Vec<Rel>>(&text) else {
            continue;
        };
        for r in rels {
            let Some(b) = r.binaries.into_iter().next() else {
                continue;
            };
            let v = format!(
                "{}.{}.{}",
                r.version_data.major, r.version_data.minor, r.version_data.security
            );
            out.push(pkg(
                Kind::Java,
                &v,
                p,
                "adoptium.net",
                vec![part(
                    b.package.link,
                    Checksum::Sha256 {
                        hex: b.package.checksum,
                    },
                    1,
                )],
            ));
        }
    }
    Ok(out)
}

/// Deno ships `<asset>.sha256sum` next to every zip.
async fn deno(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let triple = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "x86_64-unknown-linux-gnu",
        (Os::Linux, Arch::Aarch64) => "aarch64-unknown-linux-gnu",
        (Os::Macos, Arch::X86_64) => "x86_64-apple-darwin",
        (Os::Macos, Arch::Aarch64) => "aarch64-apple-darwin",
        (Os::Windows, Arch::X86_64) => "x86_64-pc-windows-msvc",
        (Os::Windows, Arch::Aarch64) => "aarch64-pc-windows-msvc",
    };
    let name = format!("deno-{triple}.zip");
    let mut out = Vec::new();
    for rel in github_releases(c, "denoland/deno", 12).await? {
        let v = rel.tag_name.trim_start_matches('v').to_string();
        let Some(a) = rel.assets.iter().find(|a| a.name == name) else {
            continue;
        };
        let checksum = match rel
            .assets
            .iter()
            .find(|s| s.name == format!("{name}.sha256sum"))
        {
            Some(s) => Checksum::Remote {
                url: s.browser_download_url.clone(),
                name: name.clone(),
            },
            None => gh_checksum(a),
        };
        out.push(pkg(
            Kind::Deno,
            &v,
            p,
            "github",
            vec![part(a.browser_download_url.clone(), checksum, 0)],
        ));
    }
    Ok(out)
}

/// ziglang.org/download/index.json: every release with tarball + sha256 per target.
async fn zig(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    let target = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "x86_64-linux",
        (Os::Linux, Arch::Aarch64) => "aarch64-linux",
        (Os::Macos, Arch::X86_64) => "x86_64-macos",
        (Os::Macos, Arch::Aarch64) => "aarch64-macos",
        (Os::Windows, Arch::X86_64) => "x86_64-windows",
        (Os::Windows, Arch::Aarch64) => "aarch64-windows",
    };
    let text = fetch_text(c, "https://ziglang.org/download/index.json").await?;
    let idx: serde_json::Value = serde_json::from_str(&text).context("parsing zig index")?;
    let mut out = Vec::new();
    if let Some(map) = idx.as_object() {
        for (ver, entry) in map {
            if ver == "master" || !ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            let Some(t) = entry.get(target) else {
                continue;
            };
            let (Some(url), Some(sha)) = (
                t.get("tarball").and_then(|v| v.as_str()),
                t.get("shasum").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            out.push(pkg(
                Kind::Zig,
                ver,
                p,
                "ziglang.org",
                vec![part(
                    url.to_string(),
                    Checksum::Sha256 {
                        hex: sha.to_string(),
                    },
                    1,
                )],
            ));
        }
    }
    Ok(out)
}

/// .NET SDK: releases-index lists channels; each channel's releases.json carries file hashes.
async fn dotnet(c: &reqwest::Client, p: Platform) -> Result<Vec<Package>> {
    #[derive(Deserialize)]
    struct Index {
        #[serde(rename = "releases-index")]
        channels: Vec<Channel>,
    }
    #[derive(Deserialize)]
    struct Channel {
        #[serde(rename = "support-phase")]
        phase: String,
        #[serde(rename = "releases.json")]
        releases: String,
    }
    #[derive(Deserialize)]
    struct Releases {
        releases: Vec<Rel>,
    }
    #[derive(Deserialize)]
    struct Rel {
        sdk: Sdk,
    }
    #[derive(Deserialize)]
    struct Sdk {
        version: String,
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        name: String,
        rid: String,
        url: String,
        #[serde(default)]
        hash: String,
    }
    let rid = match (p.os, p.arch) {
        (Os::Linux, Arch::X86_64) => "linux-x64",
        (Os::Linux, Arch::Aarch64) => "linux-arm64",
        (Os::Macos, Arch::X86_64) => "osx-x64",
        (Os::Macos, Arch::Aarch64) => "osx-arm64",
        (Os::Windows, Arch::X86_64) => "win-x64",
        (Os::Windows, Arch::Aarch64) => "win-arm64",
    };
    let text = fetch_text(
        c,
        "https://builds.dotnet.microsoft.com/dotnet/release-metadata/releases-index.json",
    )
    .await?;
    let idx: Index = serde_json::from_str(&text).context("parsing releases-index")?;
    let mut out = Vec::new();
    for ch in idx
        .channels
        .into_iter()
        .filter(|ch| matches!(ch.phase.as_str(), "active" | "maintenance"))
        .take(3)
    {
        let Ok(text) = fetch_text(c, &ch.releases).await else {
            continue;
        };
        let Ok(rels) = serde_json::from_str::<Releases>(&text) else {
            continue;
        };
        let Some(r) = rels.releases.into_iter().next() else {
            continue;
        };
        let Some(f) =
            r.sdk.files.into_iter().find(|f| {
                f.rid == rid && (f.name.ends_with(".tar.gz") || f.name.ends_with(".zip"))
            })
        else {
            continue;
        };
        let checksum = match f.hash.len() {
            128 => Checksum::Sha512 { hex: f.hash },
            64 => Checksum::Sha256 { hex: f.hash },
            _ => Checksum::Tofu,
        };
        out.push(pkg(
            Kind::DotNet,
            &r.sdk.version,
            p,
            "microsoft.com",
            vec![part(f.url, checksum, 0)],
        ));
    }
    Ok(out)
}
