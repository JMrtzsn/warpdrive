//! OpenCode is the sole AI backend for Warpdrive.
//!
//! Each agent conversation gets its own sidecar process on a dynamic port,
//! so multiple terminals/agents can run concurrently without blocking each other.
//! Sidecars are started lazily on first AI prompt.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use once_cell::sync::Lazy;
use opencode_client::{find_opencode_binary, SidecarManager};
use tokio::sync::Mutex;

use super::api::{ConvertToAPITypeError, RequestParams, ResponseStream as ApiResponseStream};
use super::opencode_adapter::{generate_opencode_output, OpenCodeAdapter};

/// Next port to allocate for a new sidecar instance.
static NEXT_PORT: AtomicU16 = AtomicU16::new(14100);

/// Pool of OpenCode adapters keyed by a stable session identifier.
static ADAPTER_POOL: Lazy<Arc<Mutex<HashMap<String, Arc<Mutex<OpenCodeAdapter>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Get or create an adapter for the given session key.
async fn get_adapter(
    key: String,
    working_dir: PathBuf,
) -> Result<Arc<Mutex<OpenCodeAdapter>>, ConvertToAPITypeError> {
    let mut pool = ADAPTER_POOL.lock().await;

    if let Some(adapter) = pool.get(&key) {
        return Ok(adapter.clone());
    }

    let binary = find_opencode_binary().ok_or_else(|| {
        log::error!("OpenCode binary not found in PATH");
        ConvertToAPITypeError::Other(
            anyhow::anyhow!("OpenCode binary not found. Install opencode and ensure it's on PATH.")
                .into(),
        )
    })?;

    let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
    let sidecar = SidecarManager::new(binary, working_dir).with_port(port);
    let adapter = Arc::new(Mutex::new(OpenCodeAdapter::with_sidecar(sidecar)));
    pool.insert(key, adapter.clone());
    Ok(adapter)
}

/// Generate agent output via OpenCode.
pub async fn generate_agent_output(
    _server_api: Arc<crate::server::server_api::ServerApi>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ApiResponseStream, ConvertToAPITypeError> {
    // Key by conversation token — each conversation gets its own sidecar.
    // For the first message (no token yet), generate a temporary key that will
    // be replaced once the conversation is established.
    let key = params
        .conversation_token
        .as_ref()
        .map(|t| t.as_str().to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let working_dir = params
        .session_context
        .current_working_directory()
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

    let adapter = get_adapter(key, working_dir).await?;

    generate_opencode_output(adapter, params, cancellation_rx).await
}
