# Using Veranda

A walkthrough of every screen. Bahasa Indonesia: [USAGE.id.md](USAGE.id.md).

Veranda has two halves. The **local stack** (Overview, Sites, Database, Logs and the service
pages) runs web servers, languages and databases on your own computer. The **Remote** group is an
SSH client for the servers you manage elsewhere. Nothing in either half leaves your computer
unless you connect to a server you chose; there are no accounts and no telemetry.

## 1. First start

1. Install Veranda ([README](../README.md#install)) and open it. The window starts on
   **Overview**, which is empty on a fresh install.
2. In the sidebar, click a service you want (for example **PHP**, **Caddy**, **MariaDB**). Each
   service page has two tabs: **Service** (state, ports, buttons) and **Versions** (what you can
   download). Pick a version and click **Install**. Downloads are verified against the vendor's
   checksums.
3. Turn the switch next to the service **on**. It starts right away and appears on Overview.
4. Open <http://localhost/> (or **Start All** on Overview to start everything that is on).

The local stack stores everything under one folder, the **Veranda root** (`~/veranda` by
default; shown in Settings). Deleting that folder removes all runtimes, sites and data.

## 2. Overview

**Startup groups** (the chips above the service list) start and stop a named set of services in
one click, in the order you chose: for example "Laravel stack" = Caddy, PHP, MariaDB, Valkey. The
group marked as default starts when Veranda launches, if "Start services on launch" is on.


- The status pill shows whether everything that is on is running.
- **Start All / Stop All** act on every service whose switch is on.
- Each row shows the version in use (a drop-down when several are installed), its ports and the
  start/stop/restart buttons. The switch turns a service off: it is stopped and hidden from
  Overview until you turn it on again in the sidebar.
- The tiles open the web root, Adminer, the mail viewer, a terminal in your document root, the
  `www` folder, and Logs.

If a port is already taken, Veranda names the program using it and offers the next free port.
On Linux, ports below 1024 need a one-time permission; Veranda asks for your password once and
grants it to the web server binary only (or switch to 8080/8443 in Settings).

## 3. Sites

**Share** (the link icon on a site, or next to the Sites title for the web root) opens a
Cloudflare quick tunnel: a public `https://….trycloudflare.com` address that reaches the site on
your computer, with no account. Install *Cloudflare Tunnel* under Tools first. Anyone with the
address can open the site while the tunnel runs, so stop it (the × next to the address) when you
are done; it also stops when Veranda quits.


Every folder inside `www/` becomes `https://folder-name.test` automatically. Laravel and Symfony
style `public/` folders are detected. For a project that lives elsewhere, use **Add external
folder** (link in place) or **Migrate** to bring projects and databases over from Laragon, XAMPP
or FlyEnv. Migrate looks for each stack in its usual place; for FlyEnv it reads the host list
(`host.json`) or the Apache/Nginx vhost files under its data folder, and tells you when FlyEnv is
installed but has no sites yet (then use "Other folder of projects" and point at the folder that
holds your projects). **New project** creates a folder from a template (plain, Composer package, Git clone)
and, optionally, a database. **Sync** rewrites the virtual hosts and your hosts file (asks for
elevation once).

## 4. Database

Host, user and password of the local MariaDB/MySQL server, a one-click **New password**, create
databases, open Adminer or phpMyAdmin, and **Backup** (dumps every database into `backups/`;
automatic backups are configured in Settings). PostgreSQL has the same card on its own page.

Veranda gives `root` a random password on first start. A project written for Laragon or XAMPP
usually expects `root` with no password: either paste the password from this card into the
project's `.env`, or click **No password**. The server only listens on `127.0.0.1`, so a
password-less root is still reachable only from your own computer.

## 5. Logs

The **Activity** box lists what Veranda itself did (started, stopped, installed, errors). The
drop-down opens the log file of any service; errors and warnings are coloured.

## 6. Settings

General (language, TLD, document root, editor, start on launch, start minimized, close button
behaviour, GPU rendering on Linux), web ports and HTTPS, PHP limits, database/cache/mail ports,
backups and updates. **Trust local CA** installs Veranda's certificate authority into your
browser's store so `https://*.test` shows no warning. **Config file** opens `veranda.toml` for
anything not in the form.

## 7. Remote (SSH client)

### Hosts
Click **New host**: label, address, port, username and how to log in (**Auth**):

| Auth | What it does |
| --- | --- |
| Password | Uses the password you type here (stored in the system credential store) or asks when connecting |
| Key | A key from the Keychain |
| Identity | A reusable username + password/key from the Keychain |
| SSH agent | Keys loaded in your agent (`ssh-agent`, Windows OpenSSH agent, Pageant) |
| Ask when connecting | No stored credential; the server's prompts are shown |

Optional: **Group** (hosts inherit the group's identity), **Jump host** (connect through another
host first), **Startup snippet** (sent after login), tags, a colour, environment variables sent
with the session, notes, keep-alive and agent forwarding.

Double-click a host to open a terminal. The row buttons open a terminal, SFTP, edit, duplicate
and delete. **Quick connect** (`user@host:22`) connects without saving. **Import** reads
`~/.ssh/config` (and PuTTY sessions on Windows) and adds the hosts, with their keys if you tick
"Import private keys".

The first time you connect to a server its key fingerprint is shown. Compare it with what the
server's administrator published, then **Trust**. A server whose key later changes is flagged in
red; only continue if you know why (reinstalled server). Accepted keys are listed under **Known
hosts**, where you can forget one.

### Terminal
Tabs hold SSH and local sessions (**+** or Ctrl+Shift+T opens a local shell with Veranda's PHP,
Node, Composer and friends on `PATH`). In the bar: state, snippet sender, search
(Ctrl+Shift+F), clear, reconnect, settings and close. Ctrl+Shift+C/V copy and paste; selecting
text copies it when "Copy on select" is on; right-click pastes. The gear sets font, size, theme
(follow Veranda, dark, light, Dracula, Solarized, Nord), cursor, scrollback, keep-alive interval,
the local shell program and whether closing a session asks first.

### SFTP
Pick a host and **Connect**. The left pane is this computer, the right pane the server.
Double-click folders to enter, use the path box, **↑**, home and refresh. Select files (Ctrl or
Shift for several) and press the arrow between the panes, or drag them across, to upload or
download; folders are transferred with their contents. The transfer list shows progress and lets
you cancel. Row buttons rename, delete, change permissions (server) and open a text file in a
small editor (server).

### Port forwarding
**New rule** with a type:

- **Local**: `127.0.0.1:3307 → 127.0.0.1:3306` through the server lets local tools reach the
  server's database on port 3307.
- **Remote**: the server listens on a port and sends connections to a service on your computer.
- **Dynamic**: a SOCKS5 proxy on your computer; point a browser at it to browse through the
  server.

The switch starts and stops a rule; the row shows how many connections are open. Keep the bind
address on `127.0.0.1` unless you really want other machines on your network to use the tunnel.

### Snippets
Commands you run often, grouped by folder. **Send** types one into the active terminal; **Run on
hosts** executes it on the hosts you tick and shows each host's output and exit code side by
side.

### Keychain
**New key** generates an Ed25519 (recommended), RSA 4096 or ECDSA key, optionally protected by a
passphrase. **Import key** reads a file from `~/.ssh` or pasted text. Per key: copy the public
key, **Install on host** (appends it to the host's `authorized_keys`, like `ssh-copy-id`), export
the public or private file, change the passphrase, delete. **Identities** combine a username with
a password or key so several hosts and groups can share one login.

### History
Every connection with its time, duration and outcome; click the terminal button to connect again.

## 8. Updates

Veranda checks the GitHub releases of this project 8 seconds after start and every 6 hours, and
shows a banner; **Update now** downloads the signed installer and restarts. With **Install
updates automatically** on, the download and restart happen by themselves: right when you switch
it on, at each check, and after **Check for updates**. Self-update works for .deb, .rpm and
AppImage on Linux, the NSIS/MSI installers on Windows and the .app on macOS; the banner says so
when an install cannot update itself (then use the release page). Service versions
(PHP, MariaDB and the rest) are listed from the vendors' own servers in each Versions tab.

## 9. Where your data is

| Data | Location |
| --- | --- |
| Settings | `<root>/etc/veranda.toml` |
| Installed runtimes | `<root>/bin/<kind>/<version>` |
| Websites | `<root>/www` |
| Databases | `<root>/data` |
| Logs, backups | `<root>/logs`, `<root>/backups` |
| Local certificate authority | `<root>/etc/ssl` (private) |
| Database root password | `<root>/etc/secrets` (private) |
| Remote hosts, groups, snippets, rules, history | `<root>/etc/remote/store.json` (private) |
| SSH private keys | `<root>/etc/remote/keys` (private, 0600) |
| Accepted server keys | `<root>/etc/remote/known_hosts` |
| SSH passwords and passphrases | system credential store (Keychain, Credential Manager, Secret Service); fallback `<root>/etc/secrets/remote-secrets.json` |

Back up the root folder to keep everything; never share the `etc/secrets` or `etc/remote/keys`
folders.

## 10. Troubleshooting

- **phpMyAdmin says "the mysqli extension is missing"**: the PHP version was installed with an
  older, smaller bundle. Open the PHP page, remove that version and install it again.
- **A service will not start**: open Logs, pick the service's log. The last lines say why
  (usually a port in use, which Overview also reports with an offer to move).
- **`*.test` does not resolve**: Sites → **Sync** updates the hosts file (needs elevation once).
- **Browser warns about the certificate**: Settings → **Trust local CA**, then restart the
  browser. Linux needs `certutil` (`sudo apt install libnss3-tools` or `dnf install nss-tools`)
  for Chrome, Brave and Firefox. Firefox on Windows/macOS reads system certificates only when
  `security.enterprise_roots.enabled` is `true` in `about:config`, or import `etc/ssl/ca.crt`
  by hand. Localhost tools (Adminer, phpMyAdmin) open over plain http and never need this.
- **"Unknown publisher" / Gatekeeper warnings when installing**: the installers are not code-signed
  yet; see [SIGNING.md](SIGNING.md).
- **"Server key changed" when connecting over SSH**: the server was reinstalled, or something is
  intercepting the connection. Confirm with the administrator before trusting the new key.
- **Password prompt although a password is saved**: the system credential store refused or timed
  out; Veranda falls back to a private file and says so on the Keychain page.
- **Linux: black or flickering window**: Settings → untick **GPU rendering**, restart Veranda.
