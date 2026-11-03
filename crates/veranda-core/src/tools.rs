//! Opening URLs, folders, editors and terminals; running commands with elevation.

use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn open_url(cfg: &Config, url: &str) -> Result<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("refusing to open non-http url {url}");
    }
    if !cfg.general.browser.trim().is_empty() {
        return spawn_quiet(Command::new(cfg.general.browser.trim()).arg(url));
    }
    open_with_system(url)
}

pub fn open_path(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("{} does not exist", path.display());
    }
    open_with_system(&path.to_string_lossy())
}

fn open_with_system(target: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        // `start` is a cmd builtin; the empty string is the window title argument.
        spawn_quiet(Command::new("cmd").args(["/C", "start", "", target]))
    }
    #[cfg(target_os = "macos")]
    {
        spawn_quiet(Command::new("open").arg(target))
    }
    #[cfg(target_os = "linux")]
    {
        spawn_quiet(Command::new("xdg-open").arg(target))
    }
}

pub fn open_editor(cfg: &Config, path: &Path) -> Result<()> {
    let editor = cfg.general.editor.trim();
    if editor.is_empty() {
        return open_path(path);
    }
    spawn_quiet(Command::new(editor).arg(path))
}

/// Open a terminal in `dir` with `path_entries` prepended to PATH.
pub fn open_terminal(dir: &Path, path_entries: &[PathBuf]) -> Result<()> {
    let path = crate::process::env_path(path_entries);
    #[cfg(target_os = "windows")]
    {
        let mut cmd = if which::which("wt.exe").is_ok() {
            let mut c = Command::new("wt.exe");
            c.args(["-d", &dir.to_string_lossy()]);
            c
        } else {
            let mut c = Command::new("cmd");
            c.args([
                "/C",
                "start",
                "",
                "cmd",
                "/K",
                &format!("cd /d \"{}\"", dir.display()),
            ]);
            c
        };
        cmd.env("PATH", path);
        spawn_quiet(&mut cmd)
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\"\n activate\n do script \"cd {} && export PATH={}\"\nend tell",
            shell_quote(&dir.to_string_lossy()),
            shell_quote(&path.to_string_lossy())
        );
        spawn_quiet(Command::new("osascript").args(["-e", &script]))
    }
    #[cfg(target_os = "linux")]
    {
        let candidates: [(&str, &[&str]); 7] = [
            ("x-terminal-emulator", &[]),
            ("gnome-terminal", &["--"]),
            ("konsole", &["-e"]),
            ("xfce4-terminal", &["-e"]),
            ("kitty", &[]),
            ("alacritty", &["-e"]),
            ("xterm", &["-e"]),
        ];
        for (term, args) in candidates {
            if which::which(term).is_ok() {
                let mut cmd = Command::new(term);
                cmd.current_dir(dir).env("PATH", &path);
                if term == "gnome-terminal" {
                    cmd.arg("--working-directory").arg(dir);
                }
                if !args.is_empty() {
                    cmd.args(args);
                    cmd.arg(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));
                }
                return spawn_quiet(&mut cmd);
            }
        }
        bail!("no terminal emulator found")
    }
}

#[allow(dead_code)]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn spawn_quiet(cmd: &mut Command) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .with_context(|| format!("launching {:?}", cmd.get_program()))?;
    Ok(())
}

/// Run `program args` with administrator rights using the platform's graphical prompt
/// (pkexec / osascript / UAC). Blocks until it finishes.
pub fn run_elevated(program: &Path, args: &[String]) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        if which::which("pkexec").is_ok() {
            let st = Command::new("pkexec").arg(program).args(args).status()?;
            if !st.success() {
                bail!("pkexec exited with {st}");
            }
            return Ok(());
        }
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            let st = Command::new("sudo").arg(program).args(args).status()?;
            if !st.success() {
                bail!("sudo exited with {st}");
            }
            return Ok(());
        }
        bail!("no elevation helper (pkexec) available")
    }
    #[cfg(target_os = "macos")]
    {
        let mut line = shell_quote(&program.to_string_lossy());
        for a in args {
            line.push(' ');
            line.push_str(&shell_quote(a));
        }
        let script = format!(
            "do shell script {} with administrator privileges",
            applescript_string(&line)
        );
        let out = Command::new("osascript").args(["-e", &script]).output()?;
        if !out.status.success() {
            bail!("osascript failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let arg_list: Vec<String> = args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect();
        let ps = format!(
            "$p = Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
            program.to_string_lossy().replace('\'', "''"),
            arg_list.join(",")
        );
        let st = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
            .status()?;
        if !st.success() {
            bail!("elevated command failed with {st}");
        }
        Ok(())
    }
}

#[allow(dead_code)]
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Grant a binary permission to bind ports < 1024 on Linux without root.
#[cfg(target_os = "linux")]
pub fn grant_bind_capability(binary: &Path) -> Result<()> {
    let setcap = which::which("setcap").context("setcap not found (install libcap2-bin)")?;
    run_elevated(
        &setcap,
        &[
            "cap_net_bind_service=+ep".into(),
            binary.to_string_lossy().into_owned(),
        ],
    )
}

#[cfg(not(target_os = "linux"))]
pub fn grant_bind_capability(_binary: &Path) -> Result<()> {
    Ok(())
}

/// Our own executable, for re-invoking as an elevated helper.
pub fn current_exe() -> Result<PathBuf> {
    Ok(std::env::current_exe()?)
}
