use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogsClientError(String);

impl std::fmt::Display for LogsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de logs indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for LogsClientError {}

impl LogsClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait LogsClient: Send + Sync {
    async fn list_units(&self) -> Result<Vec<String>, LogsClientError>;
    async fn query(
        &self,
        unit: &str,
        priority: &str,
        since: &str,
        search: &str,
        max_lines: u32,
    ) -> Result<Vec<String>, LogsClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Logs",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Logs {
    async fn list_units(&self) -> zbus::Result<Vec<String>>;
    async fn query(
        &self,
        unit: &str,
        priority: &str,
        since: &str,
        search: &str,
        max_lines: u32,
    ) -> zbus::Result<Vec<String>>;
}

pub struct ZbusLogsClient {
    connection: zbus::Connection,
}

impl ZbusLogsClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<LogsProxy<'_>, LogsClientError> {
        LogsProxy::new(&self.connection)
            .await
            .map_err(LogsClientError::from_error)
    }
}

#[async_trait]
impl LogsClient for ZbusLogsClient {
    async fn list_units(&self) -> Result<Vec<String>, LogsClientError> {
        self.proxy()
            .await?
            .list_units()
            .await
            .map_err(LogsClientError::from_error)
    }

    async fn query(
        &self,
        unit: &str,
        priority: &str,
        since: &str,
        search: &str,
        max_lines: u32,
    ) -> Result<Vec<String>, LogsClientError> {
        self.proxy()
            .await?
            .query(unit, priority, since, search, max_lines)
            .await
            .map_err(LogsClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logs_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Logs.xml");
        let start = xml.find("<node").unwrap();
        let document = roxmltree::Document::parse(&xml[start..]).unwrap();
        let mut methods = document
            .descendants()
            .filter(|node| node.has_tag_name("method"))
            .map(|node| node.attribute("name").unwrap())
            .collect::<Vec<_>>();
        methods.sort_unstable();
        assert_eq!(methods, ["ListUnits", "Query"]);
    }
}
