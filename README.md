# lyra-vega-dbus

Cliente D-Bus tipado do contrato `org.lyraos.Vega1`, compartilhado entre os
frontends do [Vega](https://github.com/lyra-os-linux/vega)
([`vega-gtk`](https://github.com/lyra-os-linux/vega)) e
[`vega-web`](https://github.com/lyra-os-linux/vega-web).

O contrato em si — a introspecção XML de cada interface — está em `dbus/`,
implementado pelo daemon [`vegad`](https://github.com/lyra-os-linux/vegad).
Este crate (`src/`) é o cliente Rust tipado desse contrato; a implementação
Go do lado do daemon deve ser mantida em sincronia com `dbus/`.

## Uso

```toml
[dependencies]
lyra-vega-dbus = { git = "https://github.com/lyra-os-linux/lyra-vega-dbus", tag = "v5.1.22" }
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
