use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInfo {
    pub username: String,
    pub is_admin: bool,
}

impl From<(String, bool)> for UserInfo {
    fn from(row: (String, bool)) -> Self {
        Self {
            username: row.0,
            is_admin: row.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsersClientError(String);

impl std::fmt::Display for UsersClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            gettextrs::gettext("interface de usuários indisponível: {detail}")
                .replace("{detail}", &self.0)
        )
    }
}

impl std::error::Error for UsersClientError {}

impl UsersClientError {
    fn from_error(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
pub trait UsersClient: Send + Sync {
    async fn list(&self) -> Result<Vec<UserInfo>, UsersClientError>;
    async fn create(&self, username: &str, is_admin: bool) -> Result<(), UsersClientError>;
    async fn remove(&self, username: &str) -> Result<(), UsersClientError>;
    async fn set_admin(&self, username: &str, is_admin: bool) -> Result<(), UsersClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Users",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Users {
    async fn list_users(&self) -> zbus::Result<Vec<(String, bool)>>;
    async fn create_user(&self, username: &str, is_admin: bool) -> zbus::Result<()>;
    async fn remove_user(&self, username: &str) -> zbus::Result<()>;
    async fn set_admin(&self, username: &str, is_admin: bool) -> zbus::Result<()>;
}

pub struct ZbusUsersClient {
    connection: zbus::Connection,
}

impl ZbusUsersClient {
    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<UsersProxy<'_>, UsersClientError> {
        UsersProxy::new(&self.connection)
            .await
            .map_err(UsersClientError::from_error)
    }
}

#[async_trait]
impl UsersClient for ZbusUsersClient {
    async fn list(&self) -> Result<Vec<UserInfo>, UsersClientError> {
        self.proxy()
            .await?
            .list_users()
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(UsersClientError::from_error)
    }

    async fn create(&self, username: &str, is_admin: bool) -> Result<(), UsersClientError> {
        self.proxy()
            .await?
            .create_user(username, is_admin)
            .await
            .map_err(UsersClientError::from_error)
    }

    async fn remove(&self, username: &str) -> Result<(), UsersClientError> {
        self.proxy()
            .await?
            .remove_user(username)
            .await
            .map_err(UsersClientError::from_error)
    }

    async fn set_admin(&self, username: &str, is_admin: bool) -> Result<(), UsersClientError> {
        self.proxy()
            .await?
            .set_admin(username, is_admin)
            .await
            .map_err(UsersClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn users_xml_contains_every_typed_method() {
        let xml = include_str!("../../dbus/org.lyraos.Vega1.Users.xml");
        let start = xml.find("<node").unwrap();
        let document = roxmltree::Document::parse(&xml[start..]).unwrap();
        let mut methods = document
            .descendants()
            .filter(|node| node.has_tag_name("method"))
            .map(|node| node.attribute("name").unwrap())
            .collect::<Vec<_>>();
        methods.sort_unstable();
        assert_eq!(
            methods,
            ["CreateUser", "ListUsers", "RemoveUser", "SetAdmin"]
        );
    }
}
