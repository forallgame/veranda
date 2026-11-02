//! Child process control: detached spawn with log files, pid files, graceful stop, port checks.

use crate::paths::Layout;
use anyhow::{Context, Result, bail};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Spawn `cmd` detached from our console, redirect output to `<logs>/<name>.log`,
/// write `<run>/<name>.pid`. Returns the pid.
pub fn spawn_detached(layout: &Layout, name: &str, mut cmd: Command) -> Result<u32> {
    let log_path = layout.logs().join(format!("{name}.log"));
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_err = log.try_clone()?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("starting {name}: {:?}", cmd.get_program()))?;
    let pid = child.id();
    write_pid(layout, name, pid)?;
    // The child keeps running; we intentionally do not wait on it.
    std::mem::forget(child);
    Ok(pid)
}

/// Run a command to completion, capturing output; fails on non-zero exit.
pub fn run_capture(mut cmd: Command) -> Result<String> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {:?}", cmd.get_program()))?;
    if !out.status.success() {
        bail!(
            "{:?} failed ({}):\n{}{}",
            cmd.get_program(),
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn pid_file(layout: &Layout, name: &str) -> PathBuf {
    layout.run().join(format!("{name}.pid"))
}

pub fn write_pid(layout: &Layout, name: &str, pid: u32) -> Result<()> {
    crate::fsutil::write_atomic(&pid_file(layout, name), pid.to_string().as_bytes())
}

pub fn read_pid(layout: &Layout, name: &str) -> Option<u32> {
    std::fs::read_to_string(pid_file(layout, name))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn clear_pid(layout: &Layout, name: &str) {
    let _ = std::fs::remove_file(pid_file(layout, name));
}

/// Is the recorded pid alive *and* does its executable live under our root?
/// (Guards against pid reuse after a reboot.)
pub fn running_pid(layout: &Layout, name: &str) -> Option<u32> {
    let pid = read_pid(layout, name)?;
    let sys = refreshed(pid)?;
    let proc_ = sys.process(Pid::from_u32(pid))?;
    let exe_ok = proc_
        .exe()
        .map(|e| e.starts_with(layout.root()) || !cfg!(target_os = "windows"))
        .unwrap_or(true);
    if exe_ok { Some(pid) } else { None }
}

/// A `System` with only `pid` refreshed.
fn refreshed(pid: u32) -> Option<System> {
    let mut sys = System::new().ok()?;
    sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    Some(sys)
}

pub fn is_pid_alive(pid: u32) -> bool {
    refreshed(pid)
        .map(|s| s.process(Pid::from_u32(pid)).is_some())
        .unwrap_or(false)
}

/// Terminate a pid: SIGTERM (unix) / TerminateProcess (windows), wait up to `grace`, then SIGKILL.
pub fn stop_pid(pid: u32, grace: Duration) -> Result<()> {
    let Some(sys) = refreshed(pid) else {
        return Ok(());
    };
    let Some(p) = sys.process(Pid::from_u32(pid)) else {
        return Ok(());
    };
    #[cfg(unix)]
    {
        let _ = p.kill_with(sysinfo::Signal::Term);
    }
    #[cfg(windows)]
    {
        let _ = p.kill();
    }
    let start = Instant::now();
    while start.elapsed() < grace {
        if !is_pid_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    if let Some(sys) = refreshed(pid)
        && let Some(p) = sys.process(Pid::from_u32(pid))
    {
        p.kill();
    }
    Ok(())
}

/// Stop every process whose pid file we own for `name`.
pub fn stop_named(layout: &Layout, name: &str, grace: Duration) -> Result<()> {
    if let Some(pid) = read_pid(layout, name) {
        stop_pid(pid, grace)?;
    }
    clear_pid(layout, name);
    Ok(())
}

/// True if something listens on 127.0.0.1:port.
pub fn port_in_use(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_err()
}

/// True once something accepts connections on 127.0.0.1:port.
pub fn port_accepting(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_millis(300),
    )
    .is_ok()
}

pub fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if port_accepting(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// Best-effort description of the process occupying a port (for error messages only).
pub fn port_owner(port: u16) -> Option<String> {
    #[cfg(target_os = "linux")]
    let out = Command::new("ss")
        .args(["-ltnp", &format!("sport = :{port}")])
        .output()
        .ok()?;
    #[cfg(target_os = "macos")]
    let out = Command::new("lsof")
        .args(["-nP", "-iTCP", &format!(":{port}"), "-sTCP:LISTEN"])
        .output()
        .ok()?;
    #[cfg(target_os = "windows")]
    let out = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().skip(1).find(|l| {
        l.contains(&format!(":{port} "))
            || l.contains(&format!(":{port}\t"))
            || l.contains(&format!(":{port}"))
    })?;
    Some(line.trim().to_string())
}

/// Build a `Command` for an executable with `PATH` prefixed by our runtime dirs.
pub fn command(exe: &Path, path_prefix: &[PathBuf]) -> Command {
    let mut cmd = Command::new(exe);
    if !path_prefix.is_empty() {
        cmd.env("PATH", env_path(path_prefix));
    }
    cmd
}

/// Compose a PATH value with `prefix` entries in front of the current PATH.
pub fn env_path(prefix: &[PathBuf]) -> std::ffi::OsString {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut all: Vec<PathBuf> = prefix.to_vec();
    all.extend(std::env::split_paths(&current));
    std::env::join_paths(all.iter().filter(|p| !p.as_os_str().is_empty())).unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_port_detect() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(port_in_use(port));
        drop(l);
        assert!(!port_in_use(port));
    }
}
