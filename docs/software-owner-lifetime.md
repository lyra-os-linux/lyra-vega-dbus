# Software transactions and daemon lifetime

`org.lyraos.Vega1` is a well-known bus name; its owner can change after an idle
exit, crash or restart. Transaction IDs are process-local counters. An ID alone
does not identify an operation across daemon instances.

Each `ZbusSoftwareClient` now binds permanently to the unique owner of that
name when first used. Initial use can activate the D-Bus service, as a normal
software request does. Calls and signal subscriptions target that unique name.
Before a call, the client verifies that the public name still has that owner.
It never redirects or replays a pending mutation to a replacement daemon.

Subscribe using the same client **before** starting the operation. The stream
registers `NameOwnerChanged` before resolving its owner and rechecks the owner
after all signal subscriptions are installed. Losing ownership, including
release while the old connection remains alive, ends the stream with
`SoftwareClientError::ServiceOwnerChanged`. Bus/stream failure also ends it.
All subsequent reads return the terminal error. Create a fresh client and
subscription explicitly for a new operation or for reconnecting passive UI.

Use `events.next_transaction(id)` while monitoring a transaction. It retains
an absolute two-hour deadline across repeated calls and unrelated/progress
events. A matching completion releases the deadline for the next queued
transaction. Expiry returns `TransactionTimedOut` and terminates the stream.
The timer starts at the first monitoring call; it does not cover the initial
method request or a user confirmation dialog. `next()` remains available for
long-lived passive update listeners without a transaction deadline.

Loss or timeout means **the result was not confirmed**, not that the operation
was undone or that installed packages are unchanged. It must not trigger an
automatic retry, cancellation, rollback or continuation of an installation
queue. Check the system state before starting another operation. There is no
persistent result query in this version of the daemon protocol.

The wire XML and daemon methods are unchanged. Existing consumers can keep
their explicit release tag; the updated GTK consumer uses an immutable Git
revision until the next packaged release. Manifest/lock consistency is checked
for both forms. This is a source integration, not an RPM/ISO publication.

## Qualification

`cargo test --locked` launches disposable session `dbus-daemon` processes and
real zbus connections. Fake Software methods only count calls and emit signals;
they never touch the system bus, RPM, package managers or installed services.

The scenarios cover service disappearance without a final signal, replacement
reusing the old transaction ID, replacement between subscription and method,
bus disconnection, completion before the method reply, sequential transactions
and an absolute deadline even with progress. Old clients cannot send mutations
to the replacement, and expired monitoring does not cancel the fake daemon.
The four existing installed-daemon tests remain explicitly ignored by default.

GTK must adopt the revised dependency, use `next_transaction`, finish the UI's
busy state on errors, stop a queue after an unconfirmed result, and reconnect
passive listeners independently. The companion Vega change supplies messages
in pt-BR, en-US and es-ES. Private-bus tests do not qualify real RPM operations
or an entire graphical login session.
