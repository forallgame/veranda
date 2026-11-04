# Changelog

All notable changes to Veranda are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

Release checklist: bump `VERSION`, `workspace.package.version` in `Cargo.toml` and `version` in `apps/desktop/src-tauri/tauri.conf.json`, move the
Unreleased notes under a new heading, tag `vX.Y.Z` and push the tag - the release workflow builds
installers for Linux, Windows and macOS and attaches them to the GitHub release.

## [Unreleased]

## [0.4.1] - 2026-11-04

### Fixed
- CI on Windows: the FlyEnv import test wrote a Windows path into hand-made JSON (invalid escape).
- Installed packages could "disappear" from the app when `etc/installed.json` held an entry an
  older build could not read (for example after running a newer build on the same root): the
  whole file was discarded and the next install overwrote it. The registry now keeps entries it
  cannot read, repairs moved paths, and re-registers every package folder under `bin/` that the
  file does not mention, so packages on disk are never shown as uninstalled.

### Changed
- Version lists show a clear loading state while upstream is queried: a progress bar on the table,
  shimmering placeholder rows and a status line; new rows fade in when the list arrives.

## [0.4.0] - 2026-11-04

### Added
- **PostgreSQL** as a second database service (runs beside MariaDB/MySQL): official binaries via
  the zonky.io Maven packages with SHA-256 checks, data in `data/postgres`, superuser `postgres`
  with a generated password, loopback only, Adminer link with the PostgreSQL driver preselected.
- **Python** (python-build-standalone), **Go** (go.dev) and **Bun** (GitHub) as downloadable
  runtimes with upstream checksums; they are added to the terminal PATH.

### Fixed
- Linux: after granting port 80/443 permission, the web server still refused to start because
  Veranda probed the port from its own (unprivileged) process. The probe now trusts a binary that
  carries `cap_net_bind_service`, so "Allow" works in one go.

### Added
- When another program holds a port a service needs, Veranda names the program and offers to
  move the service to the next free port (80 -> 8080, 443 -> 8443, others +1) in one click.

### Changed
- New app icon: a glass tile with a house roof over a server stack (artwork supplied by the
  project owner), used for the window, tray, installers and the in-app brand.
- Sidebar and dashboard switches now start or stop the service immediately (as in FlyEnv) and
  mark it to come up when Veranda launches. Hint texts updated.
- Node.js: versions installed by nvm, nvm-windows, fnm or volta are detected and listed on the
  Node.js page (with their folder) instead of being offered for download again.

## [0.3.0] - 2026-11-04

### Added
- **Migrate** (Sites page): import projects from Laragon, XAMPP, FlyEnv or any folder, linked in
  place or copied into the document root, and optionally copy databases from the source server
  through Veranda's own `mysqldump`/client (the source stack must be running).

### Changed
- Scrolling is handled by the window again instead of an inner pane, which the compositor
  accelerates; the sidebar is sticky. Measured with the built-in benchmark (`VERANDA_UI_PERF=1`):
  60 fps with GPU rendering on, 19 fps with it off, so GPU rendering stays the default.
- Version tables use fixed status/action columns so rows line up; service-page actions align right.

## [0.2.0] - 2026-11-03

### Changed
- **Renamed to Veranda** (formerly Padu). Binaries, package names, the app identifier
  (`dev.veranda.desktop`), the config file (`etc/veranda.toml`), the `VERANDA_HOME` variable and
  the default root (`~/veranda`) all follow the new name. An existing `~/padu` root is moved to
  `~/veranda` automatically on first start when no service is running, and the old managed block
  in the hosts file is replaced. Uninstall the old Padu package after installing Veranda.
- New icon and a service-centred layout: every service and tool has its own page with a
  Service tab (status, ports, actions, files) and a Versions tab (install / remove any version),
  and the sidebar switches choose which services the dashboard starts.
- Linux: GPU rendering is on by default (was forced off); `general.hardware_acceleration = false`
  turns it off for drivers that flicker. The window appears only after the first render, with a
  dark background, so there is no white flash on launch.

## [0.1.3] - 2026-11-03

### Fixed
- Linux: a privileged port (80/443) that the OS refused to bind was reported as "already in use".
  It is now detected separately and the desktop app offers the one-time permission
  (`setcap`, asks for your password) or a switch to 8080/8443.
- MySQL/MariaDB on Debian/Ubuntu 24.04+: the generic tarball needs `libaio.so.1`, which the distro
  ships as `libaio.so.1t64`. Veranda now links it into the package's private lib folder; if libaio is
  missing entirely, the error says which package to install.

### Changed
- Dashboard: every service shows a clear Running / Stopped / Starting… / Stopping… badge, running
  rows are highlighted, buttons show progress while a request runs, and results read
  "PHP stopped" / "All services stopped" instead of "OK". Stop errors have their own message.

## [0.1.2] - 2026-11-03

### Changed
- Updates always come from the official repository (`forallgame/veranda`); the GitHub repo field was
  removed from Settings (forks can still set `update.repo` in `veranda.toml`).

### Fixed
- Dashboard: the on/off switch of a service whose package is not installed is now shown off and
  locked, so it no longer looks as if the package were installed and enabled.

## [0.1.1] - 2026-11-03

### Fixed
- Smoother scrolling: the UI only re-renders when data actually changed, and the Linux build
  disables WebKitGTK's DMA-BUF renderer, which caused stuttering on many GPUs.
- Launching Veranda while it is already running (for example from the tray) now focuses the existing
  window instead of starting a second process with a second tray icon.
- README download commands use the real asset names.

### Added
- Dashboard customization (slider button): choose the web server, the database engine and which
  services are shown. Hidden services are not started by Start All.
- Close behaviour setting: ask every time (default, with "remember my choice"), hide to tray, or
  quit. Changeable later in Settings.

### Changed
- Redesigned buttons, switches, tiles and navigation highlight.

## [0.1.0] - 2026-10-31

### Added
- Core engine (`veranda-core`): configuration, package manifest with verified versions, HTTPS-only
  downloader with SHA-256/SHA-512/MD5/remote checksum verification and a trust-on-first-use ledger,
  zip/tar.gz/tar.xz extraction with path-traversal protection, process supervision with pid files.
- Services: Caddy (default), Nginx and Apache front-ends; PHP-FPM (Linux/macOS) and php-cgi workers
  (Windows); MariaDB and MySQL with automatic initialisation and random root password; Valkey/Redis;
  Mailpit.
- Automatic virtual hosts (`<folder>.test`) with `public/` detection, manual sites, hosts-file
  management through a validating elevated helper.
- Local certificate authority (ECDSA P-256) and site certificates; trust-store import commands for
  the current user on all three platforms.
- Run-time version discovery for PHP (php.net / static-php), Caddy, Mailpit, MariaDB, MySQL,
  Node.js, Composer, Adminer, phpMyAdmin, Nginx, Apache, Valkey/Redis and the Mozilla CA bundle.
- Project creation: blank, WordPress, Laravel, Symfony, any Composer template, git clone.
- Database helpers: create/drop/list, password rotation, full dumps with rotation.
- `veranda` CLI covering every feature, plus `doctor` and self-update check against GitHub releases.
- Tauri 2 desktop app with dashboard, packages, sites, database and settings pages, tray icon,
  light/dark/system theme, English and Indonesian UI, background backups.
- Desktop: Logs tab (per-service log tail with error/warning highlighting, clear, open folder) and an
  in-app activity history; packages grouped by category with a version picker (built-in or newest
  upstream); per-version uninstall with in-app confirmation (blocked while the service runs);
  inline download/extract progress per package; compact animated notifications.
- In-app auto-update (Tauri signed updater; GitHub Releases `latest.json`; Windows, macOS and
  Linux AppImage), with GitHub-API fallback for installs that cannot self-update.
- Released under the MIT License; README in English and Indonesian with per-OS install guides.
- Legal hygiene: clean-room implementation, permissive-only dependency audit, `THIRD_PARTY_NOTICES.md`
  generated from `cargo metadata`, runtime licenses documented, no bundled third-party assets or fonts.
- CI workflow (fmt, clippy, tests on Linux/Windows/macOS) and release workflow (installers on tag).
