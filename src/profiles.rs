use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt};

/// Capability announced in `Metadata.Capabilities` by daemons that implement
/// `org.lyraos.Vega1.Profiles`.
pub const SYSTEM_PROFILES_CAPABILITY: &str = "system-profiles-v1";

/// Kernel values for the SAP ABAP Platform Trial.
pub const SAP_ABAP_TRIAL_PROFILE: &str = "sap-abap-trial";
/// Real-time limits for the `audio` group and `pipewire-jack`.
pub const AUDIO_LOWLATENCY_PROFILE: &str = "audio-lowlatency";
/// `preempt=full threadirqs` on the kernel command line (next boot).
pub const AUDIO_LOWLATENCY_KERNEL_PROFILE: &str = "audio-lowlatency-kernel";

/// Every profile vegad knows; any other id is rejected.
pub const SYSTEM_PROFILE_IDS: &[&str] = &[
    SAP_ABAP_TRIAL_PROFILE,
    AUDIO_LOWLATENCY_PROFILE,
    AUDIO_LOWLATENCY_KERNEL_PROFILE,
];

/// A profile as reported by `ListSystemProfiles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfile {
    pub id: String,
    pub enabled: bool,
    /// The last change only takes full effect after a reboot or new login.
    pub restart_pending: bool,
}

type SystemProfileRow = (String, bool, bool);

impl From<SystemProfileRow> for SystemProfile {
    fn from(row: SystemProfileRow) -> Self {
        Self {
            id: row.0,
            enabled: row.1,
            restart_pending: row.2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfileProgress {
    pub job_id: u32,
    pub id: String,
    pub percent: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfileFinished {
    pub job_id: u32,
    pub id: String,
    /// `ok` or `failed`; newer values are passed through.
    pub result: String,
    /// Why the job failed; the steps already done were undone.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileEvent {
    Progress(SystemProfileProgress),
    Finished(SystemProfileFinished),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfilesClientError {
    /// The daemon does not implement the interface or is unreachable.
    Unavailable(String),
    /// Another profile job or a system transaction is running.
    Busy(String),
    /// Unknown profile id.
    NotSupported(String),
    Failed(String),
}

const ERROR_PREFIX: &str = "org.lyraos.Vega1.Error.";

impl ProfilesClientError {
    fn from_error(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, detail, _) => {
                let detail = detail.clone().unwrap_or_default();
                match name.as_str().strip_prefix(ERROR_PREFIX) {
                    Some("Busy") => return Self::Busy(detail),
                    Some("NotSupported") => return Self::NotSupported(detail),
                    _ => {}
                }
                if matches!(
                    name.as_str(),
                    "org.freedesktop.DBus.Error.UnknownInterface"
                        | "org.freedesktop.DBus.Error.UnknownMethod"
                ) {
                    return Self::Unavailable(error.to_string());
                }
                Self::Failed(error.to_string())
            }
            zbus::Error::FDO(fdo)
                if matches!(
                    fdo.as_ref(),
                    zbus::fdo::Error::UnknownInterface(_) | zbus::fdo::Error::UnknownMethod(_)
                ) =>
            {
                Self::Unavailable(error.to_string())
            }
            _ => Self::Failed(error.to_string()),
        }
    }

    fn unavailable(error: impl std::fmt::Display) -> Self {
        Self::Unavailable(error.to_string())
    }

    fn stream_ended() -> Self {
        Self::Unavailable("stream de sinais encerrado".into())
    }
}

impl std::fmt::Display for ProfilesClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (text, detail) = match self {
            Self::Unavailable(detail) => (
                gettextrs::gettext("interface de perfis do sistema indisponível: {detail}"),
                detail,
            ),
            Self::Busy(detail) => (
                gettextrs::gettext("outra operação do sistema está em andamento: {detail}"),
                detail,
            ),
            Self::NotSupported(detail) => (
                gettextrs::gettext("perfil do sistema não suportado: {detail}"),
                detail,
            ),
            Self::Failed(detail) => return f.write_str(detail),
        };
        write!(f, "{}", text.replace("{detail}", detail))
    }
}

impl std::error::Error for ProfilesClientError {}

#[async_trait]
pub trait ProfilesClient: Send + Sync {
    /// Every known profile with its current state.
    async fn list_system_profiles(&self) -> Result<Vec<SystemProfile>, ProfilesClientError>;
    /// Enables or disables one profile; returns the job id.
    async fn set_system_profile(&self, id: &str, enabled: bool)
    -> Result<u32, ProfilesClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Profiles",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Profiles {
    async fn list_system_profiles(&self) -> zbus::Result<Vec<SystemProfileRow>>;
    async fn set_system_profile(&self, id: &str, enabled: bool) -> zbus::Result<u32>;

    #[zbus(signal)]
    async fn profile_progress(
        &self,
        job_id: u32,
        id: &str,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn profile_finished(
        &self,
        job_id: u32,
        id: &str,
        result: &str,
        detail: &str,
    ) -> zbus::Result<()>;
}

pub struct ZbusProfilesClient {
    connection: zbus::Connection,
}

impl ZbusProfilesClient {
    pub async fn connect() -> Result<Self, ProfilesClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(ProfilesClientError::unavailable)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<ProfilesProxy<'_>, ProfilesClientError> {
        ProfilesProxy::new(&self.connection)
            .await
            .map_err(ProfilesClientError::unavailable)
    }

    /// Subscribe before calling `set_system_profile` so no early signal is
    /// missed.
    pub async fn subscribe(&self) -> Result<ProfileEventStream, ProfilesClientError> {
        let proxy = self.proxy().await?;
        Ok(ProfileEventStream {
            progress: proxy
                .receive_profile_progress()
                .await
                .map_err(ProfilesClientError::unavailable)?,
            finished: proxy
                .receive_profile_finished()
                .await
                .map_err(ProfilesClientError::unavailable)?,
        })
    }
}

pub struct ProfileEventStream {
    progress: ProfileProgressStream,
    finished: ProfileFinishedStream,
}

impl ProfileEventStream {
    pub async fn next(&mut self) -> Result<ProfileEvent, ProfilesClientError> {
        futures_util::select! {
            signal = self.progress.next().fuse() => {
                let signal = signal.ok_or_else(ProfilesClientError::stream_ended)?;
                let args = signal.args().map_err(ProfilesClientError::unavailable)?;
                Ok(ProfileEvent::Progress(SystemProfileProgress {
                    job_id: args.job_id,
                    id: args.id.to_owned(),
                    percent: args.percent,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.finished.next().fuse() => {
                let signal = signal.ok_or_else(ProfilesClientError::stream_ended)?;
                let args = signal.args().map_err(ProfilesClientError::unavailable)?;
                Ok(ProfileEvent::Finished(SystemProfileFinished {
                    job_id: args.job_id,
                    id: args.id.to_owned(),
                    result: args.result.to_owned(),
                    detail: args.detail.to_owned(),
                }))
            },
        }
    }
}

#[async_trait]
impl ProfilesClient for ZbusProfilesClient {
    async fn list_system_profiles(&self) -> Result<Vec<SystemProfile>, ProfilesClientError> {
        self.proxy()
            .await?
            .list_system_profiles()
            .await
            .map_err(ProfilesClientError::from_error)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn set_system_profile(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<u32, ProfilesClientError> {
        self.proxy()
            .await?
            .set_system_profile(id, enabled)
            .await
            .map_err(ProfilesClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const PROFILES_XML: &str = include_str!("../dbus/org.lyraos.Vega1.Profiles.xml");
    const INTERFACE: &str = "org.lyraos.Vega1.Profiles";

    fn members(tag: &str) -> BTreeMap<String, Vec<(String, String)>> {
        let node_start = PROFILES_XML.find("<node").expect("nó de introspecção");
        let document =
            roxmltree::Document::parse(&PROFILES_XML[node_start..]).expect("Profiles XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE)
            })
            .expect("interface Profiles presente");

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
    fn profiles_xml_methods_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            ("ListSystemProfiles".into(), args(&[("out", "a(sbb)")])),
            (
                "SetSystemProfile".into(),
                args(&[("in", "s"), ("in", "b"), ("out", "u")]),
            ),
        ]);
        assert_eq!(members("method"), expected);
    }

    #[test]
    fn profiles_xml_signals_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            (
                "ProfileFinished".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s"), ("out", "s")]),
            ),
            (
                "ProfileProgress".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "u"), ("out", "s")]),
            ),
        ]);
        assert_eq!(members("signal"), expected);
    }

    #[test]
    fn xml_documents_every_profile_and_error() {
        for id in SYSTEM_PROFILE_IDS.iter().copied() {
            assert!(PROFILES_XML.contains(id), "{id} ausente do XML");
        }
        for token in [
            "Busy",
            "NotSupported",
            "org.lyraos.vega.profiles.manage",
            "auth_admin",
            SYSTEM_PROFILES_CAPABILITY,
        ] {
            assert!(PROFILES_XML.contains(token), "{token} ausente do XML");
        }
    }

    #[test]
    fn rows_map_to_typed_profiles() {
        let profile = SystemProfile::from((AUDIO_LOWLATENCY_KERNEL_PROFILE.into(), true, true));
        assert_eq!(profile.id, "audio-lowlatency-kernel");
        assert!(profile.enabled && profile.restart_pending);
    }

    #[test]
    fn daemon_errors_map_to_typed_variants() {
        let method_error = |name: &str| {
            let name = zbus::names::OwnedErrorName::try_from(name.to_owned()).unwrap();
            let message = zbus::message::Message::method_call("/", "Ping")
                .unwrap()
                .build(&())
                .unwrap();
            zbus::Error::MethodError(name, Some("detalhe".into()), message)
        };
        assert_eq!(
            ProfilesClientError::from_error(method_error("org.lyraos.Vega1.Error.Busy")),
            ProfilesClientError::Busy("detalhe".into())
        );
        assert_eq!(
            ProfilesClientError::from_error(method_error("org.lyraos.Vega1.Error.NotSupported")),
            ProfilesClientError::NotSupported("detalhe".into())
        );
        assert!(matches!(
            ProfilesClientError::from_error(method_error(
                "org.freedesktop.DBus.Error.UnknownMethod"
            )),
            ProfilesClientError::Unavailable(_)
        ));
        assert!(matches!(
            ProfilesClientError::from_error(method_error(
                "org.freedesktop.PolicyKit1.Error.NotAuthorized"
            )),
            ProfilesClientError::Failed(_)
        ));
    }

    #[test]
    #[ignore = "requer vegad com system-profiles-v1 e acesso ao system bus"]
    fn real_daemon_lists_every_known_profile() {
        futures_lite::future::block_on(async {
            let client = ZbusProfilesClient::connect().await.unwrap();
            let profiles = client.list_system_profiles().await.unwrap();
            let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
            assert_eq!(ids, SYSTEM_PROFILE_IDS);
            drop(client.subscribe().await.unwrap());
        });
    }
}
