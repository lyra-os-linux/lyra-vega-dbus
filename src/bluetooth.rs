use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothStatus {
    pub available: bool,
    pub powered: bool,
    pub discoverable: bool,
    pub pairable: bool,
    pub scanning: bool,
    pub controller: String,
    pub controller_name: String,
    pub transfer_available: bool,
    pub receiver_active: bool,
    pub receive_path: String,
}

type StatusRow = (
    bool,
    bool,
    bool,
    bool,
    bool,
    String,
    String,
    bool,
    bool,
    String,
);

impl From<StatusRow> for BluetoothStatus {
    fn from(row: StatusRow) -> Self {
        Self {
            available: row.0,
            powered: row.1,
            discoverable: row.2,
            pairable: row.3,
            scanning: row.4,
            controller: row.5,
            controller_name: row.6,
            transfer_available: row.7,
            receiver_active: row.8,
            receive_path: row.9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub alias: String,
    pub icon: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    pub blocked: bool,
    pub rssi: i32,
}

type DeviceRow = (String, String, String, String, bool, bool, bool, bool, i32);

impl From<DeviceRow> for BluetoothDevice {
    fn from(row: DeviceRow) -> Self {
        Self {
            address: row.0,
            name: row.1,
            alias: row.2,
            icon: row.3,
            paired: row.4,
            trusted: row.5,
            connected: row.6,
            blocked: row.7,
            rssi: row.8,
        }
    }
}

impl BluetoothDevice {
    pub fn display_name(&self) -> &str {
        if !self.alias.is_empty() {
            &self.alias
        } else if !self.name.is_empty() {
            &self.name
        } else {
            &self.address
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothClientError(String);

impl std::fmt::Display for BluetoothClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface Bluetooth indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for BluetoothClientError {}

impl BluetoothClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait BluetoothClient: Send + Sync {
    async fn status(&self) -> Result<BluetoothStatus, BluetoothClientError>;
    async fn devices(&self) -> Result<Vec<BluetoothDevice>, BluetoothClientError>;
    async fn set_powered(&self, powered: bool) -> Result<(), BluetoothClientError>;
    async fn set_scanning(&self, scanning: bool) -> Result<(), BluetoothClientError>;
    async fn pair(&self, address: &str) -> Result<(), BluetoothClientError>;
    async fn connect(&self, address: &str) -> Result<(), BluetoothClientError>;
    async fn disconnect(&self, address: &str) -> Result<(), BluetoothClientError>;
    async fn send_file(&self, address: &str, path: &str) -> Result<(), BluetoothClientError>;
    async fn start_receiver(&self, directory: &str) -> Result<(), BluetoothClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Bluetooth",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Bluetooth {
    async fn status(&self) -> zbus::Result<StatusRow>;
    async fn list_devices(&self) -> zbus::Result<Vec<DeviceRow>>;
    async fn set_powered(&self, powered: bool) -> zbus::Result<()>;
    async fn set_scanning(&self, scanning: bool) -> zbus::Result<()>;
    async fn pair(&self, address: &str) -> zbus::Result<()>;
    async fn connect(&self, address: &str) -> zbus::Result<()>;
    async fn disconnect(&self, address: &str) -> zbus::Result<()>;
    async fn send_file(&self, address: &str, path: &str) -> zbus::Result<()>;
    async fn start_file_receiver(&self, directory: &str) -> zbus::Result<()>;
}

pub struct ZbusBluetoothClient {
    connection: zbus::Connection,
}

impl ZbusBluetoothClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<BluetoothProxy<'_>, BluetoothClientError> {
        BluetoothProxy::new(&self.connection)
            .await
            .map_err(BluetoothClientError::from_error)
    }
}

#[async_trait]
impl BluetoothClient for ZbusBluetoothClient {
    async fn status(&self) -> Result<BluetoothStatus, BluetoothClientError> {
        self.proxy()
            .await?
            .status()
            .await
            .map(Into::into)
            .map_err(BluetoothClientError::from_error)
    }

    async fn devices(&self) -> Result<Vec<BluetoothDevice>, BluetoothClientError> {
        self.proxy()
            .await?
            .list_devices()
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(BluetoothClientError::from_error)
    }

    async fn set_powered(&self, powered: bool) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .set_powered(powered)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn set_scanning(&self, scanning: bool) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .set_scanning(scanning)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn pair(&self, address: &str) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .pair(address)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn connect(&self, address: &str) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .connect(address)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn disconnect(&self, address: &str) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .disconnect(address)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn send_file(&self, address: &str, path: &str) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .send_file(address, path)
            .await
            .map_err(BluetoothClientError::from_error)
    }

    async fn start_receiver(&self, directory: &str) -> Result<(), BluetoothClientError> {
        self.proxy()
            .await?
            .start_file_receiver(directory)
            .await
            .map_err(BluetoothClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bluetooth_xml_contract() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Bluetooth.xml");
        for method in [
            "Status",
            "ListDevices",
            "SetPowered",
            "SetScanning",
            "Pair",
            "Connect",
            "Disconnect",
            "SendFile",
            "StartFileReceiver",
        ] {
            assert!(xml.contains(&format!("<method name=\"{method}\">")));
        }
        assert!(xml.contains("type=\"(bbbbbssbbs)\""));
        assert!(!xml.contains("type=\"(bbbbbsssbs)\""));
    }
}
