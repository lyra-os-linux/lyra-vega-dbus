use async_trait::async_trait;

pub const BUS_NAME: &str = "org.lyraos.Vega1";
pub const OBJECT_PATH: &str = "/org/lyraos/Vega1";
pub const SYSTEM_INTERFACE: &str = "org.lyraos.Vega1.System";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendStatus {
    pub version: String,
    pub distro: String,
    pub logo_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemClientError {
    Unavailable(String),
    InvalidResponse(String),
}

impl std::fmt::Display for SystemClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(detail) => write!(
                f,
                "{}",
                gettextrs::gettext("vegad indisponível: {detail}").replace("{detail}", detail)
            ),
            Self::InvalidResponse(detail) => write!(
                f,
                "{}",
                gettextrs::gettext("resposta inválida do vegad: {detail}")
                    .replace("{detail}", detail)
            ),
        }
    }
}

impl std::error::Error for SystemClientError {}

#[async_trait]
pub trait SystemClient: Send + Sync {
    async fn status(&self) -> Result<BackendStatus, SystemClientError>;
    async fn disk_usage(&self) -> Result<(String, String, u32), SystemClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.System",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait System {
    async fn version(&self) -> zbus::Result<String>;
    async fn ping(&self) -> zbus::Result<bool>;
    async fn distro(&self) -> zbus::Result<String>;
    async fn logo(&self) -> zbus::Result<String>;
    async fn disk_usage(&self) -> zbus::Result<(String, String, u32)>;
}

pub struct ZbusSystemClient {
    connection: zbus::Connection,
}

impl ZbusSystemClient {
    pub async fn connect() -> Result<Self, SystemClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(SystemClientError::unavailable)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<SystemProxy<'_>, SystemClientError> {
        SystemProxy::new(&self.connection)
            .await
            .map_err(SystemClientError::unavailable)
    }
}

impl SystemClientError {
    fn unavailable(error: impl std::fmt::Display) -> Self {
        Self::Unavailable(error.to_string())
    }
}

#[async_trait]
impl SystemClient for ZbusSystemClient {
    async fn status(&self) -> Result<BackendStatus, SystemClientError> {
        let proxy = self.proxy().await?;
        if !proxy.ping().await.map_err(SystemClientError::unavailable)? {
            return Err(SystemClientError::InvalidResponse(
                "Ping retornou false".into(),
            ));
        }

        let (version, distro, logo_path) =
            futures_util::try_join!(proxy.version(), proxy.distro(), proxy.logo(),)
                .map_err(SystemClientError::unavailable)?;

        Ok(BackendStatus {
            version,
            distro,
            logo_path,
        })
    }

    async fn disk_usage(&self) -> Result<(String, String, u32), SystemClientError> {
        self.proxy()
            .await?
            .disk_usage()
            .await
            .map_err(SystemClientError::unavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const SYSTEM_XML: &str = include_str!("../../dbus/org.lyraos.Vega1.System.xml");

    #[test]
    fn system_xml_matches_the_typed_proxy_contract() {
        // roxmltree intentionally rejects DTDs. D-Bus XML files carry only
        // the standard public DOCTYPE, so parse the introspection node itself.
        let node_start = SYSTEM_XML.find("<node").expect("nó de introspecção");
        let document =
            roxmltree::Document::parse(&SYSTEM_XML[node_start..]).expect("System XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(SYSTEM_INTERFACE)
            })
            .expect("interface System presente");

        let methods: BTreeMap<_, _> = interface
            .children()
            .filter(|node| node.has_tag_name("method"))
            .map(|method| {
                let name = method.attribute("name").expect("método com nome");
                let args = method
                    .children()
                    .filter(|node| node.has_tag_name("arg"))
                    .map(|arg| {
                        (
                            arg.attribute("direction").unwrap_or("in"),
                            arg.attribute("type").expect("argumento com tipo"),
                        )
                    })
                    .collect::<Vec<_>>();
                (name, args)
            })
            .collect();

        let expected = BTreeMap::from([
            ("DiskUsage", vec![("out", "s"), ("out", "s"), ("out", "u")]),
            ("Distro", vec![("out", "s")]),
            ("Logo", vec![("out", "s")]),
            ("Ping", vec![("out", "b")]),
            ("Version", vec![("out", "s")]),
        ]);
        assert_eq!(methods, expected);
    }

    #[test]
    fn constants_preserve_the_public_bus_contract() {
        assert_eq!(BUS_NAME, "org.lyraos.Vega1");
        assert_eq!(OBJECT_PATH, "/org/lyraos/Vega1");
        assert_eq!(SYSTEM_INTERFACE, "org.lyraos.Vega1.System");
    }
}
