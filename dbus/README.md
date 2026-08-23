# Contrato D-Bus do Vega

Estes arquivos são a introspecção D-Bus (XML) das interfaces expostas pelo
`vegad` em `org.lyraos.Vega1` / `/org/lyraos/Vega1`, uma por módulo. São a
fonte de verdade do contrato entre os frontends (`vega-gtk`, `vega-cli`,
`vega-web`) e o daemon [`vegad`](https://github.com/lyra-os-linux/vegad) — a
implementação Go vive em `internal/dbusserver/*.go` nesse repo e deve ser
mantida em sincronia com estes arquivos. O crate `lyra-vega-dbus` (irmão
deste diretório) é o cliente tipado desse contrato, usado pelos frontends
Rust.

Cada método aqui mapeia a uma action polkit granular em
`packaging/vegad/org.lyraos.vega.policy` no repo do `vegad`.
