//! Browser pool: lazy-initialized, shared Chromium instance for CDP automation.
//!
//! Uses `tokio::sync::OnceCell` for lazy initialization and `Arc<Mutex<>>` for
//! shared access. The browser instance persists between tool calls for session
//! continuity (cookies, localStorage, tabs).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chromiumoxide::browser::{Browser, BrowserConfig};
use tokio::sync::OnceCell;

use crate::browser_detect;

/// Shared browser pool that lazily launches a Chromium instance.
pub struct ChromeBrowserPool {
    browser: Browser,
    executable: PathBuf,
}

impl ChromeBrowserPool {
    /// Launch a new browser pool with the given executable path.
    pub async fn launch(executable: PathBuf) -> Result<Self> {
        let mut config = BrowserConfig::builder()
            .chrome_executable(&executable)
            .window_size(1440, 1024)
            .disable_default_args()
            .arg("--disable-gpu")
            .arg("--hide-scrollbars")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-extensions");

        if !std::env::var("CHROME_MCP_HEADED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            config = config.arg("--headless=new");
        }

        let (browser, mut handler) = Browser::launch(config.build().map_err(|e| anyhow!("{e}"))?)
            .await
            .with_context(|| format!("failed to launch browser at {}", executable.display()))?;

        // Spawn the CDP event handler — must keep running for the browser to work.
        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            browser,
            executable,
        })
    }

    /// Create a new tab and navigate to the given URL (or about:blank).
    pub async fn new_page(&self, url: &str) -> Result<chromiumoxide::Page> {
        let url = if url.is_empty() { "about:blank" } else { url };
        self.browser
            .new_page(url)
            .await
            .context("failed to create new browser tab")
    }

    /// Get all currently open pages (tabs).
    pub async fn pages(&self) -> Result<Vec<chromiumoxide::Page>> {
        Ok(self.browser.pages().await?)
    }

    /// Get the first page, or create one if none exist.
    pub async fn get_or_create_page(&self) -> Result<chromiumoxide::Page> {
        let pages = self.pages().await?;
        if let Some(page) = pages.into_iter().next() {
            return Ok(page);
        }
        self.new_page("about:blank").await
    }

    /// The detected browser executable path.
    pub fn executable(&self) -> &Path {
        &self.executable
    }
}

/// Global browser pool singleton.
static POOL: OnceCell<Arc<tokio::sync::Mutex<ChromeBrowserPool>>> = OnceCell::const_new();

/// Get or lazily initialize the global browser pool.
pub async fn global_pool() -> Result<Arc<tokio::sync::Mutex<ChromeBrowserPool>>> {
    POOL.get_or_try_init(|| async {
        let executable = browser_detect::detect_browser()
            .await
            .ok_or_else(|| anyhow!("no Chromium-based browser found for Chrome MCP"))?;
        let pool = ChromeBrowserPool::launch(executable).await?;
        tracing::info!("Chrome MCP browser pool initialized");
        Ok(Arc::new(tokio::sync::Mutex::new(pool)))
    })
    .await
    .cloned()
}
