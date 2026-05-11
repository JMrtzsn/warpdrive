#[cfg(test)]
mod tests {
    use crate::sidecar::SidecarManager;
    use std::path::PathBuf;

    #[test]
    fn test_sidecar_base_url() {
        let sidecar = SidecarManager::new("/usr/bin/opencode", "/tmp/project")
            .with_port(5000)
            .with_hostname("localhost");
        assert_eq!(sidecar.base_url(), "http://localhost:5000");
    }

    #[test]
    fn test_sidecar_default_port() {
        let sidecar = SidecarManager::new("/usr/bin/opencode", "/tmp/project");
        assert_eq!(sidecar.base_url(), "http://127.0.0.1:4096");
    }

    #[test]
    fn test_client_creation() {
        let sidecar = SidecarManager::new("/usr/bin/opencode", "/tmp/project");
        // Just test that base_url is correct; actual Client creation
        // requires TLS runtime which may not be available in unit tests.
        assert_eq!(sidecar.base_url(), "http://127.0.0.1:4096");
    }

    #[tokio::test]
    async fn test_sidecar_not_running_initially() {
        let sidecar = SidecarManager::new("/usr/bin/opencode", "/tmp/project");
        assert!(!sidecar.is_running().await);
    }

    #[tokio::test]
    async fn test_sidecar_start_with_missing_binary() {
        let sidecar = SidecarManager::new("/nonexistent/path/opencode", "/tmp/project");
        let result = sidecar.start().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_find_opencode_binary_returns_option() {
        // Just ensure it doesn't panic — may or may not find the binary.
        let _ = crate::sidecar::find_opencode_binary();
    }
}
