//! `padu` command line interface. Also used as the elevated helper for hosts-file writes.

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use padu_core::config::WebServer;
use padu_core::manifest::{Kind, Manifest};
use padu_core::packages::Registry;
use padu_core::projects::Template;
use padu_core::services::{self, ServiceId};
use padu_core::{Layout, Padu, Platform};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "padu", version = padu_core::VERSION, about = "Padu - lightweight, secure local web development stack")]
struct Cli {
    /// Machine readable output where supported.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the installation root (default: ~/padu) and its configuration.
    Init {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Show service status.
    Status,
    /// Start services (all enabled ones when no service is given).
    Start { service: Option<String> },
    /// Stop services.
    Stop { service: Option<String> },
    /// Restart services.
    Restart { service: Option<String> },
    /// Reload the web server configuration (picks up new sites).
    Reload,
    /// Download and install a runtime, e.g. `php`, `php@8.4.27-1`, `caddy`, `mariadb`.
    Install {
        /// kind or kind@version
        package: String,
        /// Ask upstream for the newest version instead of the built-in list.
        #[arg(long)]
        latest: bool,
    },
    /// List installed and available packages.
    Packages {
        /// Query upstream sources for newer versions (network).
        #[arg(long)]
        check: bool,
    },
    /// Select the active version of a runtime (`padu use php 8.4.27-1`).
    Use { kind: String, version: String },
    /// Remove an installed runtime version.
    Remove { kind: String, version: String },
    /// List sites; `--sync` regenerates configs, certificates and the hosts file.
    Sites {
        #[arg(long)]
        sync: bool,
    },
    /// Manage manual sites.
    Site {
        #[command(subcommand)]
        cmd: SiteCmd,
    },
    /// Hosts file helpers.
    Hosts {
        #[command(subcommand)]
        cmd: HostsCmd,
    },
    /// Create or delete projects in the document root.
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Database helpers.
    Db {
        #[command(subcommand)]
        cmd: DbCmd,
    },
    /// Local HTTPS certificate helpers.
    Ssl {
        #[command(subcommand)]
        cmd: SslCmd,
    },
    /// Show or change configuration.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Open things: web, db, mail, root, logs, or a site name.
    Open { what: String },
    /// Open a terminal with every runtime on PATH.
    Terminal,
    /// Check the environment for common problems.
    Doctor {
        /// Allow the web server to bind ports 80/443 on Linux (uses setcap, asks for your password).
        #[arg(long)]
        fix_ports: bool,
    },
    /// Check GitHub for a newer Padu release.
    Update,
    /// Dump all databases now.
    Backup,
}

#[derive(Subcommand)]
enum SiteCmd {
    Add {
        hostname: String,
        path: PathBuf,
        #[arg(long, default_value = "")]
        public: String,
    },
    Remove {
        hostname: String,
    },
}

#[derive(Subcommand)]
enum HostsCmd {
    /// Write the managed block (asks for administrator rights).
    Sync,
    /// Print the hosts that Padu wants in the hosts file.
    Show,
    /// Internal: apply a validated payload file. Runs elevated.
    #[command(hide = true)]
    Apply {
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    Create {
        name: String,
        /// blank | wordpress | laravel | symfony | composer=vendor/name | git=https://...
        #[arg(long, default_value = "blank")]
        template: String,
        /// Also create a database with the project name.
        #[arg(long)]
        db: bool,
    },
    Delete {
        name: String,
    },
    List,
}

#[derive(Subcommand)]
enum DbCmd {
    Create {
        name: String,
    },
    Drop {
        name: String,
    },
    List,
    /// Print the root password.
    Password,
    /// Set a new root password (generates one when omitted).
    SetPassword {
        new: Option<String>,
    },
    Backup,
}

#[derive(Subcommand)]
enum SslCmd {
    /// Import the local CA into the user's trust stores.
    Trust,
    /// Create a new CA (existing site certificates are re-issued).
    Regenerate,
    /// Print the CA certificate path.
    Path,
}

#[derive(Subcommand)]
enum ConfigCmd {
    Show,
    /// Set a value, e.g. `padu config set web.http_port 8080`.
    Set {
        key: String,
        value: String,
    },
    /// Print the config file path.
    Path,
}

#[derive(Args)]
struct Empty;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .without_time()
        .init();
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("runtime");
    if let Err(e) = rt.block_on(run(cli)) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Init { root } => {
            let app = match root {
                Some(r) => {
                    let app = Padu::open_at(r)?;
                    app.layout.remember()?;
                    app
                }
                None => Padu::open()?,
            };
            padu_core::tls::ensure_ca(&app.layout)?;
            let www = app.config.document_root(&app.layout);
            let index = www.join("index.php");
            if !index.exists() {
                std::fs::write(&index, WELCOME_PHP)?;
            }
            println!("Padu root: {}", app.layout.root().display());
            println!("Config:     {}", app.layout.config_file().display());
            println!(
                "Next: padu install php && padu install caddy && padu install mariadb && padu start"
            );
        }
        Cmd::Status => {
            let app = Padu::open()?;
            let st = services::status_all(&app)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                println!(
                    "{:<8} {:<10} {:<9} {:<9} {:<18} {:<12} NOTE",
                    "SERVICE", "ENGINE", "ENABLED", "STATE", "VERSION", "PORTS"
                );
                for s in st {
                    let ports = s
                        .ports
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    println!(
                        "{:<8} {:<10} {:<9} {:<9} {:<18} {:<12} {}",
                        s.id,
                        s.name,
                        if s.enabled { "yes" } else { "no" },
                        if s.running { "running" } else { "stopped" },
                        s.version,
                        ports,
                        s.message
                    );
                }
            }
        }
        Cmd::Start { service } => {
            let app = Padu::open()?;
            match service {
                Some(s) => {
                    services::start(&app, s.parse()?)?;
                    println!("started {s}");
                }
                None => {
                    let missing = services::prepare_all(&app)?;
                    report(services::start_all(&app));
                    if !missing.is_empty() {
                        println!("hosts file is out of date; run: padu hosts sync");
                    }
                }
            }
        }
        Cmd::Stop { service } => {
            let app = Padu::open()?;
            match service {
                Some(s) => {
                    services::stop(&app, s.parse()?)?;
                    println!("stopped {s}");
                }
                None => report(services::stop_all(&app)),
            }
        }
        Cmd::Restart { service } => {
            let app = Padu::open()?;
            match service {
                Some(s) => services::restart(&app, s.parse()?)?,
                None => {
                    report(services::stop_all(&app));
                    report(services::start_all(&app));
                }
            }
        }
        Cmd::Reload => {
            let app = Padu::open()?;
            services::reload(&app, ServiceId::Web)?;
            println!("web server reloaded");
        }
        Cmd::Install { package, latest } => {
            let app = Padu::open()?;
            let (kind, version) = match package.split_once('@') {
                Some((k, v)) => (k.parse::<Kind>()?, Some(v.to_string())),
                None => (package.parse::<Kind>()?, None),
            };
            let p = Platform::current();
            let pkg = if latest && version.is_none() {
                padu_core::resolve::latest(kind, p).await?
            } else {
                let m = Manifest::builtin()?;
                match version {
                    Some(v) => m.versions(kind, p).into_iter().find(|x| x.version == v).cloned().with_context(|| format!("{kind} {v} is not in the built-in list; try --latest or omit the version"))?,
                    None => m.latest(kind, p).cloned().with_context(|| format!("no {kind} package for {p}"))?,
                }
            };
            println!(
                "installing {} {} from {}",
                pkg.kind.display(),
                pkg.version,
                pkg.source
            );
            // Throttled progress: live line on a terminal, one line per 10% otherwise.
            let tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
            let last = std::sync::Mutex::new((std::time::Instant::now(), 0u64));
            let progress = move |pr: padu_core::download::Progress| {
                let mut l = last.lock().unwrap();
                let pct = pr.total.map(|t| pr.downloaded * 100 / t.max(1));
                let step = if tty { 1 } else { 10 };
                let due = pct
                    .map(|p| p / step != l.1 / step)
                    .unwrap_or(l.0.elapsed().as_millis() > 500)
                    || Some(pr.downloaded) == pr.total;
                if !due {
                    return;
                }
                l.0 = std::time::Instant::now();
                l.1 = pct.unwrap_or(0);
                let mb = pr.downloaded as f64 / 1048576.0;
                match (pr.total, tty) {
                    (Some(t), true) => eprint!(
                        "\r  {:>3}% of {:.1} MiB",
                        pct.unwrap_or(0),
                        t as f64 / 1048576.0
                    ),
                    (Some(t), false) => eprintln!(
                        "  {:>3}% of {:.1} MiB",
                        pct.unwrap_or(0),
                        t as f64 / 1048576.0
                    ),
                    (None, true) => eprint!("\r  {mb:.1} MiB"),
                    (None, false) => eprintln!("  {mb:.1} MiB"),
                }
            };
            let installed =
                padu_core::packages::install(&app.layout, &pkg, Some(&progress)).await?;
            eprintln!();
            println!(
                "installed {} {} -> {}",
                installed.kind.display(),
                installed.version,
                installed.path.display()
            );
            post_install_hints(&app, kind, &installed.path)?;
        }
        Cmd::Packages { check } => {
            let app = Padu::open()?;
            let reg = Registry::load(&app.layout)?;
            let p = Platform::current();
            let builtin = Manifest::builtin()?;
            let live = if check {
                Some(padu_core::resolve::latest_all(p).await)
            } else {
                None
            };
            println!(
                "{:<12} {:<20} {:<20} {}",
                "KIND",
                "INSTALLED",
                "BUILT-IN",
                if check { "UPSTREAM" } else { "" }
            );
            for kind in Kind::ALL {
                let installed = reg
                    .list(kind)
                    .iter()
                    .map(|i| i.version.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                let b = builtin
                    .latest(kind, p)
                    .map(|x| x.version.clone())
                    .unwrap_or_else(|| "-".into());
                let l = live
                    .as_ref()
                    .and_then(|m| m.latest(kind, p))
                    .map(|x| x.version.clone())
                    .unwrap_or_default();
                println!(
                    "{:<12} {:<20} {:<20} {}",
                    kind.id(),
                    if installed.is_empty() {
                        "-".into()
                    } else {
                        installed
                    },
                    b,
                    l
                );
            }
        }
        Cmd::Use { kind, version } => {
            let mut app = Padu::open()?;
            let kind: Kind = kind.parse()?;
            let reg = Registry::load(&app.layout)?;
            reg.find(kind, &version)
                .with_context(|| format!("{kind} {version} is not installed"))?;
            match kind {
                Kind::Php => app.config.php.version = version.clone(),
                Kind::Caddy | Kind::Nginx | Kind::Apache => {
                    app.config.web.server = match kind {
                        Kind::Caddy => WebServer::Caddy,
                        Kind::Nginx => WebServer::Nginx,
                        _ => WebServer::Apache,
                    };
                    app.config.web.version = version.clone();
                }
                Kind::MariaDb | Kind::MySql => {
                    app.config.db.engine = if kind == Kind::MariaDb {
                        padu_core::config::DbEngine::MariaDb
                    } else {
                        padu_core::config::DbEngine::MySql
                    };
                    app.config.db.version = version.clone();
                }
                Kind::Cache => app.config.cache.version = version.clone(),
                Kind::Mailpit => app.config.mail.version = version.clone(),
                _ => bail!("{kind} has no active-version setting"),
            }
            app.save_config()?;
            println!("{kind} -> {version}. Restart the affected service to apply.");
        }
        Cmd::Remove { kind, version } => {
            let app = Padu::open()?;
            padu_core::packages::remove(&app.layout, kind.parse()?, &version)?;
            println!("removed {kind} {version}");
        }
        Cmd::Sites { sync } => {
            let app = Padu::open()?;
            if sync {
                let missing = services::prepare_all(&app)?;
                if services::status_all(&app)?
                    .iter()
                    .any(|s| s.id == ServiceId::Web && s.running)
                {
                    services::reload(&app, ServiceId::Web)?;
                }
                if !missing.is_empty() {
                    hosts_sync(&app)?;
                }
            }
            let sites = padu_core::sites::discover(&app.layout, &app.config)?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&sites)?);
            } else {
                let scheme = if app.config.web.ssl { "https" } else { "http" };
                for s in sites {
                    println!(
                        "{scheme}://{:<30} {}{}",
                        s.hostname,
                        s.root.display(),
                        if s.auto { "" } else { "  (manual)" }
                    );
                }
            }
        }
        Cmd::Site { cmd } => {
            let app = Padu::open()?;
            match cmd {
                SiteCmd::Add {
                    hostname,
                    path,
                    public,
                } => {
                    padu_core::sites::add_manual(
                        &app.layout,
                        &app.config,
                        &hostname,
                        &path,
                        &public,
                    )?;
                    println!("added {hostname}. Run: padu sites --sync");
                }
                SiteCmd::Remove { hostname } => {
                    if padu_core::sites::remove_manual(&app.layout, &hostname)? {
                        println!("removed {hostname}. Run: padu sites --sync");
                    } else {
                        println!("{hostname} was not a manual site");
                    }
                }
            }
        }
        Cmd::Hosts { cmd } => match cmd {
            HostsCmd::Apply { from } => {
                // Elevated helper: no config access, just validate + write.
                let hosts_file = Platform::current().hosts_file();
                padu_core::hosts::apply_payload(&hosts_file, &from)?;
                let _ = std::fs::remove_file(&from);
            }
            HostsCmd::Sync => {
                let app = Padu::open()?;
                hosts_sync(&app)?;
            }
            HostsCmd::Show => {
                let app = Padu::open()?;
                let sites = padu_core::sites::discover(&app.layout, &app.config)?;
                print!(
                    "{}",
                    padu_core::hosts::render_block(&padu_core::sites::hostnames(&sites))
                );
            }
        },
        Cmd::Project { cmd } => {
            let app = Padu::open()?;
            match cmd {
                ProjectCmd::Create { name, template, db } => {
                    let t = match template.as_str() {
                        "blank" => Template::Blank,
                        "wordpress" => Template::WordPress,
                        "laravel" => Template::Laravel,
                        "symfony" => Template::Symfony,
                        other => match other.split_once('=') {
                            Some(("composer", p)) => Template::Composer { package: p.into() },
                            Some(("git", u)) => Template::Git { url: u.into() },
                            _ => bail!("unknown template {other}"),
                        },
                    };
                    let path = padu_core::projects::create(&app, &name, &t, None).await?;
                    println!("created {}", path.display());
                    if db {
                        let reg = Registry::load(&app.layout)?;
                        let dbctx = padu_core::db::DbCtx::from_app(&app, &reg)
                            .context("database not installed")?;
                        padu_core::db::create_database(&dbctx, &name.replace('-', "_"))?;
                        println!("created database {}", name.replace('-', "_"));
                    }
                    let missing = services::prepare_all(&app)?;
                    if services::status_all(&app)?
                        .iter()
                        .any(|s| s.id == ServiceId::Web && s.running)
                    {
                        services::reload(&app, ServiceId::Web)?;
                    }
                    if !missing.is_empty() {
                        hosts_sync(&app)?;
                    }
                    println!(
                        "{}://{}",
                        if app.config.web.ssl { "https" } else { "http" },
                        app.config.hostname_for(&name)
                    );
                }
                ProjectCmd::Delete { name } => {
                    padu_core::projects::delete(&app, &name)?;
                    println!("deleted {name}. Run: padu sites --sync");
                }
                ProjectCmd::List => {
                    for (id, label) in padu_core::projects::TEMPLATES {
                        println!("{id:<10} {label}");
                    }
                }
            }
        }
        Cmd::Db { cmd } => {
            let app = Padu::open()?;
            let reg = Registry::load(&app.layout)?;
            let db = padu_core::db::DbCtx::from_app(&app, &reg)
                .context("database is not installed (padu install mariadb)")?;
            match cmd {
                DbCmd::Create { name } => {
                    padu_core::db::create_database(&db, &name)?;
                    println!("created {name}");
                }
                DbCmd::Drop { name } => {
                    padu_core::db::drop_database(&db, &name)?;
                    println!("dropped {name}");
                }
                DbCmd::List => {
                    for d in padu_core::db::list_databases(&db)? {
                        println!("{d}");
                    }
                }
                DbCmd::Password => match padu_core::db::root_password(&app.layout)? {
                    Some(p) => println!("{p}"),
                    None => println!("(no password stored yet; start the database once)"),
                },
                DbCmd::SetPassword { new } => {
                    let current = padu_core::db::root_password(&app.layout)?;
                    let new = new.unwrap_or_else(|| padu_core::db::random_token(24));
                    padu_core::db::set_root_password(&db, current.as_deref(), &new)?;
                    println!("root password updated: {new}");
                }
                DbCmd::Backup => {
                    let p = padu_core::backup::run(&app)?;
                    println!("backup written to {}", p.display());
                }
            }
        }
        Cmd::Ssl { cmd } => {
            let app = Padu::open()?;
            match cmd {
                SslCmd::Trust => {
                    padu_core::tls::ensure_ca(&app.layout)?;
                    for step in padu_core::tls::trust_commands(&app.layout) {
                        println!("-> {}", step.description);
                        let result = if step.needs_admin {
                            padu_core::tools::run_elevated(
                                std::path::Path::new(&step.program),
                                &step.args,
                            )
                        } else {
                            std::process::Command::new(&step.program)
                                .args(&step.args)
                                .status()
                                .map_err(Into::into)
                                .and_then(|s| {
                                    if s.success() {
                                        Ok(())
                                    } else {
                                        bail!("{} exited with {s}", step.program)
                                    }
                                })
                        };
                        match result {
                            Ok(()) => println!("   ok"),
                            Err(e) => println!("   skipped: {e:#}"),
                        }
                    }
                }
                SslCmd::Regenerate => {
                    let p = padu_core::tls::paths(&app.layout);
                    let _ = std::fs::remove_file(&p.ca_crt);
                    let _ = std::fs::remove_file(&p.ca_key);
                    padu_core::tls::ensure_ca(&app.layout)?;
                    services::prepare_all(&app)?;
                    println!(
                        "new CA generated at {}. Run: padu ssl trust",
                        p.ca_crt.display()
                    );
                }
                SslCmd::Path => {
                    println!("{}", padu_core::tls::paths(&app.layout).ca_crt.display())
                }
            }
        }
        Cmd::Config { cmd } => {
            let mut app = Padu::open()?;
            match cmd {
                ConfigCmd::Show => print!("{}", toml::to_string_pretty(&app.config)?),
                ConfigCmd::Set { key, value } => {
                    let mut doc: toml::Value = toml::Value::try_from(&app.config)?;
                    let (section, field) =
                        key.split_once('.').context("key must be section.field")?;
                    let table = doc
                        .get_mut(section)
                        .and_then(|v| v.as_table_mut())
                        .with_context(|| format!("unknown section {section}"))?;
                    let current = table
                        .get(field)
                        .with_context(|| format!("unknown key {key}"))?;
                    let new = match current {
                        toml::Value::String(_) => toml::Value::String(value),
                        toml::Value::Integer(_) => toml::Value::Integer(value.parse()?),
                        toml::Value::Boolean(_) => toml::Value::Boolean(value.parse()?),
                        _ => bail!("unsupported value type for {key}"),
                    };
                    table.insert(field.to_string(), new);
                    app.config = doc.try_into()?;
                    app.save_config()?;
                    println!("{key} = {}", table_value(&app.config, section, field));
                }
                ConfigCmd::Path => println!("{}", app.layout.config_file().display()),
            }
        }
        Cmd::Open { what } => {
            let app = Padu::open()?;
            let cfg = &app.config;
            let scheme = if cfg.web.ssl { "https" } else { "http" };
            let port = if cfg.web.ssl {
                cfg.web.https_port
            } else {
                cfg.web.http_port
            };
            let port_s = if (scheme == "https" && port == 443) || (scheme == "http" && port == 80) {
                String::new()
            } else {
                format!(":{port}")
            };
            match what.as_str() {
                "web" | "www" => {
                    padu_core::tools::open_url(cfg, &format!("{scheme}://localhost{port_s}/"))?
                }
                "db" | "adminer" => padu_core::tools::open_url(
                    cfg,
                    &format!("{scheme}://localhost{port_s}/adminer/"),
                )?,
                "phpmyadmin" => padu_core::tools::open_url(
                    cfg,
                    &format!("{scheme}://localhost{port_s}/phpmyadmin/"),
                )?,
                "mail" => padu_core::tools::open_url(
                    cfg,
                    &format!("http://127.0.0.1:{}/", cfg.mail.http_port),
                )?,
                "root" => padu_core::tools::open_path(app.layout.root())?,
                "logs" => padu_core::tools::open_path(&app.layout.logs())?,
                site => padu_core::tools::open_url(
                    cfg,
                    &format!("{scheme}://{}{port_s}/", cfg.hostname_for(site)),
                )?,
            }
        }
        Cmd::Terminal => {
            let app = Padu::open()?;
            let reg = Registry::load(&app.layout)?;
            let path = padu_core::packages::runtime_path(&app.layout, &reg, &app.config);
            padu_core::tools::open_terminal(&app.config.document_root(&app.layout), &path)?;
        }
        Cmd::Doctor { fix_ports } => {
            let app = Padu::open()?;
            doctor(&app, fix_ports)?;
        }
        Cmd::Update => {
            let app = Padu::open()?;
            match padu_core::update::check(&app.config.update.repo).await? {
                Some(r) => println!("new version {} available: {}", r.version, r.url),
                None => println!("Padu {} is up to date", padu_core::VERSION),
            }
        }
        Cmd::Backup => {
            let app = Padu::open()?;
            let p = padu_core::backup::run(&app)?;
            println!("backup written to {}", p.display());
        }
    }
    Ok(())
}

fn table_value(cfg: &padu_core::Config, section: &str, field: &str) -> String {
    toml::Value::try_from(cfg)
        .ok()
        .and_then(|v| v.get(section)?.get(field).map(|x| x.to_string()))
        .unwrap_or_default()
}

fn report(results: Vec<(ServiceId, Result<()>)>) {
    for (id, r) in results {
        match r {
            Ok(()) => println!("{id:<6} ok"),
            Err(e) => println!("{id:<6} FAILED: {e:#}"),
        }
    }
}

fn hosts_sync(app: &Padu) -> Result<()> {
    let sites = padu_core::sites::discover(&app.layout, &app.config)?;
    let hosts = padu_core::sites::hostnames(&sites);
    let hosts_file = Platform::current().hosts_file();
    if padu_core::hosts::is_in_sync(&hosts_file, &hosts) {
        println!("hosts file already up to date");
        return Ok(());
    }
    let payload = app.layout.tmp().join("hosts.payload");
    padu_core::hosts::write_payload(&payload, &hosts, &app.config.general.tld)?;
    let exe = padu_core::tools::current_exe()?;
    println!(
        "updating {} (administrator rights required)",
        hosts_file.display()
    );
    padu_core::tools::run_elevated(
        &exe,
        &[
            "hosts".into(),
            "apply".into(),
            "--from".into(),
            payload.to_string_lossy().into_owned(),
        ],
    )?;
    if padu_core::hosts::is_in_sync(&hosts_file, &hosts) {
        println!("hosts file updated ({} entries)", hosts.len());
        Ok(())
    } else {
        bail!("hosts file was not updated")
    }
}

fn post_install_hints(app: &Padu, kind: Kind, path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    if matches!(kind, Kind::Caddy | Kind::Nginx | Kind::Apache) && app.config.web.http_port < 1024 {
        println!(
            "Linux needs permission to bind ports 80/443 without root. Run: padu doctor --fix-ports"
        );
    }
    let _ = (app, kind, path);
    Ok(())
}

fn doctor(app: &Padu, fix_ports: bool) -> Result<()> {
    let layout: &Layout = &app.layout;
    println!("root        {}", layout.root().display());
    println!("platform    {}", Platform::current());
    println!("config      {}", layout.config_file().display());
    if layout.root().to_string_lossy().contains(' ') {
        println!("WARN        the root path contains spaces; some tools misbehave");
    }
    let reg = Registry::load(layout)?;
    for kind in [Kind::Php, Kind::Caddy, Kind::MariaDb, Kind::Mailpit] {
        match reg.newest(kind) {
            Some(i) => println!("{:<11} {}", kind.id(), i.version),
            None => println!("{:<11} missing -> padu install {}", kind.id(), kind.id()),
        }
    }
    for (port, what) in [
        (app.config.web.http_port, "web http"),
        (app.config.web.https_port, "web https"),
        (app.config.php.fcgi_port, "php"),
        (app.config.db.port, "db"),
        (app.config.mail.smtp_port, "mail smtp"),
        (app.config.mail.http_port, "mail ui"),
    ] {
        let st = services::status_all(app)?;
        let ours = st.iter().any(|s| s.running && s.ports.contains(&port));
        if padu_core::process::port_in_use(port) && !ours {
            println!(
                "WARN        port {port} ({what}) is used by another program: {}",
                padu_core::process::port_owner(port).unwrap_or_default()
            );
        }
    }
    let hosts_file = Platform::current().hosts_file();
    let sites = padu_core::sites::discover(layout, &app.config)?;
    if padu_core::hosts::is_in_sync(&hosts_file, &padu_core::sites::hostnames(&sites)) {
        println!("hosts       in sync ({} sites)", sites.len());
    } else {
        println!("hosts       out of date -> padu hosts sync");
    }
    let tls = padu_core::tls::paths(layout);
    println!(
        "ca          {}{}",
        tls.ca_crt.display(),
        if tls.ca_crt.is_file() {
            ""
        } else {
            " (missing -> padu init)"
        }
    );
    #[cfg(target_os = "linux")]
    {
        if fix_ports {
            let web_reg = Registry::load(layout)?;
            let sites = padu_core::sites::discover(layout, &app.config)?;
            let ctx = services::Ctx {
                app,
                reg: &web_reg,
                sites: &sites,
            };
            match services::web::active(&ctx) {
                Some(server) => {
                    let bin = server.binary(&ctx);
                    padu_core::tools::grant_bind_capability(&bin)?;
                    println!("granted cap_net_bind_service to {}", bin.display());
                }
                None => println!("install a web server first"),
            }
        } else if app.config.web.http_port < 1024 {
            println!(
                "ports       <1024 need: padu doctor --fix-ports (or set web.http_port=8080, web.https_port=8443)"
            );
        }
    }
    let _ = fix_ports;
    Ok(())
}

const WELCOME_PHP: &str = r#"<?php
// Padu welcome page. Replace this folder's content or create projects next to it.
$local = in_array($_SERVER['REMOTE_ADDR'] ?? '', ['127.0.0.1', '::1'], true);
if (($_GET['q'] ?? '') === 'info') { if ($local) { phpinfo(); } else { http_response_code(403); } exit; }
?><!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Padu</title>
<style>
:root{color-scheme:light dark;--bg:#f6f7fb;--fg:#15171f;--card:#fff;--muted:#6b7280;--accent:#2563eb}
@media(prefers-color-scheme:dark){:root{--bg:#0f1117;--fg:#e6e8ef;--card:#171a23;--muted:#9aa0ae;--accent:#60a5fa}}
body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--bg);color:var(--fg);font:16px/1.5 system-ui,sans-serif}
main{background:var(--card);padding:40px 48px;border-radius:16px;box-shadow:0 10px 40px rgba(0,0,0,.08);max-width:560px;width:calc(100% - 32px);box-sizing:border-box}
h1{margin:0 0 8px;font-size:40px;letter-spacing:-.02em}p{margin:6px 0;color:var(--muted)}a{color:var(--accent)}code{font-family:ui-monospace,monospace}
</style></head><body><main>
<h1>Padu</h1>
<p>Your local stack is running.</p>
<?php if ($local): ?>
<p>PHP <?= htmlspecialchars(PHP_VERSION, ENT_QUOTES) ?> &middot; <a href="/?q=info">phpinfo()</a></p>
<p>Server: <?= htmlspecialchars($_SERVER['SERVER_SOFTWARE'] ?? 'unknown', ENT_QUOTES) ?></p>
<p>Document root: <code><?= htmlspecialchars($_SERVER['DOCUMENT_ROOT'] ?? '', ENT_QUOTES) ?></code></p>
<?php endif; ?>
<p>Create a folder here and it becomes <code>https://&lt;folder&gt;.test</code> after <code>padu sites --sync</code>.</p>
</main></body></html>
"#;
