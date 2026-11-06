//! Share a local site on the internet through a Cloudflare *quick tunnel* (`cloudflared tunnel
//! --url`): no account, a random `https://<words>.trycloudflare.com` address that lives as long as
//! the process. Anyone with the address can reach the site, so the UI asks before starting one.

use crate::paths::Layout;
use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct Tunnel {
    child: Child,
    pub url: String,
    pub target: String,
}

impl Tunnel {
    /// Start `cloudflared` for `local_url` (for example `http://127.0.0.1:8081`), sending
    /// `host_header` (the site's `*.test` name) so the web server picks the right virtual host.
    /// Waits up to 60 s for the public address.
    pub fn start(
        binary: &Path,
        layout: &Layout,
        target: &str,
        local_url: &str,
        host_header: Option<&str>,
    ) -> Result<Tunnel> {
        if !binary.is_file() {
            bail!("cloudflared is not installed (Tools > Cloudflare Tunnel)");
        }
        let log = std::fs::File::create(layout.logs().join("tunnel.log"))?;
        let mut cmd = Command::new(binary);
        cmd.args(["tunnel", "--no-autoupdate", "--url", local_url]);
        if let Some(h) = host_header {
            cmd.args(["--http-host-header", h]);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().context("starting cloudflared")?;
        let stderr = child.stderr.take().context("cloudflared has no stderr")?;
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut tail: Vec<String> = Vec::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(line) => {
                    if let Some(url) = line
                        .split_whitespace()
                        .find(|w| w.starts_with("https://") && w.contains(".trycloudflare.com"))
                    {
                        let url = url.trim_matches(|c: char| c == '|' || c == ',').to_string();
                        // keep draining stderr so cloudflared never blocks on a full pipe
                        std::thread::spawn(move || for _ in rx {});
                        return Ok(Tunnel {
                            child,
                            url,
                            target: target.to_string(),
                        });
                    }
                    tail.push(line);
                    if tail.len() > 12 {
                        tail.remove(0);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        bail!(
                            "cloudflared exited ({status}): {}",
                            tail.last().cloned().unwrap_or_default()
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        bail!(
            "cloudflared gave no public address within 60 s: {}",
            tail.last().cloned().unwrap_or_default()
        )
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
