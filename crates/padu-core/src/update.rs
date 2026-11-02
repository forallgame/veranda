//! Self-update check against GitHub releases of the configured repository.

use crate::download::{client, fetch_text};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub published_at: String,
    pub notes: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Returns the newest release if it is newer than the running version.
pub async fn check(repo: &str) -> Result<Option<Release>> {
    if !crate::config::is_repo_slug(repo) {
        anyhow::bail!("update.repo is not configured (owner/repo)");
    }
    let c = client()?;
    let text = fetch_text(
        &c,
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
    )
    .await?;
    let rel: GhRelease = serde_json::from_str(&text).context("parsing GitHub release")?;
    if rel.prerelease || rel.draft {
        return Ok(None);
    }
    let latest = semver::Version::parse(rel.tag_name.trim_start_matches('v'))
        .context("release tag is not semver")?;
    let current = semver::Version::parse(crate::VERSION)?;
    if latest > current {
        Ok(Some(Release {
            version: latest.to_string(),
            url: rel.html_url,
            published_at: rel.published_at,
            notes: rel.body,
        }))
    } else {
        Ok(None)
    }
}
