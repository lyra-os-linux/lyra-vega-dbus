use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallStatus {
    pub enabled: bool,
    pub active_zone: String,
}

impl From<(bool, String)> for FirewallStatus {
    fn from(row: (bool, String)) -> Self {
        Self {
            enabled: row.0,
            active_zone: row.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallService {
    pub name: String,
    pub label: String,
    pub enabled: bool,
}

impl From<(String, String, bool)> for FirewallService {
    fn from(row: (String, String, bool)) -> Self {
        Self {
            name: row.0,
            label: row.1,
            enabled: row.2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallClientError(String);

impl std::fmt::Display for FirewallClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de firewall indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for FirewallClientError {}

impl FirewallClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait FirewallClient: Send + Sync {
    async fn status(&self) -> Result<FirewallStatus, FirewallClientError>;
    async fn services(&self) -> Result<Vec<FirewallService>, FirewallClientError>;
    async fn set_service(&self, name: &str, enabled: bool) -> Result<(), FirewallClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Firewall",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Firewall {
    async fn status(&self) -> zbus::Result<(bool, String)>;
    async fn list_services(&self) -> zbus::Result<Vec<(String, String, bool)>>;
    async fn set_service_enabled(&self, name: &str, enabled: bool) -> zbus::Result<()>;
}

pub struct ZbusFirewallClient {
    connection: zbus::Connection,
}

impl ZbusFirewallClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<FirewallProxy<'_>, FirewallClientError> {
        FirewallProxy::new(&self.connection)
            .await
            .map_err(FirewallClientError::from_error)
    }
}

#[async_trait]
impl FirewallClient for ZbusFirewallClient {
    async fn status(&self) -> Result<FirewallStatus, FirewallClientError> {
        self.proxy()
            .await?
            .status()
            .await
            .map(Into::into)
            .map_err(FirewallClientError::from_error)
    }

    async fn services(&self) -> Result<Vec<FirewallService>, FirewallClientError> {
        self.proxy()
            .await?
            .list_services()
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(FirewallClientError::from_error)
    }

    async fn set_service(&self, name: &str, enabled: bool) -> Result<(), FirewallClientError> {
        self.proxy()
            .await?
            .set_service_enabled(name, enabled)
            .await
            .map_err(FirewallClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn firewall_xml_contract() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Firewall.xml");
        for method in ["Status", "ListServices", "SetServiceEnabled"] {
            assert!(xml.contains(&format!("<method name=\"{method}\">")));
        }
    }
}
