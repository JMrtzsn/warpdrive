//! OpenCode is the sole AI backend for Warpdrive.
//!
//! The sidecar is lazily started on first use.

use std::sync::Arc;

use once_cell::sync::Lazy;
use opencode_client::{find_opencode_binary, SidecarManager};
use tokio::sync::Mutex;

use super::api::{ConvertToAPITypeError, RequestParams, ResponseStream as ApiResponseStream};
use super::opencode_adapter::{generate_opencode_output, OpenCodeAdapter};

/// Global OpenCode adapter instance (lazily initialized).
static OPENCODE_ADAPTER: Lazy<Option<Arc<Mutex<OpenCodeAdapter>>>> = Lazy::new(|| {
    let binary = find_opencode_binary()?;
    let working_dir = std::env::current_dir().ok()?;
    let sidecar = SidecarManager::new(binary, working_dir).with_port(14096);
    Some(Arc::new(Mutex::new(OpenCodeAdapter::with_sidecar(sidecar))))
});

/// Generate agent output via OpenCode.
pub async fn generate_agent_output(
    _server_api: Arc<crate::server::server_api::ServerApi>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ApiResponseStream, ConvertToAPITypeError> {
    let adapter = OPENCODE_ADAPTER.as_ref().ok_or_else(|| {
        log::error!("OpenCode binary not found in PATH");
        ConvertToAPITypeError::Other(
            anyhow::anyhow!("OpenCode binary not found. Install opencode and ensure it's on PATH.")
                .into(),
        )
    })?;

    generate_opencode_output(adapter.clone(), params, cancellation_rx).await
}
