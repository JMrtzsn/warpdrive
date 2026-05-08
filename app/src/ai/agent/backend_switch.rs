//! Feature-flag controlled switch between Warp server and OpenCode backends.

use std::sync::Arc;

use once_cell::sync::Lazy;
use opencode_client::{find_opencode_binary, SidecarManager};
use tokio::sync::Mutex;

use super::api::{ConvertToAPITypeError, RequestParams, ResponseStream as ApiResponseStream};
use super::opencode_adapter::{generate_opencode_output, OpenCodeAdapter};
use crate::ai::agent::api::generate_multi_agent_output;
use crate::server::server_api::ServerApi;

/// Global OpenCode adapter instance (lazily initialized).
static OPENCODE_ADAPTER: Lazy<Option<Arc<Mutex<OpenCodeAdapter>>>> = Lazy::new(|| {
    let binary = find_opencode_binary()?;
    let working_dir = std::env::current_dir().ok()?;
    // Use port 14096 to avoid conflicts with user's opencode instance.
    let sidecar = SidecarManager::new(binary, working_dir).with_port(14096);
    Some(Arc::new(Mutex::new(OpenCodeAdapter::new(sidecar))))
});

/// Check whether the OpenCode backend should be used.
pub fn should_use_opencode() -> bool {
    if let Ok(val) = std::env::var("WARP_USE_OPENCODE") {
        return val == "1" || val == "true";
    }
    cfg!(feature = "opencode_backend")
}

/// Unified entry point for generating agent output.
///
/// Routes to OpenCode or Warp server based on `should_use_opencode()`.
pub async fn generate_agent_output(
    server_api: Arc<ServerApi>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ApiResponseStream, ConvertToAPITypeError> {
    if should_use_opencode() {
        if let Some(adapter) = OPENCODE_ADAPTER.as_ref() {
            return generate_opencode_output(adapter.clone(), params, cancellation_rx).await;
        }
        log::warn!("WARP_USE_OPENCODE is set but opencode binary not found in PATH; falling back to Warp server");
    }

    generate_multi_agent_output(server_api, params, cancellation_rx).await
}
