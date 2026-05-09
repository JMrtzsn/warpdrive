use crate::sidecar::{find_opencode_binary, SidecarManager};
use crate::types::*;

/// Integration test: starts a real OpenCode sidecar, sends a prompt, and
/// inspects the actual response `Part` types returned by the API.
///
/// Run with: cargo test -p opencode_client -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn prompt_returns_expected_part_types() {
    // Install crypto provider for rustls (reqwest needs this).
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let binary = find_opencode_binary().expect("opencode binary not found on PATH");
    let work_dir = std::env::temp_dir().join("opencode_client_test");
    std::fs::create_dir_all(&work_dir).unwrap();

    let sidecar = SidecarManager::new(binary.to_str().unwrap(), work_dir.to_str().unwrap())
        .with_port(14199); // avoid collision with any running instance

    sidecar.start().await.expect("failed to start sidecar");

    // Wait for health.
    let client = sidecar.client();
    let mut healthy = false;
    for _ in 0..30 {
        if client.health().await.is_ok() {
            healthy = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(healthy, "sidecar never became healthy");

    // Create session.
    let session = client
        .create_session(&CreateSessionRequest {
            title: Some("integration-test".to_string()),
            parent_id: None,
        })
        .await
        .expect("failed to create session");

    // Send prompt — get raw response first to see all fields.
    let raw_resp = client
        .raw_prompt(&session.id, &PromptRequest {
            parts: vec![MessagePart::Text {
                text: "Say hello in one sentence.".to_string(),
            }],
            model: None,
            agent: None,
            no_reply: None,
            system: None,
            tools: None,
        })
        .await
        .expect("prompt failed");

    println!("=== RAW RESPONSE BODY ===");
    println!("{raw_resp}");
    println!();

    // Also try list_messages to see if content appears there.
    let messages = client.list_messages(&session.id).await.expect("list_messages failed");
    println!("=== LIST MESSAGES ({} messages) ===", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        println!("--- Message {i} (role={}) ---", msg.info.role);
        println!("{}", serde_json::to_string_pretty(&msg).unwrap());
    }

    // Also try get_message for the specific assistant message.
    let response: serde_json::Value = serde_json::from_str(&raw_resp).unwrap();
    if let Some(msg_id) = response.get("info").and_then(|i| i.get("id")).and_then(|v| v.as_str()) {
        let single = client.get_message(&session.id, msg_id).await;
        println!("\n=== GET MESSAGE {msg_id} ===");
        match single {
            Ok(m) => println!("{}", serde_json::to_string_pretty(&m).unwrap()),
            Err(e) => println!("Error: {e:#}"),
        }
    }

    // Cleanup.
    let _ = client.abort_session(&session.id).await;
    let _ = client.delete_session(&session.id).await;
    let _ = sidecar.stop().await;
}
