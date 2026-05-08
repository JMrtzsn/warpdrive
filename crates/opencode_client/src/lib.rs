mod client;
mod events;
pub mod mapping;
mod sidecar;
mod types;

pub use client::OpenCodeClient;
pub use events::{subscribe_events, OpenCodeEvent, OpenCodeEventStream};
pub use mapping::{map_tool_call, MappedAction};
pub use sidecar::{find_opencode_binary, SidecarManager};
pub use types::*;
