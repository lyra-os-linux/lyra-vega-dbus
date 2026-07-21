use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootStatus {
    pub loader: String,
    pub default_entry: String,
    pub timeout: u32,
    pub cmdline: String,
}

impl From<(String, String, u32, String)> for BootStatus {
    fn from(row: (String, String, u32, String)) -> Self {
        Self {
            loader: row.0,
            default_entry: row.1,
            timeout: row.2,
            cmdline: row.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelClientError(String);

impl std::fmt::Display for KernelClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de kernel indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for KernelClientError {}

impl KernelClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait KernelClient: Send + Sync {
    async fn list_installed(&self) -> Result<Vec<String>, KernelClientError>;
    async fn available_packages(&self) -> Result<Vec<String>, KernelClientError>;
    async fn boot_status(&self) -> Result<BootStatus, KernelClientError>;
    async fn list_boot_entries(&self) -> Result<Vec<String>, KernelClientError>;
    async fn install(&self, kernel: &str) -> Result<u32, KernelClientError>;
    async fn remove(&self, kernel: &str) -> Result<(), KernelClientError>;
    async fn apply_boot_config(
        &self,
        default_entry: &str,
        timeout: u32,
        cmdline: &str,
    ) -> Result<(), KernelClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Kernel",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Kernel {
    async fn list_installed(&self) -> zbus::Result<Vec<String>>;
    async fn available_packages(&self) -> zbus::Result<Vec<String>>;
    async fn boot_status(&self) -> zbus::Result<(String, String, u32, String)>;
    async fn list_boot_entries(&self) -> zbus::Result<Vec<String>>;
    async fn install(&self, kernel: &str) -> zbus::Result<u32>;
    async fn remove(&self, kernel: &str) -> zbus::Result<()>;
    async fn apply_boot_config(
        &self,
        default_entry: &str,
        timeout: u32,
        cmdline: &str,
    ) -> zbus::Result<()>;
}

pub struct ZbusKernelClient {
    connection: zbus::Connection,
}

impl ZbusKernelClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<KernelProxy<'_>, KernelClientError> {
        KernelProxy::new(&self.connection)
            .await
            .map_err(KernelClientError::from_error)
    }
}

macro_rules! call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self.proxy().await?.$method($($arg),*).await.map_err(KernelClientError::from_error)
    };
}

#[async_trait]
impl KernelClient for ZbusKernelClient {
    async fn list_installed(&self) -> Result<Vec<String>, KernelClientError> {
        call!(self, list_installed())
    }
    async fn available_packages(&self) -> Result<Vec<String>, KernelClientError> {
        call!(self, available_packages())
    }
    async fn boot_status(&self) -> Result<BootStatus, KernelClientError> {
        call!(self, boot_status()).map(Into::into)
    }
    async fn list_boot_entries(&self) -> Result<Vec<String>, KernelClientError> {
        call!(self, list_boot_entries())
    }
    async fn install(&self, kernel: &str) -> Result<u32, KernelClientError> {
        call!(self, install(kernel))
    }
    async fn remove(&self, kernel: &str) -> Result<(), KernelClientError> {
        call!(self, remove(kernel))
    }
    async fn apply_boot_config(
        &self,
        default_entry: &str,
        timeout: u32,
        cmdline: &str,
    ) -> Result<(), KernelClientError> {
        call!(self, apply_boot_config(default_entry, timeout, cmdline))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn kernel_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Kernel.xml");
        for method in [
            "ListInstalled",
            "AvailablePackages",
            "Install",
            "Remove",
            "BootStatus",
            "ListBootEntries",
            "ApplyBootConfig",
        ] {
            assert!(xml.contains(&format!("<method name=\"{method}\">")));
        }
    }
}
