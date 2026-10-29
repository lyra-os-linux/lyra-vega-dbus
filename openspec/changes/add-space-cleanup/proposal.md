# add-space-cleanup: parte do contrato

Proposta-mãe, requisitos e design:
[lyra-ecosystem/openspec/changes/add-space-cleanup](https://github.com/lyra-os-linux/lyra-ecosystem/tree/main/openspec/changes/add-space-cleanup),
validada pelo mantenedor em 29/10/2026.

Este repositório entrega:

- a interface `org.lyraos.Vega1.Cleanup` (`Scan`, `Clean` e os sinais
  `Progress`, `CategoryFinished` e `Finished`), com a capability
  `space-cleanup-v1`;
- o código de achado `storage.low-space`, documentado em `docs/diagnostics.md`.

Em relação ao design da proposta-mãe, o contrato acrescenta o sinal
`CategoryFinished`, que entrega o "resultado por categoria" do requisito
"Limpar com confirmação e progresso", e usa a ação nova
`org.lyraos.vega.storage.read-admin` (sem senha) no `Scan`, porque a
`logs.read-admin` pede senha.

A mudança é aditiva: nenhum método ou assinatura existente muda.
