use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("private dbus-daemon required");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(!address.trim().is_empty(), "private bus did not start");
        Self {
            child,
            address: address.trim().into(),
        }
    }

    async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct FakeSoftware {
    calls: Arc<AtomicU32>,
    finish_before_reply: bool,
}

#[zbus::interface(name = "org.lyraos.Vega1.Software")]
impl FakeSoftware {
    async fn install(
        &self,
        origin: &str,
        id: &str,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> u32 {
        assert_eq!(origin, "official");
        assert_eq!(id, "fixture");
        let id = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.finish_before_reply {
            finish(connection, id).await;
        }
        id
    }
}

async fn daemon(bus: &PrivateBus, early: bool) -> (zbus::Connection, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let connection = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .serve_at(
            "/org/lyraos/Vega1",
            FakeSoftware {
                calls: calls.clone(),
                finish_before_reply: early,
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    connection.request_name(SERVICE).await.unwrap();
    (connection, calls)
}

async fn finish(connection: &zbus::Connection, id: u32) {
    connection
        .emit_signal(
            None::<&str>,
            "/org/lyraos/Vega1",
            "org.lyraos.Vega1.Software",
            "TransactionFinished",
            &(id, true, "fixture completed"),
        )
        .await
        .unwrap();
}

fn run(test: impl std::future::Future<Output = ()>) {
    futures_lite::future::block_on(futures_lite::future::or(test, async {
        async_io::Timer::after(Duration::from_secs(10)).await;
        panic!("private bus scenario exceeded its deadline");
    }));
}

#[test]
fn completion_before_method_reply_and_sequential_transactions_are_preserved() {
    run(async {
        let bus = PrivateBus::start();
        let (_daemon, calls) = daemon(&bus, true).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut events = client.subscribe().await.unwrap();
        for expected in [1, 2] {
            let id = client.install("official", "fixture").await.unwrap();
            assert_eq!(id, expected);
            assert!(matches!(events.next_transaction(id).await.unwrap(),
                SoftwareEvent::Finished(event) if event.transaction_id == id && event.success));
            assert!(events.transaction_deadline.is_none());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn owner_disappears_without_any_transaction_signal() {
    run(async {
        let bus = PrivateBus::start();
        let (service, _) = daemon(&bus, false).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut events = client.subscribe().await.unwrap();
        let id = client.install("official", "fixture").await.unwrap();
        service.close().await.unwrap();
        assert_eq!(
            events.next_transaction(id).await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
        assert_eq!(
            events.next().await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
    });
}

#[test]
fn replacement_reusing_id_cannot_complete_or_receive_old_clients_mutations() {
    run(async {
        let bus = PrivateBus::start();
        let (old, old_calls) = daemon(&bus, false).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut old_events = client.subscribe().await.unwrap();
        let id = client.install("official", "fixture").await.unwrap();
        // Keep the old unique connection alive: losing the public name is enough.
        old.release_name(SERVICE).await.unwrap();
        let (new, new_calls) = daemon(&bus, true).await;
        let fresh = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut new_events = fresh.subscribe().await.unwrap();
        assert_eq!(fresh.install("official", "fixture").await.unwrap(), id);
        assert!(matches!(
            new_events.next_transaction(id).await.unwrap(),
            SoftwareEvent::Finished(_)
        ));
        finish(&new, id).await;
        assert_eq!(
            old_events.next_transaction(id).await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
        assert_eq!(
            client.install("official", "fixture").await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
        assert!(matches!(
            client.subscribe().await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        ));
        assert_eq!(old_calls.load(Ordering::SeqCst), 1);
        assert_eq!(new_calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn replacement_between_subscription_and_method_starts_no_operation() {
    run(async {
        let bus = PrivateBus::start();
        let (old, old_calls) = daemon(&bus, false).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut events = client.subscribe().await.unwrap();
        old.release_name(SERVICE).await.unwrap();
        let (_new, new_calls) = daemon(&bus, false).await;
        assert_eq!(
            client.install("official", "fixture").await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
        assert_eq!(
            events.next().await,
            Err(SoftwareClientError::ServiceOwnerChanged)
        );
        assert_eq!(old_calls.load(Ordering::SeqCst), 0);
        assert_eq!(new_calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn bus_disconnect_terminates_stream_instead_of_waiting_forever() {
    run(async {
        let mut bus = PrivateBus::start();
        let (_service, _) = daemon(&bus, false).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut events = client.subscribe().await.unwrap();
        bus.child.kill().unwrap();
        bus.child.wait().unwrap();
        assert!(events.next().await.is_err());
        assert!(events.next().await.is_err());
    });
}

#[test]
fn deadline_is_absolute_even_with_progress_and_does_not_cancel_daemon() {
    run(async {
        let bus = PrivateBus::start();
        let (service, calls) = daemon(&bus, false).await;
        let client = ZbusSoftwareClient::from_connection(bus.connect().await);
        let mut events = client.subscribe().await.unwrap();
        let id = client.install("official", "fixture").await.unwrap();
        service
            .emit_signal(
                None::<&str>,
                "/org/lyraos/Vega1",
                "org.lyraos.Vega1.Software",
                "TransactionProgress",
                &(id, 10u32, "still running"),
            )
            .await
            .unwrap();
        assert!(matches!(
            events
                .next_transaction_with_timeout(id, Duration::from_millis(200))
                .await
                .unwrap(),
            SoftwareEvent::Progress(_)
        ));
        let deadline = events.transaction_deadline.unwrap().1;
        async_io::Timer::at(deadline + Duration::from_millis(10)).await;
        finish(&service, id).await;
        assert_eq!(
            events.next_transaction(id).await,
            Err(SoftwareClientError::TransactionTimedOut)
        );
        assert_eq!(
            events.next_transaction(id + 1).await,
            Err(SoftwareClientError::TransactionTimedOut)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(service.unique_name().is_some());
    });
}
