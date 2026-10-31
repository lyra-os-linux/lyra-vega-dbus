# lyra-vega-dbus

Cliente D-Bus tipado do contrato `org.lyraos.Vega1`, compartilhado entre os
frontends Rust do Vega: [`vega-gtk`](https://github.com/lyra-os-linux/vega) e
[`vega-web`](https://github.com/lyra-os-linux/vega-web). O
[`vega-cli`](https://github.com/lyra-os-linux/vega-cli) consome só o contrato
XML em `dbus/`, via `busctl`.

O contrato em si — a introspecção XML de cada interface — está em `dbus/`,
implementado pelo daemon [`vegad`](https://github.com/lyra-os-linux/vegad).
Este crate (`src/`) é o cliente Rust tipado desse contrato; a implementação
Go do lado do daemon deve ser mantida em sincronia com `dbus/`.

## Uso

```toml
[dependencies]
lyra-vega-dbus = { git = "https://github.com/lyra-os-linux/lyra-vega-dbus", tag = "v5.2.0" }
```

Veja [a correlação de transações com a instância do daemon](docs/software-owner-lifetime.md)
para acompanhar operações e tratar interrupções sem repetir mutações.

## Desenvolvimento

```sh
cargo test --locked
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings

# Com os checkouts irmãos presentes:
./scripts/check-consumer-pin.sh ../vega vega
./scripts/check-consumer-pin.sh ../vega-web vega-web

# Executa o daemon irmão num barramento privado e valida as interfaces reais:
../vegad/scripts/test-dbus-integration.sh
```

Licenciado sob GPL-3.0.

## NVIDIA (aposentado)

O Vega não gerencia mais drivers NVIDIA (spec `remove-nvidia-integration` do
`lyra-ecosystem`). `InstallNvidia`, `NvidiaStatus`, `NvidiaRecovery`,
`CheckNvidia` e `SwitchNvidiaDriver` continuam no XML do v1, marcados como
`Deprecated`, e o daemon sempre responde `NotSupported`. As capacidades
`nvidia-official-v1` e `nvidia-recovery-v1` não são mais anunciadas, e o crate
não expõe esses métodos a partir da 5.2.0.

## Space cleanup capability

`space-cleanup-v1` announces `org.lyraos.Vega1.Cleanup`: `Scan` measures the
system categories (core dumps, journal, package cache, unused Flatpak
runtimes, old kernels, snapshot cleanup) without deleting anything, and
`Clean` runs them as a job. See [the cleanup contract](docs/cleanup.md).

## Diagnostics capability

`diagnostics-v1` announces `org.lyraos.Vega1.Diagnostics`: health tests run by
vegad (packages, disk health, btrfs scrub, CPU and memory stress, disk
benchmark, logs and platform state), with progress, live telemetry, safeguards
and a versioned JSON report. See [the diagnostics contract](docs/diagnostics.md)
for the job lifecycle, verdicts, errors, Polkit actions and report schema.
