use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::client::OpenCodeClient;

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;

/// Manages the OpenCode server as a sidecar process.
#[derive(Debug)]
pub struct SidecarManager {
    /// The spawned child process (if running).
    process: Arc<Mutex<Option<Child>>>,
    /// Port the server is listening on.
    port: u16,
    /// Hostname.
    hostname: String,
    /// Path to the opencode binary.
    binary_path: PathBuf,
    /// Working directory for the server (project root).
    working_dir: PathBuf,
}

impl SidecarManager {
    pub fn new(binary_path: impl Into<PathBuf>, working_dir: impl Into<PathBuf>) -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            port: 4096,
            hostname: "127.0.0.1".to_string(),
            binary_path: binary_path.into(),
            working_dir: working_dir.into(),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    /// The base URL for the OpenCode server.
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.hostname, self.port)
    }

    /// Create a client connected to this sidecar.
    pub fn client(&self) -> OpenCodeClient {
        OpenCodeClient::new(self.base_url())
    }

    /// Start the OpenCode server process.
    pub async fn start(&self) -> Result<()> {
        let mut guard = self.process.lock().await;
        if guard.is_some() {
            bail!("OpenCode sidecar is already running");
        }

        let child = Command::new(&self.binary_path)
            .arg("serve")
            .arg("--port")
            .arg(self.port.to_string())
            .arg("--hostname")
            .arg(&self.hostname)
            .current_dir(&self.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("failed to spawn opencode process")?;

        *guard = Some(child);
        drop(guard);

        // Wait for the server to become healthy.
        self.wait_for_healthy(Duration::from_secs(10)).await?;

        log::info!(
            "OpenCode sidecar started on {}:{}",
            self.hostname,
            self.port
        );
        Ok(())
    }

    /// Stop the sidecar process.
    pub async fn stop(&self) -> Result<()> {
        let mut guard = self.process.lock().await;
        if let Some(mut child) = guard.take() {
            child
                .kill()
                .await
                .context("failed to kill opencode process")?;
            log::info!("OpenCode sidecar stopped");
        }
        Ok(())
    }

    /// Check if the process is still running.
    pub async fn is_running(&self) -> bool {
        let mut guard = self.process.lock().await;
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(None) => true, // still running
                _ => {
                    *guard = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Restart the sidecar.
    pub async fn restart(&self) -> Result<()> {
        self.stop().await.ok();
        self.start().await
    }

    /// Poll health endpoint until server responds or timeout.
    async fn wait_for_healthy(&self, timeout: Duration) -> Result<()> {
        let client = self.client();
        let start = tokio::time::Instant::now();
        let poll_interval = Duration::from_millis(100);

        loop {
            if start.elapsed() > timeout {
                bail!(
                    "OpenCode sidecar did not become healthy within {:?}",
                    timeout
                );
            }

            match client.health().await {
                Ok(h) if h.healthy => return Ok(()),
                _ => {}
            }

            // Check that process hasn't exited.
            if !self.is_running().await {
                bail!("OpenCode sidecar process exited unexpectedly");
            }

            tokio::time::sleep(poll_interval).await;
        }
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        // Best-effort cleanup — kill_on_drop on the Child handles this,
        // but we log for visibility.
        if let Ok(mut guard) = self.process.try_lock() {
            if guard.is_some() {
                log::info!("SidecarManager dropped, process will be killed");
                *guard = None;
            }
        }
    }
}

/// Find the opencode binary in PATH or common locations.
pub fn find_opencode_binary() -> Option<PathBuf> {
    // Check PATH first.
    if let Ok(output) = command::blocking::Command::new("which")
        .arg("opencode")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Common installation locations.
    let candidates = ["/usr/local/bin/opencode", "/opt/homebrew/bin/opencode"];

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
