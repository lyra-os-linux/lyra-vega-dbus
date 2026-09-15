# Required checks for main

The `main` branch requires four GitHub Actions checks (app ID `15368`):

- `rust-contracts`: Rust tests, Python/XML contracts, formatting and Clippy.
- `consumers (vega)`: explicit manifest/lockfile contract revision for Vega GTK.
- `consumers (vega-web)`: the existing shared consumer pin contract.
- `daemon-contract`: the running vegad against the canonical D-Bus interface.

Branches must be up to date before merge, and the policy applies to repository
administrators. Additional review count remains zero. Force pushes and branch
deletion remain disabled. The existing web consumer check is preserved without
adding that frontend to the GNOME release scope. Consumer pin checks do not
claim to compile or exercise the consumer applications.

The daemon job now installs `polkit`: the current vegad integration harness
requires `pkcheck` to verify denial of protected backup reads on a private bus.
The private bus has no Polkit authority. Real system-manager/Polkit and desktop
behavior retain their separate qualification gates. No D-Bus API changes are
introduced by this issue.

Issue #1 is qualified with a temporary failure step limited to the test PR
branch. The step must be removed before the final full CI and squash merge.
Pending/failing merge attempts and final successful checks are recorded on
the issue.

The effective policy is stored in GitHub repository settings. Administrators
can still deliberately edit protection settings; routine merges must satisfy
the checks. Preserve exact matrix check names and the GitHub Actions app
binding when changing workflows.

References:

- https://github.com/lyra-os-linux/lyra-vega-dbus/issues/1
- https://docs.github.com/en/rest/branches/branch-protection#update-branch-protection
