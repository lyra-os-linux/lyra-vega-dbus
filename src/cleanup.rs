use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt};

/// Capability announced in `Metadata.Capabilities` by daemons that implement
/// `org.lyraos.Vega1.Cleanup`.
pub const SPACE_CLEANUP_CAPABILITY: &str = "space-cleanup-v1";

/// System categories cleaned by vegad, in the order the frontend shows them.
pub const CLEANUP_CATEGORY_IDS: &[&str] = &[
    "coredumps",
    "journal",
    "package-cache",
    "flatpak-unused",
    "old-kernels",
    "snapshots",
];

/// Finding code of the diagnostics `platform` test when the root file system
/// is low on space; frontends offer a shortcut to the cleanup section.
pub const LOW_SPACE_FINDING: &str = "storage.low-space";

/// A category as reported by `Scan`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupCategory {
    pub id: String,
    /// Bytes cleaning frees; 0 when there is nothing or the size is unknown.
    pub bytes: u64,
    pub can_clean: bool,
    pub detail: CleanupDetail,
}

type CleanupCategoryRow = (String, u64, bool, String);

impl From<CleanupCategoryRow> for CleanupCategory {
    fn from(row: CleanupCategoryRow) -> Self {
        Self {
            id: row.0,
            bytes: row.1,
            can_clean: row.2,
            detail: CleanupDetail::parse(&row.3),
        }
    }
}

/// The `detail` tokens of a category. Unknown tokens are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupDetail {
    pub count: Option<u64>,
    /// The size cannot be measured before cleaning (`size=unknown`).
    pub size_unknown: bool,
    /// `count` is a lower bound (`minimum`).
    pub minimum: bool,
    /// Why `can_clean` is false (`nothing`, `busy`, `missing-tool:<package>`,
    /// `not-configured`); frontends localize it.
    pub reason: Option<String>,
    /// Entries such as kernel versions or runtime refs, at most 20.
    pub items: Vec<String>,
}

impl CleanupDetail {
    pub fn parse(value: &str) -> Self {
        let mut detail = Self::default();
        for token in value.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            match token.split_once('=') {
                Some(("count", count)) => detail.count = count.parse().ok(),
                Some(("size", "unknown")) => detail.size_unknown = true,
                Some(("reason", reason)) if !reason.is_empty() => {
                    detail.reason = Some(reason.to_owned());
                }
                Some(("item", item)) if !item.is_empty() => detail.items.push(item.to_owned()),
                None if token == "minimum" => detail.minimum = true,
                _ => {}
            }
        }
        detail
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupProgress {
    pub job_id: u32,
    pub id: String,
    pub percent: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupCategoryFinished {
    pub job_id: u32,
    pub id: String,
    /// `ok`, `failed` or `skipped`; newer values are passed through.
    pub result: String,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupFinished {
    pub job_id: u32,
    /// `ok`, `partial` or `failed`; newer values are passed through.
    pub result: String,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupEvent {
    Progress(CleanupProgress),
    CategoryFinished(CleanupCategoryFinished),
    Finished(CleanupFinished),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupClientError {
    /// The daemon does not implement the interface or is unreachable.
    Unavailable(String),
    /// A diagnostic, cleanup or a Zypper, snapshot or backup transaction runs.
    Busy(String),
    /// Unknown category id or empty list.
    NotSupported(String),
    Failed(String),
}

const ERROR_PREFIX: &str = "org.lyraos.Vega1.Error.";

impl CleanupClientError {
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

impl std::fmt::Display for CleanupClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (text, detail) = match self {
            Self::Unavailable(detail) => (
                gettextrs::gettext("interface de limpeza indisponível: {detail}"),
                detail,
            ),
            Self::Busy(detail) => (
                gettextrs::gettext("outra operação do sistema está em andamento: {detail}"),
                detail,
            ),
            Self::NotSupported(detail) => (
                gettextrs::gettext("categoria de limpeza não suportada: {detail}"),
                detail,
            ),
            Self::Failed(detail) => return f.write_str(detail),
        };
        write!(f, "{}", text.replace("{detail}", detail))
    }
}

impl std::error::Error for CleanupClientError {}

#[async_trait]
pub trait CleanupClient: Send + Sync {
    /// Measures every system category without deleting anything.
    async fn scan(&self) -> Result<Vec<CleanupCategory>, CleanupClientError>;
    /// Cleans the given categories in order; returns the job id.
    async fn clean(&self, ids: &[String]) -> Result<u32, CleanupClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Cleanup",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Cleanup {
    async fn scan(&self) -> zbus::Result<Vec<CleanupCategoryRow>>;
    async fn clean(&self, ids: &[String]) -> zbus::Result<u32>;

    #[zbus(signal)]
    async fn progress(
        &self,
        job_id: u32,
        id: &str,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn category_finished(
        &self,
        job_id: u32,
        id: &str,
        result: &str,
        freed_bytes: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn finished(&self, job_id: u32, result: &str, freed_bytes: u64) -> zbus::Result<()>;
}

pub struct ZbusCleanupClient {
    connection: zbus::Connection,
}

impl ZbusCleanupClient {
    pub async fn connect() -> Result<Self, CleanupClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(CleanupClientError::unavailable)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<CleanupProxy<'_>, CleanupClientError> {
        CleanupProxy::new(&self.connection)
            .await
            .map_err(CleanupClientError::unavailable)
    }

    /// Subscribe before calling `clean` so no early signal is missed.
    pub async fn subscribe(&self) -> Result<CleanupEventStream, CleanupClientError> {
        let proxy = self.proxy().await?;
        Ok(CleanupEventStream {
            progress: proxy
                .receive_progress()
                .await
                .map_err(CleanupClientError::unavailable)?,
            category_finished: proxy
                .receive_category_finished()
                .await
                .map_err(CleanupClientError::unavailable)?,
            finished: proxy
                .receive_finished()
                .await
                .map_err(CleanupClientError::unavailable)?,
        })
    }
}

pub struct CleanupEventStream {
    progress: ProgressStream,
    category_finished: CategoryFinishedStream,
    finished: FinishedStream,
}

impl CleanupEventStream {
    pub async fn next(&mut self) -> Result<CleanupEvent, CleanupClientError> {
        futures_util::select! {
            signal = self.progress.next().fuse() => {
                let signal = signal.ok_or_else(CleanupClientError::stream_ended)?;
                let args = signal.args().map_err(CleanupClientError::unavailable)?;
                Ok(CleanupEvent::Progress(CleanupProgress {
                    job_id: args.job_id,
                    id: args.id.to_owned(),
                    percent: args.percent,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.category_finished.next().fuse() => {
                let signal = signal.ok_or_else(CleanupClientError::stream_ended)?;
                let args = signal.args().map_err(CleanupClientError::unavailable)?;
                Ok(CleanupEvent::CategoryFinished(CleanupCategoryFinished {
                    job_id: args.job_id,
                    id: args.id.to_owned(),
                    result: args.result.to_owned(),
                    freed_bytes: args.freed_bytes,
                }))
            },
            signal = self.finished.next().fuse() => {
                let signal = signal.ok_or_else(CleanupClientError::stream_ended)?;
                let args = signal.args().map_err(CleanupClientError::unavailable)?;
                Ok(CleanupEvent::Finished(CleanupFinished {
                    job_id: args.job_id,
                    result: args.result.to_owned(),
                    freed_bytes: args.freed_bytes,
                }))
            },
        }
    }
}

#[async_trait]
impl CleanupClient for ZbusCleanupClient {
    async fn scan(&self) -> Result<Vec<CleanupCategory>, CleanupClientError> {
        self.proxy()
            .await?
            .scan()
            .await
            .map_err(CleanupClientError::from_error)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn clean(&self, ids: &[String]) -> Result<u32, CleanupClientError> {
        self.proxy()
            .await?
            .clean(ids)
            .await
            .map_err(CleanupClientError::from_error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const CLEANUP_XML: &str = include_str!("../dbus/org.lyraos.Vega1.Cleanup.xml");
    const INTERFACE: &str = "org.lyraos.Vega1.Cleanup";

    fn members(tag: &str) -> BTreeMap<String, Vec<(String, String)>> {
        let node_start = CLEANUP_XML.find("<node").expect("nó de introspecção");
        let document =
            roxmltree::Document::parse(&CLEANUP_XML[node_start..]).expect("Cleanup XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE)
            })
            .expect("interface Cleanup presente");

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
    fn cleanup_xml_methods_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            ("Clean".into(), args(&[("in", "as"), ("out", "u")])),
            ("Scan".into(), args(&[("out", "a(stbs)")])),
        ]);
        assert_eq!(members("method"), expected);
    }

    #[test]
    fn cleanup_xml_signals_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            (
                "CategoryFinished".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s"), ("out", "t")]),
            ),
            (
                "Finished".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "t")]),
            ),
            (
                "Progress".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "u"), ("out", "s")]),
            ),
        ]);
        assert_eq!(members("signal"), expected);
    }

    #[test]
    fn xml_documents_every_category_reason_and_error() {
        for id in CLEANUP_CATEGORY_IDS.iter().copied() {
            assert!(CLEANUP_XML.contains(id), "{id} ausente do XML");
        }
        for token in [
            "count=",
            "size=unknown",
            "minimum",
            "reason=",
            "item=",
            "missing-tool:",
            "not-configured",
            "Busy",
            "NotSupported",
            "org.lyraos.vega.storage.read-admin",
            "org.lyraos.vega.storage.cleanup",
            SPACE_CLEANUP_CAPABILITY,
        ] {
            assert!(CLEANUP_XML.contains(token), "{token} ausente do XML");
        }
    }

    #[test]
    fn detail_tokens_parse_and_unknown_ones_are_ignored() {
        let detail = CleanupDetail::parse(
            "count=4;size=unknown;minimum;item=6.12.0-160100.5-default;item=6.12.0-160100.6-default;future=1;novo",
        );
        assert_eq!(detail.count, Some(4));
        assert!(detail.size_unknown && detail.minimum);
        assert_eq!(detail.reason, None);
        assert_eq!(
            detail.items,
            ["6.12.0-160100.5-default", "6.12.0-160100.6-default"]
        );

        let blocked = CleanupDetail::parse("reason=missing-tool:snapper");
        assert_eq!(blocked.reason.as_deref(), Some("missing-tool:snapper"));
        assert_eq!(blocked.count, None);
        assert_eq!(CleanupDetail::parse(""), CleanupDetail::default());
        assert_eq!(CleanupDetail::parse("count=x").count, None);
    }

    #[test]
    fn rows_map_to_typed_categories() {
        let category =
            CleanupCategory::from(("coredumps".into(), 256_642_935, true, "count=30".into()));
        assert_eq!(category.id, "coredumps");
        assert_eq!(category.bytes, 256_642_935);
        assert!(category.can_clean);
        assert_eq!(category.detail.count, Some(30));
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
            CleanupClientError::from_error(method_error("org.lyraos.Vega1.Error.Busy")),
            CleanupClientError::Busy("detalhe".into())
        );
        assert_eq!(
            CleanupClientError::from_error(method_error("org.lyraos.Vega1.Error.NotSupported")),
            CleanupClientError::NotSupported("detalhe".into())
        );
        assert!(matches!(
            CleanupClientError::from_error(method_error(
                "org.freedesktop.DBus.Error.UnknownMethod"
            )),
            CleanupClientError::Unavailable(_)
        ));
        assert!(matches!(
            CleanupClientError::from_error(method_error(
                "org.freedesktop.PolicyKit1.Error.NotAuthorized"
            )),
            CleanupClientError::Failed(_)
        ));
    }

    #[test]
    #[ignore = "requer vegad com space-cleanup-v1 e acesso ao system bus"]
    fn real_daemon_scans_without_deleting() {
        futures_lite::future::block_on(async {
            let client = ZbusCleanupClient::connect().await.unwrap();
            let categories = client.scan().await.unwrap();
            let ids: Vec<&str> = categories.iter().map(|c| c.id.as_str()).collect();
            assert!(
                ids.contains(&"coredumps") && ids.contains(&"package-cache"),
                "{ids:?}"
            );
            drop(client.subscribe().await.unwrap());
        });
    }
}
