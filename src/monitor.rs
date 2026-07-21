use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMetrics {
    pub cpu_percent: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub cpu_per_core: Vec<f64>,
}

type MetricsRow = (f64, u64, u64, u64, u64, u64, u64, u64, u64, Vec<f64>);

impl From<MetricsRow> for SystemMetrics {
    fn from(row: MetricsRow) -> Self {
        Self {
            cpu_percent: row.0,
            mem_used: row.1,
            mem_total: row.2,
            swap_used: row.3,
            swap_total: row.4,
            disk_read_bytes: row.5,
            disk_write_bytes: row.6,
            net_rx_bytes: row.7,
            net_tx_bytes: row.8,
            cpu_per_core: row.9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub user: String,
    pub cpu_percent: NotNan,
    pub memory: u64,
    pub state: String,
}

type ProcessRow = (u32, u32, String, String, f64, u64, String);

impl From<ProcessRow> for ProcessInfo {
    fn from(row: ProcessRow) -> Self {
        Self {
            pid: row.0,
            ppid: row.1,
            name: row.2,
            user: row.3,
            cpu_percent: NotNan(row.4),
            memory: row.5,
            state: row.6,
        }
    }
}

/// `f64` puro não implementa `Eq`/`Ord` (por causa de NaN); o daemon nunca
/// devolve NaN aqui (é sempre um percentual calculado a partir de contadores
/// de CPU), então um wrapper simples basta — sem precisar de uma dependência
/// externa só para isso.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotNan(f64);

impl NotNan {
    pub fn get(self) -> f64 {
        self.0
    }
}

impl From<f64> for NotNan {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl Eq for NotNan {}

impl PartialOrd for NotNan {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NotNan {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorClientError(String);

impl std::fmt::Display for MonitorClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de monitor indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for MonitorClientError {}

impl MonitorClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait MonitorClient: Send + Sync {
    async fn metrics(&self) -> Result<SystemMetrics, MonitorClientError>;
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>, MonitorClientError>;
    async fn kill_process(&self, pid: u32) -> Result<(), MonitorClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Monitor",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Monitor {
    async fn metrics(&self) -> zbus::Result<MetricsRow>;
    async fn list_processes(&self) -> zbus::Result<Vec<ProcessRow>>;
    async fn kill_process(&self, pid: u32) -> zbus::Result<()>;
}

pub struct ZbusMonitorClient {
    connection: zbus::Connection,
}

impl ZbusMonitorClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }
    async fn proxy(&self) -> Result<MonitorProxy<'_>, MonitorClientError> {
        MonitorProxy::new(&self.connection)
            .await
            .map_err(MonitorClientError::from_error)
    }
}

macro_rules! call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self.proxy().await?.$method($($arg),*).await.map_err(MonitorClientError::from_error)
    };
}

#[async_trait]
impl MonitorClient for ZbusMonitorClient {
    async fn metrics(&self) -> Result<SystemMetrics, MonitorClientError> {
        call!(self, metrics()).map(Into::into)
    }
    async fn list_processes(&self) -> Result<Vec<ProcessInfo>, MonitorClientError> {
        call!(self, list_processes())
            .map(|rows: Vec<ProcessRow>| rows.into_iter().map(Into::into).collect())
    }
    async fn kill_process(&self, pid: u32) -> Result<(), MonitorClientError> {
        call!(self, kill_process(pid))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn monitor_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Monitor.xml");
        let start = xml.find("<node").unwrap();
        let document = roxmltree::Document::parse(&xml[start..]).unwrap();
        let mut methods = document
            .descendants()
            .filter(|node| node.has_tag_name("method"))
            .map(|node| node.attribute("name").unwrap())
            .collect::<Vec<_>>();
        methods.sort_unstable();
        assert_eq!(methods, ["KillProcess", "ListProcesses", "Metrics"]);
    }
}
