use async_trait::async_trait;

/// Persistent first-boot repository preparation state. State and error_kind are
/// stable identifiers; clients localize them instead of parsing LastError.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparationStatus {
    pub state: String,
    pub phase: String,
    pub error_kind: String,
    pub last_error: String,
    pub updated_at: String,
    pub next_retry_at: String,
    pub can_retry: bool,
}

/// A persistent review proposal; the opaque token binds source identity and key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparationKey {
    pub repo: String,
    pub fingerprint: String,
    pub user_id: String,
    pub token: String,
}

type PreparationRow = (String, String, String, String, String, String, bool);
impl From<PreparationRow> for PreparationStatus {
    fn from(row: PreparationRow) -> Self {
        Self {
            state: row.0,
            phase: row.1,
            error_kind: row.2,
            last_error: row.3,
            updated_at: row.4,
            next_retry_at: row.5,
            can_retry: row.6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparationClientError {
    Unavailable,
    Failed(String),
}
impl PreparationClientError {
    fn from_error(error: zbus::Error) -> Self {
        let unsupported = match &error {
            zbus::Error::MethodError(name, _, _) => matches!(
                name.as_str(),
                "org.freedesktop.DBus.Error.UnknownInterface"
                    | "org.freedesktop.DBus.Error.UnknownMethod"
            ),
            zbus::Error::FDO(error) => matches!(
                error.as_ref(),
                zbus::fdo::Error::UnknownInterface(_) | zbus::fdo::Error::UnknownMethod(_)
            ),
            _ => false,
        };
        if unsupported {
            Self::Unavailable
        } else {
            Self::Failed(error.to_string())
        }
    }
}
impl std::fmt::Display for PreparationClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(
                f,
                "{}",
                gettextrs::gettext("Estado da preparação indisponível nesta versão do serviço.")
            ),
            Self::Failed(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for PreparationClientError {}

#[async_trait]
pub trait PreparationClient: Send + Sync {
    /// Returns None for older daemons which do not implement this interface.
    async fn status(&self) -> Result<Option<PreparationStatus>, PreparationClientError>;
    async fn pending_keys(&self) -> Result<Vec<PreparationKey>, PreparationClientError> {
        Ok(Vec::new())
    }
    async fn approve_key(&self, _key: &PreparationKey) -> Result<u32, PreparationClientError> {
        Err(PreparationClientError::Unavailable)
    }
    /// Requests an authorized retry; completion is observed through status().
    async fn retry(&self) -> Result<(), PreparationClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Preparation",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Preparation {
    async fn get_status(&self) -> zbus::Result<PreparationRow>;
    async fn get_pending_keys(&self) -> zbus::Result<Vec<(String, String, String, String)>>;
    async fn approve_key(&self, repo: &str, fingerprint: &str, token: &str) -> zbus::Result<u32>;
    async fn retry(&self) -> zbus::Result<()>;
}

#[derive(Clone)]
pub struct ZbusPreparationClient {
    connection: zbus::Connection,
}
impl ZbusPreparationClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl PreparationClient for ZbusPreparationClient {
    async fn pending_keys(&self) -> Result<Vec<PreparationKey>, PreparationClientError> {
        let proxy = PreparationProxy::new(&self.connection)
            .await
            .map_err(PreparationClientError::from_error)?;
        match proxy
            .get_pending_keys()
            .await
            .map_err(PreparationClientError::from_error)
        {
            Ok(rows) => Ok(rows
                .into_iter()
                .map(|(repo, fingerprint, user_id, token)| PreparationKey {
                    repo,
                    fingerprint,
                    user_id,
                    token,
                })
                .collect()),
            Err(PreparationClientError::Unavailable) => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }
    async fn approve_key(&self, key: &PreparationKey) -> Result<u32, PreparationClientError> {
        PreparationProxy::new(&self.connection)
            .await
            .map_err(PreparationClientError::from_error)?
            .approve_key(&key.repo, &key.fingerprint, &key.token)
            .await
            .map_err(PreparationClientError::from_error)
    }

    async fn status(&self) -> Result<Option<PreparationStatus>, PreparationClientError> {
        let proxy = PreparationProxy::new(&self.connection)
            .await
            .map_err(PreparationClientError::from_error)?;
        match proxy
            .get_status()
            .await
            .map_err(PreparationClientError::from_error)
        {
            Ok(row) => Ok(Some(row.into())),
            Err(PreparationClientError::Unavailable) => Ok(None),
            Err(error) => Err(error),
        }
    }
    async fn retry(&self) -> Result<(), PreparationClientError> {
        let proxy = PreparationProxy::new(&self.connection)
            .await
            .map_err(PreparationClientError::from_error)?;
        proxy
            .retry()
            .await
            .map_err(PreparationClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_state_diagnostic_and_retry_fields() {
        let status: PreparationStatus = (
            "waiting-retry".into(),
            "refreshing".into(),
            "network".into(),
            "offline".into(),
            "now".into(),
            "later".into(),
            true,
        )
            .into();
        assert_eq!(status.state, "waiting-retry");
        assert_eq!(status.phase, "refreshing");
        assert_eq!(status.error_kind, "network");
        assert_eq!(status.last_error, "offline");
        assert_eq!(status.updated_at, "now");
        assert_eq!(status.next_retry_at, "later");
        assert!(status.can_retry);
    }
    #[test]
    fn old_daemon_fallback_does_not_hide_authorization_errors() {
        for error in [
            zbus::fdo::Error::UnknownInterface("old".into()),
            zbus::fdo::Error::UnknownMethod("old".into()),
        ] {
            assert_eq!(
                PreparationClientError::from_error(zbus::Error::FDO(Box::new(error))),
                PreparationClientError::Unavailable
            );
        }
        assert!(matches!(
            PreparationClientError::from_error(zbus::Error::FDO(Box::new(
                zbus::fdo::Error::AccessDenied("denied".into())
            ))),
            PreparationClientError::Failed(_)
        ));
    }
    #[test]
    fn canonical_xml_matches_proxy() {
        let xml = include_str!("../dbus/org.lyraos.Vega1.Preparation.xml");
        let doc = roxmltree::Document::parse(xml).unwrap();
        let interface = doc
            .descendants()
            .find(|n| n.has_tag_name("interface"))
            .unwrap();
        assert_eq!(
            interface.attribute("name"),
            Some("org.lyraos.Vega1.Preparation")
        );
        let methods: Vec<_> = interface
            .children()
            .filter(|n| n.has_tag_name("method"))
            .map(|n| {
                (
                    n.attribute("name").unwrap(),
                    n.children()
                        .filter(|a| a.has_tag_name("arg"))
                        .map(|a| {
                            (
                                a.attribute("type").unwrap(),
                                a.attribute("direction").unwrap(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            methods,
            vec![
                ("GetStatus", vec![("(ssssssb)", "out")]),
                ("GetPendingKeys", vec![("a(ssss)", "out")]),
                (
                    "ApproveKey",
                    vec![("s", "in"), ("s", "in"), ("s", "in"), ("u", "out")]
                ),
                ("Retry", vec![])
            ]
        );
    }
}
