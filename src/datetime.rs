use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeStatus {
    pub timezone: String,
    pub ntp: bool,
    pub locale: String,
    pub keymap: String,
}

impl From<(String, bool, String, String)> for DateTimeStatus {
    fn from(row: (String, bool, String, String)) -> Self {
        Self {
            timezone: row.0,
            ntp: row.1,
            locale: row.2,
            keymap: row.3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeClientError(String);

impl std::fmt::Display for DateTimeClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de data e hora indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for DateTimeClientError {}

impl DateTimeClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait DateTimeClient: Send + Sync {
    async fn status(&self) -> Result<DateTimeStatus, DateTimeClientError>;
    async fn list_timezones(&self) -> Result<Vec<String>, DateTimeClientError>;
    async fn list_locales(&self) -> Result<Vec<String>, DateTimeClientError>;
    async fn list_keymaps(&self) -> Result<Vec<String>, DateTimeClientError>;
    async fn apply(
        &self,
        timezone: &str,
        ntp: bool,
        locale: &str,
        keymap: &str,
    ) -> Result<(), DateTimeClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.DateTime",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait DateTime {
    async fn status(&self) -> zbus::Result<(String, bool, String, String)>;
    async fn list_timezones(&self) -> zbus::Result<Vec<String>>;
    async fn list_locales(&self) -> zbus::Result<Vec<String>>;
    async fn list_keymaps(&self) -> zbus::Result<Vec<String>>;
    async fn apply(
        &self,
        timezone: &str,
        ntp: bool,
        locale: &str,
        keymap: &str,
    ) -> zbus::Result<()>;
}

pub struct ZbusDateTimeClient {
    connection: zbus::Connection,
}

impl ZbusDateTimeClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<DateTimeProxy<'_>, DateTimeClientError> {
        DateTimeProxy::new(&self.connection)
            .await
            .map_err(DateTimeClientError::from_error)
    }
}

macro_rules! call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self.proxy().await?.$method($($arg),*).await.map_err(DateTimeClientError::from_error)
    };
}

#[async_trait]
impl DateTimeClient for ZbusDateTimeClient {
    async fn status(&self) -> Result<DateTimeStatus, DateTimeClientError> {
        call!(self, status()).map(Into::into)
    }
    async fn list_timezones(&self) -> Result<Vec<String>, DateTimeClientError> {
        call!(self, list_timezones())
    }
    async fn list_locales(&self) -> Result<Vec<String>, DateTimeClientError> {
        call!(self, list_locales())
    }
    async fn list_keymaps(&self) -> Result<Vec<String>, DateTimeClientError> {
        call!(self, list_keymaps())
    }
    async fn apply(
        &self,
        timezone: &str,
        ntp: bool,
        locale: &str,
        keymap: &str,
    ) -> Result<(), DateTimeClientError> {
        call!(self, apply(timezone, ntp, locale, keymap))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn datetime_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.DateTime.xml");
        for method in [
            "Status",
            "ListTimezones",
            "ListLocales",
            "ListKeymaps",
            "Apply",
        ] {
            assert!(xml.contains(&format!("<method name=\"{method}\">")));
        }
    }
}
