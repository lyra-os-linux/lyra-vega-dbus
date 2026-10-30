use std::collections::HashMap;

use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt};
use zbus::zvariant::{OwnedValue, Value};

/// Capability announced in `Metadata.Capabilities` by daemons that implement
/// `org.lyraos.Vega1.Diagnostics`.
pub const DIAGNOSTICS_CAPABILITY: &str = "diagnostics-v1";

/// Capability of daemons that also implement `Repair`.
pub const DIAGNOSTICS_REPAIR_CAPABILITY: &str = "diagnostics-v2";

/// Remedy ids `Repair` accepts, as they appear in a report's findings.
/// `enable-firewall`, `close-port` and `apply-security-patches` need
/// `diagnostics-v3`; the network ones need `diagnostics-v4`.
pub const DIAGNOSTIC_REMEDIES: &[&str] = &[
    "reinstall-packages",
    "verify-dependencies",
    "restart-units",
    "enable-ntp",
    "enable-firewall",
    "close-port",
    "apply-security-patches",
    "reconnect",
    "renew-dhcp",
    "temporary-dns",
];

/// Capability of daemons with the `security` test and its remedies.
pub const DIAGNOSTICS_SECURITY_CAPABILITY: &str = "diagnostics-v3";

/// Capability of daemons with the `network` test and its remedies
/// (`reconnect`, `renew-dhcp`, `temporary-dns`).
pub const DIAGNOSTICS_NETWORK_CAPABILITY: &str = "diagnostics-v4";

/// Capability of daemons with `Remedies`, `RepairFindings` and
/// `MarkLogsSeen`: a fix per test or per finding, and "seen" for the logs.
pub const DIAGNOSTICS_ACTIONS_CAPABILITY: &str = "diagnostics-actions-v1";

/// A finding with a remedy, as `Remedies` reports it: `finding_index` is
/// the position of the finding inside its stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRemedy {
    pub stage: String,
    pub finding_index: u32,
    pub remedy: String,
}

type DiagnosticRemedyRow = (String, u32, String);

impl From<DiagnosticRemedyRow> for DiagnosticRemedy {
    fn from(row: DiagnosticRemedyRow) -> Self {
        Self {
            stage: row.0,
            finding_index: row.1,
            remedy: row.2,
        }
    }
}

/// Test ids run by vegad, in the order the daemon recommends running them.
/// The GPU test runs in the frontend and is not part of the contract.
/// `security` needs `diagnostics-v3`, `network` needs `diagnostics-v4`.
pub const DIAGNOSTIC_TEST_IDS: &[&str] = &[
    "packages",
    "disk-health",
    "fs-scrub",
    "cpu-stress",
    "memory-stress",
    "disk-bench",
    "logs",
    "platform",
    "security",
    "network",
];

/// A test as reported by `ListTests`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTest {
    pub id: String,
    pub available: bool,
    /// Stable reason when `available` is false (`missing-tool:<package>`,
    /// `not-btrfs`, `not-implemented`); empty otherwise. Frontends localize it.
    pub reason: String,
    pub estimated_seconds: u32,
    pub requires_ac: bool,
}

type DiagnosticTestRow = (String, bool, String, u32, bool);

impl From<DiagnosticTestRow> for DiagnosticTest {
    fn from(row: DiagnosticTestRow) -> Self {
        Self {
            id: row.0,
            available: row.1,
            reason: row.2,
            estimated_seconds: row.3,
            requires_ac: row.4,
        }
    }
}

/// Optional knobs for `Start`. `None` keeps the daemon default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticOptions {
    /// cpu-stress/memory-stress duration, 30..600 s (default 240).
    pub duration_seconds: Option<u32>,
    /// Share of MemAvailable used by memory-stress, 10..70 (default 70).
    pub memory_percent: Option<u32>,
    /// Boots examined by the logs test, 1..20 (default 8).
    pub boots_back: Option<u32>,
    pub locale: Option<String>,
}

impl DiagnosticOptions {
    fn to_dict(&self) -> HashMap<&'static str, Value<'_>> {
        let mut dict = HashMap::new();
        if let Some(value) = self.duration_seconds {
            dict.insert("durationSeconds", Value::from(value));
        }
        if let Some(value) = self.memory_percent {
            dict.insert("memoryPercent", Value::from(value));
        }
        if let Some(value) = self.boots_back {
            dict.insert("bootsBack", Value::from(value));
        }
        if let Some(value) = &self.locale {
            dict.insert("locale", Value::from(value.as_str()));
        }
        dict
    }
}

/// The running job (`Current`); `job_id` is 0 when the daemon is idle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticJob {
    pub job_id: u32,
    pub tests: Vec<String>,
    pub stage: String,
    pub percent: u32,
}

type DiagnosticJobRow = (u32, Vec<String>, String, u32);

impl From<DiagnosticJobRow> for DiagnosticJob {
    fn from(row: DiagnosticJobRow) -> Self {
        Self {
            job_id: row.0,
            tests: row.1,
            stage: row.2,
            percent: row.3,
        }
    }
}

impl DiagnosticJob {
    pub fn is_running(&self) -> bool {
        self.job_id != 0
    }
}

/// Stage verdicts and job results. Unknown values from a newer daemon are
/// kept in `Other` instead of failing the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticVerdict {
    Ok,
    Attention,
    Fail,
    Unavailable,
    Cancelled,
    Aborted,
    Other(String),
}

impl DiagnosticVerdict {
    pub fn parse(value: &str) -> Self {
        match value {
            "ok" => Self::Ok,
            "attention" => Self::Attention,
            "fail" => Self::Fail,
            "unavailable" => Self::Unavailable,
            "cancelled" => Self::Cancelled,
            "aborted" => Self::Aborted,
            other => Self::Other(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Ok => "ok",
            Self::Attention => "attention",
            Self::Fail => "fail",
            Self::Unavailable => "unavailable",
            Self::Cancelled => "cancelled",
            Self::Aborted => "aborted",
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticProgress {
    pub job_id: u32,
    pub stage: String,
    pub percent: u32,
    pub message: String,
}

/// Live telemetry; `None` means the daemon did not measure that value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiagnosticSample {
    pub job_id: u32,
    pub pkg_temp_c: Option<i32>,
    pub max_core_temp_c: Option<i32>,
    pub avg_mhz: Option<u32>,
    pub throttle_events: Option<u32>,
    pub nvme_temp_c: Option<i32>,
    pub load1: Option<f64>,
}

impl DiagnosticSample {
    fn from_dict(job_id: u32, dict: &HashMap<String, OwnedValue>) -> Self {
        fn get<T: TryFrom<OwnedValue>>(dict: &HashMap<String, OwnedValue>, key: &str) -> Option<T> {
            dict.get(key)
                .and_then(|value| value.try_clone().ok())
                .and_then(|value| T::try_from(value).ok())
        }
        Self {
            job_id,
            pkg_temp_c: get(dict, "pkgTempC"),
            max_core_temp_c: get(dict, "maxCoreTempC"),
            avg_mhz: get(dict, "avgMHz"),
            throttle_events: get(dict, "throttleEvents"),
            nvme_temp_c: get(dict, "nvmeTempC"),
            load1: get(dict, "load1"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticStageFinished {
    pub job_id: u32,
    pub stage: String,
    pub verdict: DiagnosticVerdict,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticFinished {
    pub job_id: u32,
    pub result: DiagnosticVerdict,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticsEvent {
    Progress(DiagnosticProgress),
    Sample(DiagnosticSample),
    StageFinished(DiagnosticStageFinished),
    Finished(DiagnosticFinished),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticsClientError {
    /// The daemon does not implement the interface or is unreachable.
    Unavailable(String),
    /// Another diagnostic or a Zypper, snapshot or backup transaction runs.
    Busy(String),
    /// A stress or benchmark test was requested without line power.
    OnBattery(String),
    /// Not enough available memory or free disk space.
    InsufficientResources(String),
    /// Unknown test id or invalid option.
    NotSupported(String),
    Failed(String),
}

const ERROR_PREFIX: &str = "org.lyraos.Vega1.Error.";

impl DiagnosticsClientError {
    fn from_error(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, detail, _) => {
                let detail = detail.clone().unwrap_or_default();
                match name.as_str().strip_prefix(ERROR_PREFIX) {
                    Some("Busy") => return Self::Busy(detail),
                    Some("OnBattery") => return Self::OnBattery(detail),
                    Some("InsufficientResources") => return Self::InsufficientResources(detail),
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

impl std::fmt::Display for DiagnosticsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (text, detail) = match self {
            Self::Unavailable(detail) => (
                gettextrs::gettext("interface de diagnóstico indisponível: {detail}"),
                detail,
            ),
            Self::Busy(detail) => (
                gettextrs::gettext("outra operação do sistema está em andamento: {detail}"),
                detail,
            ),
            Self::OnBattery(detail) => (
                gettextrs::gettext("conecte o computador à tomada: {detail}"),
                detail,
            ),
            Self::InsufficientResources(detail) => (
                gettextrs::gettext("recursos insuficientes para o teste: {detail}"),
                detail,
            ),
            Self::NotSupported(detail) => {
                (gettextrs::gettext("teste não suportado: {detail}"), detail)
            }
            Self::Failed(detail) => return f.write_str(detail),
        };
        write!(f, "{}", text.replace("{detail}", detail))
    }
}

impl std::error::Error for DiagnosticsClientError {}

#[async_trait]
pub trait DiagnosticsClient: Send + Sync {
    async fn list_tests(&self) -> Result<Vec<DiagnosticTest>, DiagnosticsClientError>;
    async fn start(
        &self,
        tests: &[String],
        options: &DiagnosticOptions,
    ) -> Result<u32, DiagnosticsClientError>;
    async fn cancel(&self, job_id: u32) -> Result<(), DiagnosticsClientError>;
    async fn current(&self) -> Result<DiagnosticJob, DiagnosticsClientError>;
    /// JSON report (schema 1); `job_id` 0 returns the most recent job.
    async fn report(&self, job_id: u32, redact: bool) -> Result<String, DiagnosticsClientError>;
    /// Applies the given remedies of `job_id`'s report; returns the repair job.
    async fn repair(&self, job_id: u32, remedies: &[String])
    -> Result<u32, DiagnosticsClientError>;
    /// Findings with a remedy in `job_id`'s report (0 = most recent).
    async fn remedies(&self, job_id: u32) -> Result<Vec<DiagnosticRemedy>, DiagnosticsClientError>;
    /// Fixes only the given `(stage, finding_index)` findings.
    async fn repair_findings(
        &self,
        job_id: u32,
        findings: &[(String, u32)],
    ) -> Result<u32, DiagnosticsClientError>;
    /// Later logs tests ignore what happened before `job_id`'s logs stage.
    async fn mark_logs_seen(&self, job_id: u32) -> Result<(), DiagnosticsClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Diagnostics",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Diagnostics {
    async fn list_tests(&self) -> zbus::Result<Vec<DiagnosticTestRow>>;
    async fn start(&self, tests: &[String], options: HashMap<&str, Value<'_>>)
    -> zbus::Result<u32>;
    async fn cancel(&self, job_id: u32) -> zbus::Result<()>;
    async fn current(&self) -> zbus::Result<DiagnosticJobRow>;
    async fn report(&self, job_id: u32, redact: bool) -> zbus::Result<String>;
    async fn repair(&self, job_id: u32, remedies: &[String]) -> zbus::Result<u32>;
    async fn remedies(&self, job_id: u32) -> zbus::Result<Vec<DiagnosticRemedyRow>>;
    async fn repair_findings(&self, job_id: u32, findings: &[(String, u32)]) -> zbus::Result<u32>;
    async fn mark_logs_seen(&self, job_id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn progress(
        &self,
        job_id: u32,
        stage: &str,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn sample(&self, job_id: u32, sample: HashMap<String, OwnedValue>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn stage_finished(
        &self,
        job_id: u32,
        stage: &str,
        verdict: &str,
        summary: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn finished(&self, job_id: u32, result: &str, message: &str) -> zbus::Result<()>;
}

pub struct ZbusDiagnosticsClient {
    connection: zbus::Connection,
}

impl ZbusDiagnosticsClient {
    pub async fn connect() -> Result<Self, DiagnosticsClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(DiagnosticsClientError::unavailable)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<DiagnosticsProxy<'_>, DiagnosticsClientError> {
        DiagnosticsProxy::new(&self.connection)
            .await
            .map_err(DiagnosticsClientError::unavailable)
    }

    /// Subscribe before calling `start` so no early signal is missed.
    pub async fn subscribe(&self) -> Result<DiagnosticsEventStream, DiagnosticsClientError> {
        let proxy = self.proxy().await?;
        Ok(DiagnosticsEventStream {
            progress: proxy
                .receive_progress()
                .await
                .map_err(DiagnosticsClientError::unavailable)?,
            sample: proxy
                .receive_sample()
                .await
                .map_err(DiagnosticsClientError::unavailable)?,
            stage_finished: proxy
                .receive_stage_finished()
                .await
                .map_err(DiagnosticsClientError::unavailable)?,
            finished: proxy
                .receive_finished()
                .await
                .map_err(DiagnosticsClientError::unavailable)?,
        })
    }
}

pub struct DiagnosticsEventStream {
    progress: ProgressStream,
    sample: SampleStream,
    stage_finished: StageFinishedStream,
    finished: FinishedStream,
}

impl DiagnosticsEventStream {
    pub async fn next(&mut self) -> Result<DiagnosticsEvent, DiagnosticsClientError> {
        futures_util::select! {
            signal = self.progress.next().fuse() => {
                let signal = signal.ok_or_else(DiagnosticsClientError::stream_ended)?;
                let args = signal.args().map_err(DiagnosticsClientError::unavailable)?;
                Ok(DiagnosticsEvent::Progress(DiagnosticProgress {
                    job_id: args.job_id,
                    stage: args.stage.to_owned(),
                    percent: args.percent,
                    message: args.message.to_owned(),
                }))
            },
            signal = self.sample.next().fuse() => {
                let signal = signal.ok_or_else(DiagnosticsClientError::stream_ended)?;
                let args = signal.args().map_err(DiagnosticsClientError::unavailable)?;
                Ok(DiagnosticsEvent::Sample(DiagnosticSample::from_dict(
                    args.job_id,
                    &args.sample,
                )))
            },
            signal = self.stage_finished.next().fuse() => {
                let signal = signal.ok_or_else(DiagnosticsClientError::stream_ended)?;
                let args = signal.args().map_err(DiagnosticsClientError::unavailable)?;
                Ok(DiagnosticsEvent::StageFinished(DiagnosticStageFinished {
                    job_id: args.job_id,
                    stage: args.stage.to_owned(),
                    verdict: DiagnosticVerdict::parse(args.verdict),
                    summary: args.summary.to_owned(),
                }))
            },
            signal = self.finished.next().fuse() => {
                let signal = signal.ok_or_else(DiagnosticsClientError::stream_ended)?;
                let args = signal.args().map_err(DiagnosticsClientError::unavailable)?;
                Ok(DiagnosticsEvent::Finished(DiagnosticFinished {
                    job_id: args.job_id,
                    result: DiagnosticVerdict::parse(args.result),
                    message: args.message.to_owned(),
                }))
            },
        }
    }
}

macro_rules! proxy_call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self
            .proxy()
            .await?
            .$method($($arg),*)
            .await
            .map_err(DiagnosticsClientError::from_error)
    };
}

#[async_trait]
impl DiagnosticsClient for ZbusDiagnosticsClient {
    async fn list_tests(&self) -> Result<Vec<DiagnosticTest>, DiagnosticsClientError> {
        proxy_call!(self, list_tests()).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn start(
        &self,
        tests: &[String],
        options: &DiagnosticOptions,
    ) -> Result<u32, DiagnosticsClientError> {
        proxy_call!(self, start(tests, options.to_dict()))
    }

    async fn cancel(&self, job_id: u32) -> Result<(), DiagnosticsClientError> {
        proxy_call!(self, cancel(job_id))
    }

    async fn current(&self) -> Result<DiagnosticJob, DiagnosticsClientError> {
        proxy_call!(self, current()).map(Into::into)
    }

    async fn report(&self, job_id: u32, redact: bool) -> Result<String, DiagnosticsClientError> {
        proxy_call!(self, report(job_id, redact))
    }

    async fn repair(
        &self,
        job_id: u32,
        remedies: &[String],
    ) -> Result<u32, DiagnosticsClientError> {
        proxy_call!(self, repair(job_id, remedies))
    }

    async fn remedies(&self, job_id: u32) -> Result<Vec<DiagnosticRemedy>, DiagnosticsClientError> {
        proxy_call!(self, remedies(job_id)).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn repair_findings(
        &self,
        job_id: u32,
        findings: &[(String, u32)],
    ) -> Result<u32, DiagnosticsClientError> {
        proxy_call!(self, repair_findings(job_id, findings))
    }

    async fn mark_logs_seen(&self, job_id: u32) -> Result<(), DiagnosticsClientError> {
        proxy_call!(self, mark_logs_seen(job_id))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use zbus::zvariant::{OwnedValue, Value};

    use super::*;

    const DIAGNOSTICS_XML: &str = include_str!("../dbus/org.lyraos.Vega1.Diagnostics.xml");
    const INTERFACE: &str = "org.lyraos.Vega1.Diagnostics";

    fn members(tag: &str) -> BTreeMap<String, Vec<(String, String)>> {
        let node_start = DIAGNOSTICS_XML.find("<node").expect("nó de introspecção");
        let document = roxmltree::Document::parse(&DIAGNOSTICS_XML[node_start..])
            .expect("Diagnostics XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE)
            })
            .expect("interface Diagnostics presente");

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
    fn diagnostics_xml_methods_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            ("Cancel".into(), args(&[("in", "u")])),
            ("Current".into(), args(&[("out", "(uassu)")])),
            ("ListTests".into(), args(&[("out", "a(sbsub)")])),
            ("MarkLogsSeen".into(), args(&[("in", "u")])),
            ("Remedies".into(), args(&[("in", "u"), ("out", "a(sus)")])),
            (
                "Report".into(),
                args(&[("in", "u"), ("in", "b"), ("out", "s")]),
            ),
            (
                "Repair".into(),
                args(&[("in", "u"), ("in", "as"), ("out", "u")]),
            ),
            (
                "RepairFindings".into(),
                args(&[("in", "u"), ("in", "a(su)"), ("out", "u")]),
            ),
            (
                "Start".into(),
                args(&[("in", "as"), ("in", "a{sv}"), ("out", "u")]),
            ),
        ]);
        assert_eq!(members("method"), expected);
    }

    #[test]
    fn diagnostics_xml_signals_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            (
                "Finished".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s")]),
            ),
            (
                "Progress".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "u"), ("out", "s")]),
            ),
            ("Sample".into(), args(&[("out", "u"), ("out", "a{sv}")])),
            (
                "StageFinished".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s"), ("out", "s")]),
            ),
        ]);
        assert_eq!(members("signal"), expected);
    }

    #[test]
    fn xml_documents_every_test_id_and_error() {
        for id in DIAGNOSTIC_TEST_IDS.iter().copied() {
            assert!(DIAGNOSTICS_XML.contains(id), "{id} ausente do XML");
        }
        for error in ["Busy", "OnBattery", "InsufficientResources", "NotSupported"] {
            assert!(DIAGNOSTICS_XML.contains(error), "{error} ausente do XML");
        }
        assert!(DIAGNOSTICS_XML.contains(DIAGNOSTICS_CAPABILITY));
        assert!(DIAGNOSTICS_XML.contains(DIAGNOSTICS_REPAIR_CAPABILITY));
        assert!(DIAGNOSTICS_XML.contains(DIAGNOSTICS_SECURITY_CAPABILITY));
        assert!(DIAGNOSTICS_XML.contains(DIAGNOSTICS_ACTIONS_CAPABILITY));
        assert!(DIAGNOSTICS_XML.contains(DIAGNOSTICS_NETWORK_CAPABILITY));
        assert!(DIAGNOSTICS_XML.contains("org.lyraos.vega.network.diagnose"));
        for remedy in DIAGNOSTIC_REMEDIES.iter().copied() {
            assert!(DIAGNOSTICS_XML.contains(remedy), "{remedy} ausente do XML");
        }
    }

    #[test]
    fn options_send_only_the_values_that_were_set() {
        assert!(DiagnosticOptions::default().to_dict().is_empty());
        let options = DiagnosticOptions {
            duration_seconds: Some(60),
            memory_percent: None,
            boots_back: Some(3),
            locale: Some("pt_BR".into()),
        };
        let dict = options.to_dict();
        assert_eq!(dict.len(), 3);
        assert_eq!(dict["durationSeconds"], Value::from(60u32));
        assert_eq!(dict["bootsBack"], Value::from(3u32));
        assert_eq!(dict["locale"], Value::from("pt_BR"));
    }

    #[test]
    fn samples_keep_measured_keys_and_ignore_unknown_or_mistyped_ones() {
        let mut dict: HashMap<String, OwnedValue> = HashMap::new();
        dict.insert("pkgTempC".into(), OwnedValue::from(81i32));
        dict.insert("avgMHz".into(), OwnedValue::from(3600u32));
        dict.insert("load1".into(), OwnedValue::from(11.5f64));
        dict.insert("nvmeTempC".into(), OwnedValue::from(40u32)); // tipo errado
        dict.insert("futureKey".into(), OwnedValue::from(1u32));
        let sample = DiagnosticSample::from_dict(7, &dict);
        assert_eq!(sample.job_id, 7);
        assert_eq!(sample.pkg_temp_c, Some(81));
        assert_eq!(sample.avg_mhz, Some(3600));
        assert_eq!(sample.load1, Some(11.5));
        assert_eq!(sample.nvme_temp_c, None);
        assert_eq!(sample.max_core_temp_c, None);
        assert_eq!(sample.throttle_events, None);
    }

    #[test]
    fn verdicts_round_trip_and_keep_unknown_values() {
        for value in [
            "ok",
            "attention",
            "fail",
            "unavailable",
            "cancelled",
            "aborted",
        ] {
            assert_eq!(DiagnosticVerdict::parse(value).as_str(), value);
        }
        assert_eq!(
            DiagnosticVerdict::parse("skipped"),
            DiagnosticVerdict::Other("skipped".into())
        );
    }

    #[test]
    fn rows_map_to_typed_structs() {
        let test = DiagnosticTest::from((
            "disk-bench".into(),
            false,
            "missing-tool:fio".into(),
            120,
            true,
        ));
        assert!(!test.available && test.requires_ac);
        assert_eq!(test.estimated_seconds, 120);

        let remedy = DiagnosticRemedy::from(("security".into(), 2, "close-port".into()));
        assert_eq!(remedy.stage, "security");
        assert_eq!(remedy.finding_index, 2);
        assert_eq!(remedy.remedy, "close-port");

        let idle = DiagnosticJob::from((0, vec![], String::new(), 0));
        assert!(!idle.is_running());
        let running = DiagnosticJob::from((4, vec!["cpu-stress".into()], "cpu-stress".into(), 40));
        assert!(running.is_running());
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
            DiagnosticsClientError::from_error(method_error("org.lyraos.Vega1.Error.Busy")),
            DiagnosticsClientError::Busy("detalhe".into())
        );
        assert_eq!(
            DiagnosticsClientError::from_error(method_error("org.lyraos.Vega1.Error.OnBattery")),
            DiagnosticsClientError::OnBattery("detalhe".into())
        );
        assert_eq!(
            DiagnosticsClientError::from_error(method_error(
                "org.lyraos.Vega1.Error.InsufficientResources"
            )),
            DiagnosticsClientError::InsufficientResources("detalhe".into())
        );
        assert_eq!(
            DiagnosticsClientError::from_error(method_error("org.lyraos.Vega1.Error.NotSupported")),
            DiagnosticsClientError::NotSupported("detalhe".into())
        );
        assert!(matches!(
            DiagnosticsClientError::from_error(method_error(
                "org.freedesktop.DBus.Error.UnknownInterface"
            )),
            DiagnosticsClientError::Unavailable(_)
        ));
        assert!(matches!(
            DiagnosticsClientError::from_error(method_error(
                "org.freedesktop.PolicyKit1.Error.NotAuthorized"
            )),
            DiagnosticsClientError::Failed(_)
        ));
    }

    #[test]
    #[ignore = "requer vegad com diagnostics-v1 e acesso ao system bus"]
    fn real_daemon_exposes_the_read_only_diagnostics_contract() {
        futures_lite::future::block_on(async {
            let client = ZbusDiagnosticsClient::connect().await.unwrap();
            let tests = client.list_tests().await.unwrap();
            // A newer daemon may announce tests this client does not know yet
            // (gated by a newer capability); every test this client knows
            // must still be there.
            let ids: Vec<&str> = tests.iter().map(|test| test.id.as_str()).collect();
            assert!(
                ids.contains(&"packages") && ids.contains(&"logs"),
                "{ids:?}"
            );
            assert!(
                ids.iter()
                    .all(|id| !id.is_empty()
                        && id.chars().all(|c| c.is_ascii_lowercase() || c == '-')),
                "{ids:?}"
            );
            client.current().await.unwrap();
            drop(client.subscribe().await.unwrap());
        });
    }
}
