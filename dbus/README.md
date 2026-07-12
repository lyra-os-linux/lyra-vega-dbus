# Contrato D-Bus do Vega

Estes arquivos são a introspecção D-Bus (XML) das interfaces expostas pelo
`vegad` em `org.lyraos.Vega1` / `/org/lyraos/Vega1`, uma por módulo. São a
fonte de verdade do contrato entre `vega` (UI) e `vegad` (daemon) — a
implementação Go vive em `vegad/internal/dbusserver/*.go` e deve ser mantida
em sincronia com estes arquivos.

Cada método aqui mapeia a uma action polkit granular em
`packaging/vegad/org.lyraos.vega.policy`.
