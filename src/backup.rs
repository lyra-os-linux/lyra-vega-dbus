use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupConfig {
    pub id: String,
    pub paths: Vec<String>,
    pub destination: String,
    pub destination_uuid: String,
    pub frequency: String,
}

type BackupConfigRow = (String, Vec<String>, String, String, String);

impl From<BackupConfigRow> for BackupConfig {
    fn from(row: BackupConfigRow) -> Self {
        Self {
            id: row.0,
            paths: row.1,
            destination: row.2,
            destination_uuid: row.3,
            frequency: row.4,
        }
    }
}

impl From<BackupConfig> for BackupConfigRow {
    fn from(config: BackupConfig) -> Self {
        (
            config.id,
            config.paths,
            config.destination,
            config.destination_uuid,
            config.frequency,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSnapshot {
    pub id: String,
    pub timestamp: i64,
    pub file_count: u64,
    pub size_bytes: u64,
}

type BackupSnapshotRow = (String, i64, u64, u64);

impl From<BackupSnapshotRow> for BackupSnapshot {
    fn from(row: BackupSnapshotRow) -> Self {
        Self {
            id: row.0,
            timestamp: row.1,
            file_count: row.2,
            size_bytes: row.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupTransactionProgress {
    pub transaction_id: u32,
    pub percent: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupTransactionFinished {
    pub transaction_id: u32,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupAlertEvent {
    pub config_id: String,
    pub consecutive_failures: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupEvent {
    BackupProgress(BackupTransactionProgress),
    BackupFinished(BackupTransactionFinished),
    RestoreProgress(BackupTransactionProgress),
    RestoreFinished(BackupTransactionFinished),
    Alert(BackupAlertEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupClientError {
    Unavailable(String),
}

impl std::fmt::Display for BackupClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(detail) => write!(
                f,
                "{}",
                gettextrs::gettext("interface de backup indisponível: {detail}")
                    .replace("{detail}", detail)
            ),
        }
    }
}

impl std::error::Error for BackupClientError {}

#[async_trait]
pub trait BackupClient: Send + Sync {
    async fn create_config(&self, config: BackupConfig) -> Result<String, BackupClientError>;
    async fn list_configs(&self) -> Result<Vec<BackupConfig>, BackupClientError>;
    async fn run_now(&self, config_id: &str) -> Result<u32, BackupClientError>;
    async fn list_snapshots(
        &self,
        config_id: &str,
    ) -> Result<Vec<BackupSnapshot>, BackupClientError>;
    async fn list_snapshot_paths(
        &self,
        config_id: &str,
        snapshot_id: &str,
    ) -> Result<Vec<String>, BackupClientError>;
    async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
    ) -> Result<u32, BackupClientError>;
    async fn restore_items(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
        paths: &[String],
    ) -> Result<u32, BackupClientError>;
    async fn delete_config(&self, config_id: &str) -> Result<(), BackupClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Backup",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Backup {
    async fn create_config(&self, config: BackupConfigRow) -> zbus::Result<String>;
    async fn list_configs(&self) -> zbus::Result<Vec<BackupConfigRow>>;
    async fn run_backup_now(&self, config_id: &str) -> zbus::Result<u32>;
    async fn list_snapshots(&self, config_id: &str) -> zbus::Result<Vec<BackupSnapshotRow>>;
    async fn list_snapshot_paths(
        &self,
        config_id: &str,
        snapshot_id: &str,
    ) -> zbus::Result<Vec<String>>;
    async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
    ) -> zbus::Result<u32>;
    async fn restore_items(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
        paths: &[String],
    ) -> zbus::Result<u32>;
    async fn delete_config(&self, config_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn backup_progress(
        &self,
        transaction_id: u32,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn backup_finished(
        &self,
        transaction_id: u32,
        success: bool,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn restore_progress(
        &self,
        transaction_id: u32,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn restore_finished(
        &self,
        transaction_id: u32,
        success: bool,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn backup_alert(
        &self,
        config_id: &str,
        consecutive_failures: u32,
        message: &str,
    ) -> zbus::Result<()>;
}

pub struct ZbusBackupClient {
    connection: zbus::Connection,
}

impl ZbusBackupClient {
    pub async fn connect() -> Result<Self, BackupClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(BackupClientError::unavailable)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<BackupProxy<'_>, BackupClientError> {
        BackupProxy::new(&self.connection)
            .await
            .map_err(BackupClientError::unavailable)
    }

    pub async fn subscribe(&self) -> Result<BackupEventStream, BackupClientError> {
        let proxy = self.proxy().await?;
        Ok(BackupEventStream {
            backup_progress: proxy
                .receive_backup_progress()
                .await
                .map_err(BackupClientError::unavailable)?,
            backup_finished: proxy
                .receive_backup_finished()
                .await
                .map_err(BackupClientError::unavailable)?,
            restore_progress: proxy
                .receive_restore_progress()
                .await
                .map_err(BackupClientError::unavailable)?,
            restore_finished: proxy
                .receive_restore_finished()
                .await
                .map_err(BackupClientError::unavailable)?,
            alerts: proxy
                .receive_backup_alert()
                .await
                .map_err(BackupClientError::unavailable)?,
        })
    }
}

pub struct BackupEventStream {
    backup_progress: BackupProgressStream,
    backup_finished: BackupFinishedStream,
    restore_progress: RestoreProgressStream,
    restore_finished: RestoreFinishedStream,
    alerts: BackupAlertStream,
}

impl BackupEventStream {
    pub async fn next(&mut self) -> Result<BackupEvent, BackupClientError> {
        futures_util::select! {
            signal = self.backup_progress.next().fuse() => {
                let signal = signal.ok_or_else(BackupClientError::stream_ended)?;
                let args = signal.args().map_err(BackupClientError::unavailable)?;
                Ok(BackupEvent::BackupProgress(BackupTransactionProgress {
                    transaction_id: args.transaction_id,
                    percent: args.percent,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.backup_finished.next().fuse() => {
                let signal = signal.ok_or_else(BackupClientError::stream_ended)?;
                let args = signal.args().map_err(BackupClientError::unavailable)?;
                Ok(BackupEvent::BackupFinished(BackupTransactionFinished {
                    transaction_id: args.transaction_id,
                    success: args.success,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.restore_progress.next().fuse() => {
                let signal = signal.ok_or_else(BackupClientError::stream_ended)?;
                let args = signal.args().map_err(BackupClientError::unavailable)?;
                Ok(BackupEvent::RestoreProgress(BackupTransactionProgress {
                    transaction_id: args.transaction_id,
                    percent: args.percent,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.restore_finished.next().fuse() => {
                let signal = signal.ok_or_else(BackupClientError::stream_ended)?;
                let args = signal.args().map_err(BackupClientError::unavailable)?;
                Ok(BackupEvent::RestoreFinished(BackupTransactionFinished {
                    transaction_id: args.transaction_id,
                    success: args.success,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.alerts.next().fuse() => {
                let signal = signal.ok_or_else(BackupClientError::stream_ended)?;
                let args = signal.args().map_err(BackupClientError::unavailable)?;
                Ok(BackupEvent::Alert(BackupAlertEvent {
                    config_id: args.config_id.to_owned(),
                    consecutive_failures: args.consecutive_failures,
                    message: args.message.to_owned(),
                }))
            },
        }
    }
}

impl BackupClientError {
    fn unavailable(error: impl std::fmt::Display) -> Self {
        Self::Unavailable(error.to_string())
    }

    fn stream_ended() -> Self {
        Self::Unavailable("stream de sinais encerrado".into())
    }
}

macro_rules! proxy_call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self.proxy().await?.$method($($arg),*).await.map_err(BackupClientError::unavailable)
    };
}

#[async_trait]
impl BackupClient for ZbusBackupClient {
    async fn create_config(&self, config: BackupConfig) -> Result<String, BackupClientError> {
        proxy_call!(self, create_config(config.into()))
    }

    async fn list_configs(&self) -> Result<Vec<BackupConfig>, BackupClientError> {
        proxy_call!(self, list_configs()).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn run_now(&self, config_id: &str) -> Result<u32, BackupClientError> {
        proxy_call!(self, run_backup_now(config_id))
    }

    async fn list_snapshots(
        &self,
        config_id: &str,
    ) -> Result<Vec<BackupSnapshot>, BackupClientError> {
        proxy_call!(self, list_snapshots(config_id))
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn list_snapshot_paths(
        &self,
        config_id: &str,
        snapshot_id: &str,
    ) -> Result<Vec<String>, BackupClientError> {
        proxy_call!(self, list_snapshot_paths(config_id, snapshot_id))
    }

    async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
    ) -> Result<u32, BackupClientError> {
        proxy_call!(self, restore_snapshot(snapshot_id, target_path, mode))
    }

    async fn restore_items(
        &self,
        snapshot_id: &str,
        target_path: &str,
        mode: &str,
        paths: &[String],
    ) -> Result<u32, BackupClientError> {
        proxy_call!(self, restore_items(snapshot_id, target_path, mode, paths))
    }

    async fn delete_config(&self, config_id: &str) -> Result<(), BackupClientError> {
        proxy_call!(self, delete_config(config_id))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::BackupClient;

    const BACKUP_XML: &str = include_str!("../../dbus/org.lyraos.Vega1.Backup.xml");
    const INTERFACE: &str = "org.lyraos.Vega1.Backup";

    fn members(tag: &str) -> BTreeMap<String, Vec<(String, String)>> {
        let node_start = BACKUP_XML.find("<node").expect("nó de introspecção");
        let document =
            roxmltree::Document::parse(&BACKUP_XML[node_start..]).expect("Backup XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE)
            })
            .expect("interface Backup presente");

        interface
            .children()
            .filter(|node| node.has_tag_name(tag))
            .map(|member| {
                let name = member.attribute("name").expect("membro com nome");
                let args = member
                    .children()
                    .filter(|node| node.has_tag_name("arg"))
                    .map(|arg| {
                        (
                            arg.attribute("direction").unwrap_or("out").to_owned(),
                            arg.attribute("type")
                                .expect("argumento com tipo")
                                .to_owned(),
                        )
                    })
                    .collect();
                (name.to_owned(), args)
            })
            .collect()
    }

    fn args(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(direction, signature)| ((*direction).into(), (*signature).into()))
            .collect()
    }

    #[test]
    fn backup_xml_methods_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            (
                "CreateConfig".into(),
                args(&[("in", "(sassss)"), ("out", "s")]),
            ),
            ("DeleteConfig".into(), args(&[("in", "s")])),
            ("ListConfigs".into(), args(&[("out", "a(sassss)")])),
            (
                "ListSnapshotPaths".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "as")]),
            ),
            (
                "ListSnapshots".into(),
                args(&[("in", "s"), ("out", "a(sxtt)")]),
            ),
            (
                "RestoreItems".into(),
                args(&[
                    ("in", "s"),
                    ("in", "s"),
                    ("in", "s"),
                    ("in", "as"),
                    ("out", "u"),
                ]),
            ),
            (
                "RestoreSnapshot".into(),
                args(&[("in", "s"), ("in", "s"), ("in", "s"), ("out", "u")]),
            ),
            ("RunBackupNow".into(), args(&[("in", "s"), ("out", "u")])),
        ]);
        assert_eq!(members("method"), expected);
    }

    #[test]
    fn backup_xml_signals_match_the_typed_proxy() {
        let progress = args(&[("out", "u"), ("out", "u"), ("out", "s")]);
        let finished = args(&[("out", "u"), ("out", "b"), ("out", "s")]);
        let expected = BTreeMap::from([
            (
                "BackupAlert".into(),
                args(&[("out", "s"), ("out", "u"), ("out", "s")]),
            ),
            ("BackupFinished".into(), finished.clone()),
            ("BackupProgress".into(), progress.clone()),
            ("RestoreFinished".into(), finished),
            ("RestoreProgress".into(), progress),
        ]);
        assert_eq!(members("signal"), expected);
    }

    #[test]
    fn config_rows_preserve_paths_and_destination_uuid() {
        let config = super::BackupConfig::from((
            "home".into(),
            vec!["/home/user".into(), "/etc".into()],
            "/backup".into(),
            "uuid-123".into(),
            "daily".into(),
        ));
        assert_eq!(config.paths.len(), 2);
        assert_eq!(config.destination_uuid, "uuid-123");
        assert_eq!(config.frequency, "daily");
    }

    #[test]
    #[ignore = "requer vegad instalado e acesso ao system bus"]
    fn real_daemon_exposes_the_read_only_backup_contract() {
        futures_lite::future::block_on(async {
            let client = super::ZbusBackupClient::connect().await.unwrap();
            client.list_configs().await.unwrap();
            let subscriptions = client.subscribe().await.unwrap();
            drop(subscriptions);
        });
    }
}
