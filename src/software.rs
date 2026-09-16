use async_trait::async_trait;
use futures_util::{FutureExt, StreamExt};
use std::time::{Duration, Instant};
use zbus::names::OwnedUniqueName;

const SERVICE: &str = "org.lyraos.Vega1";
/// Absolute observation limit; expiry never cancels or retries the daemon operation.
pub const SOFTWARE_TRANSACTION_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRef {
    pub origin: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub icon: String,
    /// Source repository's display name, populated for pending updates so
    /// the UI can group the Updates tab by it ("Flathub" for that origin,
    /// the zypper repo's Name for "official"). Empty where grouping
    /// doesn't apply (Search, ListInstalled).
    pub repository: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRef {
    pub name: String,
    pub enabled: bool,
}

type RepositoryRefRow = (String, bool);

impl From<RepositoryRefRow> for RepositoryRef {
    fn from(row: RepositoryRefRow) -> Self {
        Self {
            name: row.0,
            enabled: row.1,
        }
    }
}

type PackageRefRow = (String, String, String, String, bool, String, String);

impl From<PackageRefRow> for PackageRef {
    fn from(row: PackageRefRow) -> Self {
        Self {
            origin: row.0,
            id: row.1,
            name: row.2,
            description: row.3,
            installed: row.4,
            icon: row.5,
            repository: row.6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDetails {
    pub origin: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub installed_version: String,
    pub available_version: String,
    pub download_size: String,
    pub installed_size: String,
    pub dependencies: Vec<String>,
    pub licenses: Vec<String>,
    pub url: String,
    pub maintainer: String,
}

type PackageDetailsRow = (
    String,
    String,
    String,
    String,
    bool,
    String,
    String,
    String,
    String,
    Vec<String>,
    Vec<String>,
    String,
    String,
);

impl From<PackageDetailsRow> for PackageDetails {
    fn from(row: PackageDetailsRow) -> Self {
        Self {
            origin: row.0,
            id: row.1,
            name: row.2,
            description: row.3,
            installed: row.4,
            installed_version: row.5,
            available_version: row.6,
            download_size: row.7,
            installed_size: row.8,
            dependencies: row.9,
            licenses: row.10,
            url: row.11,
            maintainer: row.12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareTransactionProgress {
    pub transaction_id: u32,
    pub percent: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareTransactionFinished {
    pub transaction_id: u32,
    pub success: bool,
    pub message: String,
}

/// Fine-grained, per-package progress alongside a transaction's overall
/// SoftwareTransactionProgress — only emitted for Install/Remove/UpdateAll
/// on the "official" origin, driven by zypper --xmlout. phase is the wire
/// value verbatim, "download" | "install" (kept as a plain String rather
/// than an enum, matching how PackageRef.origin is handled in this file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwarePackageProgress {
    pub transaction_id: u32,
    pub package: String,
    pub phase: String,
    pub percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareConsoleLine {
    pub transaction_id: u32,
    pub source: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaStatus {
    pub supported: bool,
    pub installed: bool,
    pub reboot_required: bool,
    pub gpu: String,
    pub secure_boot: String,
    pub state: String,
    pub detail: String,
    pub recovery_snapshot: u32,
}

type NvidiaStatusRow = (bool, bool, bool, String, String, String, String, u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaRecovery {
    pub available: bool,
    pub kind: String,
    pub reference: String,
    pub state: String,
    pub detail: String,
}

type NvidiaRecoveryRow = (bool, String, String, String, String);
impl From<NvidiaRecoveryRow> for NvidiaRecovery {
    fn from(row: NvidiaRecoveryRow) -> Self {
        Self {
            available: row.0,
            kind: row.1,
            reference: row.2,
            state: row.3,
            detail: row.4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonFreeFirmwareStatus {
    pub detected: bool,
    pub installed: bool,
    pub detail: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatus {
    pub checked_at: String,
    pub profile: String,
    pub native_count: u32,
    pub flatpak_count: u32,
    pub total_count: u32,
    pub security_count: u32,
    pub in_progress: bool,
    pub error: String,
}

type UpdateStatusRow = (String, String, u32, u32, u32, u32, bool, String);

impl From<UpdateStatusRow> for UpdateStatus {
    fn from(row: UpdateStatusRow) -> Self {
        Self {
            checked_at: row.0,
            profile: row.1,
            native_count: row.2,
            flatpak_count: row.3,
            total_count: row.4,
            security_count: row.5,
            in_progress: row.6,
            error: row.7,
        }
    }
}

type NonFreeFirmwareStatusRow = (bool, bool, String, Vec<String>);

impl From<NonFreeFirmwareStatusRow> for NonFreeFirmwareStatus {
    fn from(row: NonFreeFirmwareStatusRow) -> Self {
        Self {
            detected: row.0,
            installed: row.1,
            detail: row.2,
            packages: row.3,
        }
    }
}

impl From<NvidiaStatusRow> for NvidiaStatus {
    fn from(row: NvidiaStatusRow) -> Self {
        Self {
            supported: row.0,
            installed: row.1,
            reboot_required: row.2,
            gpu: row.3,
            secure_boot: row.4,
            state: row.5,
            detail: row.6,
            recovery_snapshot: row.7,
        }
    }
}

/// A repository's signing key discovered by AddRepo/TrustRepoKey that isn't
/// trusted yet — the UI shows `user_id`/`fingerprint` and lets the user
/// approve importing it via `SoftwareClient::trust_repo_key`. This is
/// trust-on-first-use, the same level of verification a human would give by
/// approving the equivalent pacman/zypper/dnf terminal prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryKeyInfo {
    pub transaction_id: u32,
    pub repo: String,
    pub key_id: String,
    pub fingerprint: String,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftwareEvent {
    Progress(SoftwareTransactionProgress),
    Finished(SoftwareTransactionFinished),
    UpdatesAvailable(u32),
    KeyPending(RepositoryKeyInfo),
    PackageProgress(SoftwarePackageProgress),
    ConsoleLine(SoftwareConsoleLine),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftwareClientError {
    Unavailable(String),
    ServiceOwnerChanged,
    TransactionTimedOut,
}

impl std::fmt::Display for SoftwareClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServiceOwnerChanged => f.write_str(&gettextrs::gettext(
                "O serviço de software foi interrompido ou reiniciado. O resultado da operação não foi confirmado; verifique o estado do sistema antes de tentar novamente.",
            )),
            Self::TransactionTimedOut => f.write_str(&gettextrs::gettext(
                "O prazo de acompanhamento terminou. A operação pode continuar em execução; verifique o estado do sistema antes de tentar novamente.",
            )),
            Self::Unavailable(detail) => write!(
                f,
                "{}",
                gettextrs::gettext("interface de software indisponível: {detail}")
                    .replace("{detail}", detail)
            ),
        }
    }
}

impl std::error::Error for SoftwareClientError {}

#[async_trait]
pub trait SoftwareClient: Send + Sync {
    async fn package_manager_name(&self) -> Result<String, SoftwareClientError>;
    async fn search(&self, query: &str) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn search_native(&self, query: &str) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn package_details(
        &self,
        origin: &str,
        id: &str,
    ) -> Result<PackageDetails, SoftwareClientError>;
    async fn list_updates(&self) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn list_native_updates(&self) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn update_status(&self) -> Result<UpdateStatus, SoftwareClientError>;
    async fn request_update_check(&self) -> Result<UpdateStatus, SoftwareClientError>;
    async fn list_installed(&self) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn list_native_installed(&self) -> Result<Vec<PackageRef>, SoftwareClientError>;
    async fn list_repos(&self) -> Result<Vec<RepositoryRef>, SoftwareClientError>;
    async fn install(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError>;
    async fn remove(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError>;
    async fn update_all(&self) -> Result<u32, SoftwareClientError>;
    async fn update_all_native(&self) -> Result<u32, SoftwareClientError>;
    async fn update_package(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError>;
    async fn set_repo_enabled(&self, repo: &str, enabled: bool) -> Result<(), SoftwareClientError>;
    async fn add_repo(&self, name: &str, url: &str) -> Result<u32, SoftwareClientError>;
    async fn trust_repo_key(&self, repo: &str, key_id: &str) -> Result<u32, SoftwareClientError>;
    async fn clear_cache(&self) -> Result<u32, SoftwareClientError>;
    async fn clear_native_cache(&self) -> Result<u32, SoftwareClientError>;
    async fn nvidia_status(&self) -> Result<NvidiaStatus, SoftwareClientError>;
    /// Public recovery preflight and reference; requires nvidia-recovery-v1.
    async fn nvidia_recovery(&self) -> Result<NvidiaRecovery, SoftwareClientError> {
        Err(SoftwareClientError::Unavailable(
            "NVIDIA recovery is not supported by this client".into(),
        ))
    }
    /// Optional qualified NVIDIA installation. Require the `nvidia-official-v1`
    /// metadata capability and explicit user confirmation; authorization occurs
    /// only for installation. Earlier daemons return NotSupported. The wire
    /// signature is unchanged. Cancellation must not start a transaction.
    async fn install_nvidia(&self, confirmed: bool) -> Result<u32, SoftwareClientError>;
    async fn check_nvidia(&self) -> Result<(bool, String), SoftwareClientError>;
    async fn non_free_firmware_status(&self) -> Result<NonFreeFirmwareStatus, SoftwareClientError>;
    /// Legacy v1 endpoint: the daemon returns NotSupported without installing firmware.
    async fn install_non_free_firmware(&self, confirmed: bool) -> Result<u32, SoftwareClientError>;
}

#[zbus::proxy(
    interface = "org.lyraos.Vega1.Software",
    default_service = "org.lyraos.Vega1",
    default_path = "/org/lyraos/Vega1"
)]
trait Software {
    async fn package_manager_name(&self) -> zbus::Result<String>;
    async fn search(&self, query: &str) -> zbus::Result<Vec<PackageRefRow>>;
    async fn search_native(&self, query: &str) -> zbus::Result<Vec<PackageRefRow>>;
    async fn get_package_details(&self, origin: &str, id: &str) -> zbus::Result<PackageDetailsRow>;
    async fn list_updates(&self) -> zbus::Result<Vec<PackageRefRow>>;
    async fn list_native_updates(&self) -> zbus::Result<Vec<PackageRefRow>>;
    async fn get_update_status(&self) -> zbus::Result<UpdateStatusRow>;
    async fn request_update_check(&self) -> zbus::Result<UpdateStatusRow>;
    async fn list_installed(&self) -> zbus::Result<Vec<PackageRefRow>>;
    async fn list_native_installed(&self) -> zbus::Result<Vec<PackageRefRow>>;
    async fn list_repos(&self) -> zbus::Result<Vec<RepositoryRefRow>>;
    async fn install(&self, origin: &str, id: &str) -> zbus::Result<u32>;
    async fn remove(&self, origin: &str, id: &str) -> zbus::Result<u32>;
    async fn update_all(&self) -> zbus::Result<u32>;
    async fn update_all_native(&self) -> zbus::Result<u32>;
    async fn update_package(&self, origin: &str, id: &str) -> zbus::Result<u32>;
    async fn set_repo_enabled(&self, repo: &str, enabled: bool) -> zbus::Result<()>;
    async fn add_repo(&self, name: &str, url: &str) -> zbus::Result<u32>;
    async fn trust_repo_key(&self, repo: &str, key_id: &str) -> zbus::Result<u32>;
    async fn clear_cache(&self) -> zbus::Result<u32>;
    async fn clear_native_cache(&self) -> zbus::Result<u32>;
    async fn nvidia_status(&self) -> zbus::Result<NvidiaStatusRow>;
    async fn nvidia_recovery(&self) -> zbus::Result<NvidiaRecoveryRow>;
    async fn install_nvidia(&self, confirmed: bool) -> zbus::Result<u32>;
    async fn check_nvidia(&self) -> zbus::Result<(bool, String)>;
    async fn non_free_firmware_status(&self) -> zbus::Result<NonFreeFirmwareStatusRow>;
    async fn install_non_free_firmware(&self, confirmed: bool) -> zbus::Result<u32>;

    #[zbus(signal)]
    async fn transaction_progress(
        &self,
        transaction_id: u32,
        percent: u32,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transaction_finished(
        &self,
        transaction_id: u32,
        success: bool,
        message: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn package_progress(
        &self,
        transaction_id: u32,
        package: &str,
        phase: &str,
        percent: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transaction_console_line(
        &self,
        transaction_id: u32,
        source: &str,
        line: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn updates_available(&self, count: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_state_changed(&self, status: UpdateStatusRow) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn repo_key_pending(
        &self,
        transaction_id: u32,
        repo: &str,
        key_id: &str,
        fingerprint: &str,
        user_id: &str,
    ) -> zbus::Result<()>;
}

/// A software session is permanently bound to one unique bus owner. Create a
/// new client explicitly after a daemon restart; pending mutations are never replayed.
pub struct ZbusSoftwareClient {
    connection: zbus::Connection,
    owner: async_lock::OnceCell<OwnedUniqueName>,
}

impl ZbusSoftwareClient {
    pub async fn connect() -> Result<Self, SoftwareClientError> {
        let connection = zbus::Connection::system()
            .await
            .map_err(SoftwareClientError::unavailable)?;
        Ok(Self::from_connection(connection))
    }

    pub fn from_connection(connection: zbus::Connection) -> Self {
        Self {
            connection,
            owner: async_lock::OnceCell::new(),
        }
    }

    async fn owner(&self) -> Result<&OwnedUniqueName, SoftwareClientError> {
        self.owner
            .get_or_try_init(|| async {
                let bus = zbus::fdo::DBusProxy::new(&self.connection)
                    .await
                    .map_err(SoftwareClientError::unavailable)?;
                let name = SERVICE.try_into().expect("static service name");
                match bus.get_name_owner(name).await {
                    Ok(owner) => Ok(owner),
                    Err(zbus::fdo::Error::NameHasNoOwner(_)) => {
                        bus.start_service_by_name(
                            SERVICE.try_into().expect("static service name"),
                            0,
                        )
                        .await
                        .map_err(SoftwareClientError::unavailable)?;
                        bus.get_name_owner(SERVICE.try_into().expect("static service name"))
                            .await
                            .map_err(SoftwareClientError::unavailable)
                    }
                    Err(error) => Err(SoftwareClientError::unavailable(error)),
                }
            })
            .await
    }

    async fn proxy(&self) -> Result<SoftwareProxy<'_>, SoftwareClientError> {
        let owner = self.owner().await?;
        self.check_owner(owner).await?;
        SoftwareProxy::builder(&self.connection)
            .destination(owner.clone())
            .map_err(SoftwareClientError::unavailable)?
            .build()
            .await
            .map_err(SoftwareClientError::unavailable)
    }

    async fn check_owner(&self, expected: &OwnedUniqueName) -> Result<(), SoftwareClientError> {
        let bus = zbus::fdo::DBusProxy::new(&self.connection)
            .await
            .map_err(SoftwareClientError::unavailable)?;
        match bus
            .get_name_owner(SERVICE.try_into().expect("static service name"))
            .await
        {
            Ok(owner) if owner == *expected => Ok(()),
            Ok(_) | Err(zbus::fdo::Error::NameHasNoOwner(_)) => {
                Err(SoftwareClientError::ServiceOwnerChanged)
            }
            Err(error) => Err(SoftwareClientError::unavailable(error)),
        }
    }

    pub async fn subscribe(&self) -> Result<SoftwareEventStream, SoftwareClientError> {
        let bus = zbus::fdo::DBusProxy::new(&self.connection)
            .await
            .map_err(SoftwareClientError::unavailable)?;
        // Install this match before resolving/activating the owner or subscribing
        // to its signals, then recheck to cover replacement during setup.
        let owner_changes = bus
            .receive_name_owner_changed_with_args(&[(0, SERVICE)])
            .await
            .map_err(SoftwareClientError::unavailable)?;
        let proxy = self.proxy().await?;
        let owner = self.owner().await?.clone();
        let stream = SoftwareEventStream {
            owner_changes,
            owner: owner.clone(),
            terminal_error: None,
            transaction_deadline: None,
            progress: proxy
                .receive_transaction_progress()
                .await
                .map_err(SoftwareClientError::unavailable)?,
            finished: proxy
                .receive_transaction_finished()
                .await
                .map_err(SoftwareClientError::unavailable)?,
            package_progress: proxy
                .receive_package_progress()
                .await
                .map_err(SoftwareClientError::unavailable)?,
            console: proxy
                .receive_transaction_console_line()
                .await
                .map_err(SoftwareClientError::unavailable)?,
            updates: proxy
                .receive_updates_available()
                .await
                .map_err(SoftwareClientError::unavailable)?,
            key_pending: proxy
                .receive_repo_key_pending()
                .await
                .map_err(SoftwareClientError::unavailable)?,
        };
        self.check_owner(&owner).await?;
        Ok(stream)
    }
}

pub struct SoftwareEventStream {
    owner_changes: zbus::fdo::NameOwnerChangedStream,
    owner: OwnedUniqueName,
    terminal_error: Option<SoftwareClientError>,
    transaction_deadline: Option<(u32, Instant)>,
    progress: TransactionProgressStream,
    finished: TransactionFinishedStream,
    package_progress: PackageProgressStream,
    console: TransactionConsoleLineStream,
    updates: UpdatesAvailableStream,
    key_pending: RepoKeyPendingStream,
}

impl SoftwareEventStream {
    /// Observe this owner's events, ending permanently on owner/bus loss.
    pub async fn next(&mut self) -> Result<SoftwareEvent, SoftwareClientError> {
        if let Some(error) = &self.terminal_error {
            return Err(error.clone());
        }
        let result = self.next_event().await;
        if let Err(error) = &result {
            self.terminal_error = Some(error.clone());
        }
        result
    }

    /// Bound the whole transaction wait, including unrelated signals and progress.
    /// Repeated calls do not extend the deadline. A Finished event permits the next
    /// transaction in a queue; an error invalidates the stream permanently.
    pub async fn next_transaction(
        &mut self,
        transaction_id: u32,
    ) -> Result<SoftwareEvent, SoftwareClientError> {
        self.next_transaction_with_timeout(transaction_id, SOFTWARE_TRANSACTION_TIMEOUT)
            .await
    }

    async fn next_transaction_with_timeout(
        &mut self,
        transaction_id: u32,
        timeout: Duration,
    ) -> Result<SoftwareEvent, SoftwareClientError> {
        if let Some(error) = &self.terminal_error {
            return Err(error.clone());
        }
        let deadline = match self.transaction_deadline {
            Some((id, deadline)) if id == transaction_id => deadline,
            _ => {
                let deadline = Instant::now() + timeout;
                self.transaction_deadline = Some((transaction_id, deadline));
                deadline
            }
        };
        let result = futures_lite::future::or(
            async {
                async_io::Timer::at(deadline).await;
                Err(SoftwareClientError::TransactionTimedOut)
            },
            self.next(),
        )
        .await;
        if let Ok(SoftwareEvent::Finished(event)) = &result
            && event.transaction_id == transaction_id
        {
            self.transaction_deadline = None;
        }
        if let Err(error) = &result {
            self.terminal_error = Some(error.clone());
        }
        result
    }

    async fn next_event(&mut self) -> Result<SoftwareEvent, SoftwareClientError> {
        loop {
            futures_util::select! {
                change = self.owner_changes.next().fuse() => {
                    let change = change.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = change.args().map_err(SoftwareClientError::unavailable)?;
                    if args.old_owner.as_ref().is_some_and(|owner| owner.as_str() == self.owner.as_str()) {
                        return Err(SoftwareClientError::ServiceOwnerChanged);
                    }
                    continue;
                },
                signal = self.progress.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::Progress(SoftwareTransactionProgress {
                        transaction_id: args.transaction_id,
                        percent: args.percent,
                        message: args.message.to_owned(),
                    }))
                },
                signal = self.finished.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::Finished(SoftwareTransactionFinished {
                        transaction_id: args.transaction_id,
                        success: args.success,
                        message: args.message.to_owned(),
                    }))
                },
                signal = self.package_progress.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::PackageProgress(SoftwarePackageProgress {
                        transaction_id: args.transaction_id,
                        package: args.package.to_owned(),
                        phase: args.phase.to_owned(),
                        percent: args.percent,
                    }))
                },
                signal = self.console.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::ConsoleLine(SoftwareConsoleLine {
                        transaction_id: args.transaction_id,
                        source: args.source.to_owned(),
                        line: args.line.to_owned(),
                    }))
                },
                signal = self.updates.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::UpdatesAvailable(args.count))
                },
                signal = self.key_pending.next().fuse() => {
                    let signal = signal.ok_or_else(SoftwareClientError::stream_ended)?;
                    let args = signal.args().map_err(SoftwareClientError::unavailable)?;
                    return Ok(SoftwareEvent::KeyPending(RepositoryKeyInfo {
                        transaction_id: args.transaction_id,
                        repo: args.repo.to_owned(),
                        key_id: args.key_id.to_owned(),
                        fingerprint: args.fingerprint.to_owned(),
                        user_id: args.user_id.to_owned(),
                    }))
                },
            }
        }
    }
}

impl SoftwareClientError {
    fn unavailable(error: impl std::fmt::Display) -> Self {
        Self::Unavailable(error.to_string())
    }

    fn stream_ended() -> Self {
        Self::Unavailable("stream de sinais encerrado".into())
    }
}

macro_rules! proxy_call {
    ($self:ident, $method:ident ( $($arg:expr),* $(,)? )) => {
        $self.proxy().await?.$method($($arg),*).await.map_err(SoftwareClientError::unavailable)
    };
}

#[async_trait]
impl SoftwareClient for ZbusSoftwareClient {
    async fn package_manager_name(&self) -> Result<String, SoftwareClientError> {
        proxy_call!(self, package_manager_name())
    }

    async fn search(&self, query: &str) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, search(query)).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn search_native(&self, query: &str) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, search_native(query))
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn package_details(
        &self,
        origin: &str,
        id: &str,
    ) -> Result<PackageDetails, SoftwareClientError> {
        proxy_call!(self, get_package_details(origin, id)).map(Into::into)
    }

    async fn list_updates(&self) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, list_updates()).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn list_native_updates(&self) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, list_native_updates())
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn update_status(&self) -> Result<UpdateStatus, SoftwareClientError> {
        proxy_call!(self, get_update_status()).map(Into::into)
    }

    async fn request_update_check(&self) -> Result<UpdateStatus, SoftwareClientError> {
        proxy_call!(self, request_update_check()).map(Into::into)
    }

    async fn list_installed(&self) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, list_installed()).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn list_native_installed(&self) -> Result<Vec<PackageRef>, SoftwareClientError> {
        proxy_call!(self, list_native_installed())
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn list_repos(&self) -> Result<Vec<RepositoryRef>, SoftwareClientError> {
        proxy_call!(self, list_repos()).map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn install(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, install(origin, id))
    }

    async fn remove(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, remove(origin, id))
    }

    async fn update_all(&self) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, update_all())
    }

    async fn update_all_native(&self) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, update_all_native())
    }

    async fn update_package(&self, origin: &str, id: &str) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, update_package(origin, id))
    }

    async fn set_repo_enabled(&self, repo: &str, enabled: bool) -> Result<(), SoftwareClientError> {
        proxy_call!(self, set_repo_enabled(repo, enabled))
    }

    async fn add_repo(&self, name: &str, url: &str) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, add_repo(name, url))
    }

    async fn trust_repo_key(&self, repo: &str, key_id: &str) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, trust_repo_key(repo, key_id))
    }

    async fn clear_cache(&self) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, clear_cache())
    }

    async fn clear_native_cache(&self) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, clear_native_cache())
    }

    async fn nvidia_status(&self) -> Result<NvidiaStatus, SoftwareClientError> {
        proxy_call!(self, nvidia_status()).map(Into::into)
    }

    async fn nvidia_recovery(&self) -> Result<NvidiaRecovery, SoftwareClientError> {
        proxy_call!(self, nvidia_recovery()).map(Into::into)
    }

    async fn install_nvidia(&self, confirmed: bool) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, install_nvidia(confirmed))
    }

    async fn check_nvidia(&self) -> Result<(bool, String), SoftwareClientError> {
        proxy_call!(self, check_nvidia())
    }

    async fn non_free_firmware_status(&self) -> Result<NonFreeFirmwareStatus, SoftwareClientError> {
        proxy_call!(self, non_free_firmware_status()).map(Into::into)
    }

    async fn install_non_free_firmware(&self, confirmed: bool) -> Result<u32, SoftwareClientError> {
        proxy_call!(self, install_non_free_firmware(confirmed))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::SoftwareClient;

    const SOFTWARE_XML: &str = include_str!("../dbus/org.lyraos.Vega1.Software.xml");
    const INTERFACE: &str = "org.lyraos.Vega1.Software";

    fn members(tag: &str) -> BTreeMap<String, Vec<(String, String)>> {
        let node_start = SOFTWARE_XML.find("<node").expect("nó de introspecção");
        let document =
            roxmltree::Document::parse(&SOFTWARE_XML[node_start..]).expect("Software XML válido");
        let interface = document
            .descendants()
            .find(|node| {
                node.has_tag_name("interface") && node.attribute("name") == Some(INTERFACE)
            })
            .expect("interface Software presente");

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
    fn software_xml_methods_match_the_typed_proxy() {
        let package_rows = "a(ssssbss)";
        let expected = BTreeMap::from([
            (
                "AddRepo".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "u")]),
            ),
            (
                "TrustRepoKey".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "u")]),
            ),
            ("ClearCache".into(), args(&[("out", "u")])),
            ("ClearNativeCache".into(), args(&[("out", "u")])),
            ("CheckNvidia".into(), args(&[("out", "b"), ("out", "s")])),
            (
                "GetPackageDetails".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "(ssssbssssasasss)")]),
            ),
            ("GetUpdateStatus".into(), args(&[("out", "(ssuuuubs)")])),
            ("RequestUpdateCheck".into(), args(&[("out", "(ssuuuubs)")])),
            (
                "Install".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "u")]),
            ),
            ("InstallNvidia".into(), args(&[("in", "b"), ("out", "u")])),
            (
                "InstallNonFreeFirmware".into(),
                args(&[("in", "b"), ("out", "u")]),
            ),
            ("ListInstalled".into(), args(&[("out", package_rows)])),
            ("ListNativeInstalled".into(), args(&[("out", package_rows)])),
            ("ListNativeUpdates".into(), args(&[("out", package_rows)])),
            ("ListRepos".into(), args(&[("out", "a(sb)")])),
            ("ListUpdates".into(), args(&[("out", package_rows)])),
            ("PackageManagerName".into(), args(&[("out", "s")])),
            ("NvidiaStatus".into(), args(&[("out", "(bbbssssu)")])),
            ("NvidiaRecovery".into(), args(&[("out", "(bssss)")])),
            ("NonFreeFirmwareStatus".into(), args(&[("out", "(bbsas)")])),
            (
                "Remove".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "u")]),
            ),
            ("Search".into(), args(&[("in", "s"), ("out", package_rows)])),
            (
                "SearchNative".into(),
                args(&[("in", "s"), ("out", package_rows)]),
            ),
            ("SetRepoEnabled".into(), args(&[("in", "s"), ("in", "b")])),
            ("UpdateAll".into(), args(&[("out", "u")])),
            ("UpdateAllNative".into(), args(&[("out", "u")])),
            (
                "UpdatePackage".into(),
                args(&[("in", "s"), ("in", "s"), ("out", "u")]),
            ),
        ]);
        assert_eq!(members("method"), expected);
    }

    #[test]
    fn software_xml_signals_match_the_typed_proxy() {
        let expected = BTreeMap::from([
            (
                "TransactionFinished".into(),
                args(&[("out", "u"), ("out", "b"), ("out", "s")]),
            ),
            (
                "TransactionProgress".into(),
                args(&[("out", "u"), ("out", "u"), ("out", "s")]),
            ),
            (
                "TransactionConsoleLine".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s")]),
            ),
            ("UpdatesAvailable".into(), args(&[("out", "u")])),
            ("UpdateStateChanged".into(), args(&[("out", "(ssuuuubs)")])),
            (
                "RepoKeyPending".into(),
                args(&[
                    ("out", "u"),
                    ("out", "s"),
                    ("out", "s"),
                    ("out", "s"),
                    ("out", "s"),
                ]),
            ),
            (
                "PackageProgress".into(),
                args(&[("out", "u"), ("out", "s"), ("out", "s"), ("out", "u")]),
            ),
        ]);
        assert_eq!(members("signal"), expected);
    }

    #[test]
    fn package_rows_are_converted_without_losing_origin() {
        let package = super::PackageRef::from((
            "flathub".into(),
            "org.example.App".into(),
            "Example".into(),
            "Description".into(),
            true,
            "org.example.App".into(),
            "Flathub".into(),
        ));
        assert_eq!(package.origin, "flathub");
        assert_eq!(package.id, "org.example.App");
        assert!(package.installed);
    }

    #[test]
    #[ignore = "requer vegad instalado e acesso ao system bus"]
    fn real_daemon_exposes_the_read_only_software_contract() {
        futures_lite::future::block_on(async {
            let client = super::ZbusSoftwareClient::connect().await.unwrap();
            assert!(!client.package_manager_name().await.unwrap().is_empty());
            client.list_repos().await.unwrap();
            let subscriptions = client.subscribe().await.unwrap();
            drop(subscriptions);
        });
    }
}

#[cfg(test)]
#[path = "software_owner_loss_tests.rs"]
mod owner_loss_tests;
