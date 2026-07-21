use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterface {
    pub name: String,
    pub kind: String,
    pub state: String,
    pub ipv4: String,
    pub ipv6: String,
    pub gateway: String,
    pub dns: String,
    pub mac: String,
    pub speed: String,
    pub ssid: String,
    pub signal: u32,
    pub device: String,
    pub autoconf: bool,
}
type InterfaceRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    u32,
    String,
    bool,
);
impl From<InterfaceRow> for NetworkInterface {
    fn from(r: InterfaceRow) -> Self {
        Self {
            name: r.0,
            kind: r.1,
            state: r.2,
            ipv4: r.3,
            ipv6: r.4,
            gateway: r.5,
            dns: r.6,
            mac: r.7,
            speed: r.8,
            ssid: r.9,
            signal: r.10,
            device: r.11,
            autoconf: r.12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    pub security: String,
    pub signal: u32,
    pub active: bool,
    pub device: String,
}
type WifiRow = (String, String, u32, bool, String);
impl From<WifiRow> for WifiNetwork {
    fn from(r: WifiRow) -> Self {
        Self {
            ssid: r.0,
            security: r.1,
            signal: r.2,
            active: r.3,
            device: r.4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub http: String,
    pub https: String,
    pub socks: String,
    pub no_proxy: String,
}
impl From<(String, String, String, String)> for ProxyConfig {
    fn from(r: (String, String, String, String)) -> Self {
        Self {
            http: r.0,
            https: r.1,
            socks: r.2,
            no_proxy: r.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkClientError(String);

impl std::fmt::Display for NetworkClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de rede indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for NetworkClientError {}

impl NetworkClientError {
    fn from_error(e: impl std::fmt::Display) -> Self {
        Self(e.to_string())
    }
}

#[async_trait]
pub trait NetworkClient: Send + Sync {
    async fn interfaces(&self) -> Result<Vec<NetworkInterface>, NetworkClientError>;
    async fn wifi(&self) -> Result<Vec<WifiNetwork>, NetworkClientError>;
    async fn proxy(&self) -> Result<ProxyConfig, NetworkClientError>;
    async fn set_proxy(&self, config: &ProxyConfig) -> Result<(), NetworkClientError>;
    async fn connect_wifi(&self, ssid: &str, password: &str) -> Result<(), NetworkClientError>;
    async fn disconnect(&self, device: &str) -> Result<(), NetworkClientError>;
    async fn set_static_ipv4(
        &self,
        connection: &str,
        address: &str,
        gateway: &str,
        dns: &str,
    ) -> Result<(), NetworkClientError>;
    async fn import_vpn(&self, path: &str) -> Result<(), NetworkClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Network",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Network {
    async fn list_interfaces(&self) -> zbus::Result<Vec<InterfaceRow>>;
    async fn list_wifi(&self) -> zbus::Result<Vec<WifiRow>>;
    async fn get_proxy(&self) -> zbus::Result<(String, String, String, String)>;
    async fn set_proxy(
        &self,
        http: &str,
        https: &str,
        socks: &str,
        no_proxy: &str,
    ) -> zbus::Result<()>;
    async fn connect_wifi(&self, ssid: &str, password: &str) -> zbus::Result<()>;
    async fn disconnect(&self, device: &str) -> zbus::Result<()>;
    async fn set_static_ipv4(
        &self,
        connection: &str,
        address: &str,
        gateway: &str,
        dns: &str,
    ) -> zbus::Result<()>;
    async fn import_vpn(&self, path: &str) -> zbus::Result<()>;
}
pub struct ZbusNetworkClient {
    connection: zbus::Connection,
}
impl ZbusNetworkClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }
    async fn proxy_client(&self) -> Result<NetworkProxy<'_>, NetworkClientError> {
        NetworkProxy::new(&self.connection)
            .await
            .map_err(NetworkClientError::from_error)
    }
}
#[async_trait]
impl NetworkClient for ZbusNetworkClient {
    async fn interfaces(&self) -> Result<Vec<NetworkInterface>, NetworkClientError> {
        self.proxy_client()
            .await?
            .list_interfaces()
            .await
            .map(|r| r.into_iter().map(Into::into).collect())
            .map_err(NetworkClientError::from_error)
    }
    async fn wifi(&self) -> Result<Vec<WifiNetwork>, NetworkClientError> {
        self.proxy_client()
            .await?
            .list_wifi()
            .await
            .map(|r| r.into_iter().map(Into::into).collect())
            .map_err(NetworkClientError::from_error)
    }
    async fn proxy(&self) -> Result<ProxyConfig, NetworkClientError> {
        self.proxy_client()
            .await?
            .get_proxy()
            .await
            .map(Into::into)
            .map_err(NetworkClientError::from_error)
    }

    async fn connect_wifi(&self, ssid: &str, password: &str) -> Result<(), NetworkClientError> {
        self.proxy_client()
            .await?
            .connect_wifi(ssid, password)
            .await
            .map_err(NetworkClientError::from_error)
    }

    async fn set_proxy(&self, config: &ProxyConfig) -> Result<(), NetworkClientError> {
        self.proxy_client()
            .await?
            .set_proxy(&config.http, &config.https, &config.socks, &config.no_proxy)
            .await
            .map_err(NetworkClientError::from_error)
    }

    async fn disconnect(&self, device: &str) -> Result<(), NetworkClientError> {
        self.proxy_client()
            .await?
            .disconnect(device)
            .await
            .map_err(NetworkClientError::from_error)
    }

    async fn set_static_ipv4(
        &self,
        connection: &str,
        address: &str,
        gateway: &str,
        dns: &str,
    ) -> Result<(), NetworkClientError> {
        self.proxy_client()
            .await?
            .set_static_ipv4(connection, address, gateway, dns)
            .await
            .map_err(NetworkClientError::from_error)
    }

    async fn import_vpn(&self, path: &str) -> Result<(), NetworkClientError> {
        self.proxy_client()
            .await?
            .import_vpn(path)
            .await
            .map_err(NetworkClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn network_xml_contract() {
        let x = include_str!("../../dbus/org.lyraos.Vega1.Network.xml");
        for m in [
            "ListInterfaces",
            "ListWifi",
            "ConnectWifi",
            "Disconnect",
            "SetStaticIPv4",
            "ImportVPN",
            "GetProxy",
            "SetProxy",
        ] {
            assert!(x.contains(&format!("<method name=\"{m}\">")));
        }
    }
}
