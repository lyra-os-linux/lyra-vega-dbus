use async_trait::async_trait;

use super::{BackendStatus, SystemClient, SystemClientError};

#[derive(Debug, Clone)]
pub struct MockSystemClient {
    status: Result<BackendStatus, SystemClientError>,
    disk_usage: Result<(String, String, u32), SystemClientError>,
}

impl MockSystemClient {
    pub fn connected(status: BackendStatus, disk_usage: (String, String, u32)) -> Self {
        Self {
            status: Ok(status),
            disk_usage: Ok(disk_usage),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        let error = SystemClientError::Unavailable(message.into());
        Self {
            status: Err(error.clone()),
            disk_usage: Err(error),
        }
    }
}

#[async_trait]
impl SystemClient for MockSystemClient {
    async fn status(&self) -> Result<BackendStatus, SystemClientError> {
        self.status.clone()
    }

    async fn disk_usage(&self) -> Result<(String, String, u32), SystemClientError> {
        self.disk_usage.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_mock_covers_the_offline_state() {
        let mock = MockSystemClient::unavailable("system bus ausente");
        let result = futures_lite::future::block_on(mock.status());
        assert!(matches!(result, Err(SystemClientError::Unavailable(_))));
    }

    #[test]
    fn connected_mock_returns_typed_values() {
        let expected = BackendStatus {
            version: "2.0.3".into(),
            distro: "Test Linux".into(),
            logo_path: "/test/logo.svg".into(),
        };
        let mock = MockSystemClient::connected(expected.clone(), ("10G".into(), "20G".into(), 50));

        assert_eq!(
            futures_lite::future::block_on(mock.status()).unwrap(),
            expected
        );
        assert_eq!(
            futures_lite::future::block_on(mock.disk_usage()).unwrap(),
            ("10G".into(), "20G".into(), 50)
        );
    }
}
