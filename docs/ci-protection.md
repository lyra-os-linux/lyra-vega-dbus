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

## Qualification on 2026-09-15

[PR #4](https://github.com/lyra-os-linux/lyra-vega-dbus/pull/4) exercised the
policy with the maintainer's administrator account. A temporary step limited
to that PR branch delayed and deliberately failed `rust-contracts`; it was
removed before the final full CI run. The lasting workflow change only adds
the missing `polkit` test dependency.

| State on the controlled PR | Merge API result |
| --- | --- |
| One consumer passed; three checks running | HTTP 405: `3 of 4 required status checks are in progress.` |
| Both consumers passed; Rust failed; daemon running | HTTP 405: `2 of 4 required status checks have not succeeded: 1 failing.` |

The requests included the exact PR head SHA. Both refusals preserved `main`
at `c41e2abb0f4d21fb554bdbb6d36faffbcac18281`. The
[controlled run](https://github.com/lyra-os-linux/lyra-vega-dbus/actions/runs/34988598649)
records the intentional failure, not a contract regression. All four checks
on the final head remain required for merge. Issue #1 records the final CI
and merge receipt.

## Administration and recovery

The effective policy is stored in GitHub repository settings. Administrators
can still deliberately edit protection settings; routine merges must satisfy
the checks. Preserve exact matrix check names and the GitHub Actions app
binding when changing workflows. No user/team/app bypass is configured in
the inspected protection, and the API returned no additional rules applicable
to main. Review count, force-push, deletion and other existing fields were
preserved; only required checks and administrator enforcement changed.

GitHub accepts successful, neutral or skipped conclusions for required checks.
The current jobs have no job-level skip conditions and the workflow has no
PR path/branch filter. Preserve that coverage when changing CI. If a workflow
or its infrastructure fails, correct it and rerun before merging.

An explicitly approved policy rollback would restore no required checks and
disabled administrator enforcement while retaining the other fields. That
removes the CI gate and must be recorded as a policy change.

References:

- https://github.com/lyra-os-linux/lyra-vega-dbus/issues/1
- https://docs.github.com/en/rest/branches/branch-protection#update-branch-protection
