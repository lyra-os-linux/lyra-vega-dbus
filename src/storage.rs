use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageVolume {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub fs_type: String,
    pub size: String,
    pub used: String,
    pub available: String,
    pub use_percent: u32,
    pub mountpoint: String,
    pub model: String,
    pub removable: bool,
    pub can_mount: bool,
    pub can_unmount: bool,
}

type StorageVolumeRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    u32,
    String,
    String,
    bool,
    bool,
    bool,
);

impl From<StorageVolumeRow> for StorageVolume {
    fn from(row: StorageVolumeRow) -> Self {
        Self {
            name: row.0,
            path: row.1,
            kind: row.2,
            fs_type: row.3,
            size: row.4,
            used: row.5,
            available: row.6,
            use_percent: row.7,
            mountpoint: row.8,
            model: row.9,
            removable: row.10,
            can_mount: row.11,
            can_unmount: row.12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageClientError(String);

impl std::fmt::Display for StorageClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de armazenamento indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for StorageClientError {}

impl StorageClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait StorageClient: Send + Sync {
    async fn list(&self) -> Result<Vec<StorageVolume>, StorageClientError>;
    async fn mount(&self, path: &str) -> Result<(), StorageClientError>;
    async fn unmount(&self, path: &str) -> Result<(), StorageClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Storage",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Storage {
    async fn list_volumes(&self) -> zbus::Result<Vec<StorageVolumeRow>>;
    async fn mount(&self, path: &str) -> zbus::Result<()>;
    async fn unmount(&self, path: &str) -> zbus::Result<()>;
}

pub struct ZbusStorageClient {
    connection: zbus::Connection,
}

impl ZbusStorageClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }
    async fn proxy(&self) -> Result<StorageProxy<'_>, StorageClientError> {
        StorageProxy::new(&self.connection)
            .await
            .map_err(StorageClientError::from_error)
    }
}

#[async_trait]
impl StorageClient for ZbusStorageClient {
    async fn list(&self) -> Result<Vec<StorageVolume>, StorageClientError> {
        self.proxy()
            .await?
            .list_volumes()
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(StorageClientError::from_error)
    }
    async fn mount(&self, path: &str) -> Result<(), StorageClientError> {
        self.proxy()
            .await?
            .mount(path)
            .await
            .map_err(StorageClientError::from_error)
    }
    async fn unmount(&self, path: &str) -> Result<(), StorageClientError> {
        self.proxy()
            .await?
            .unmount(path)
            .await
            .map_err(StorageClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn storage_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Storage.xml");
        for method in ["ListVolumes", "Mount", "Unmount"] {
            assert!(xml.contains(&format!("<method name=\"{method}\">")));
        }
    }
}
