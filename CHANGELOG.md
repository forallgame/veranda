# Changelog

All notable changes to Padu are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

Release checklist: bump `VERSION`, `workspace.package.version` in `Cargo.toml` and `version` in `apps/desktop/src-tauri/tauri.conf.json`, move the
Unreleased notes under a new heading, tag `vX.Y.Z` and push the tag - the release workflow builds
installers for Linux, Windows and macOS and attaches them to the GitHub release.

## [Unreleased]

### Changed
- The official repository (`forallgame/padu`) is the default update source; the GitHub repo
  setting is now optional and only needed for forks.

## [0.1.1] - 2026-11-03

### Fixed
- Smoother scrolling: the UI only re-renders when data actually changed, and the Linux build
  disables WebKitGTK's DMA-BUF renderer, which caused stuttering on many GPUs.
- Launching Padu while it is already running (for example from the tray) now focuses the existing
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
- Core engine (`padu-core`): configuration, package manifest with verified versions, HTTPS-only
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
- `padu` CLI covering every feature, plus `doctor` and self-update check against GitHub releases.
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
