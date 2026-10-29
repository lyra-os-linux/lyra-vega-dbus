# add-security-review: parte do contrato

Proposta-mãe, requisitos e design:
[lyra-ecosystem/openspec/changes/add-security-review](https://github.com/lyra-os-linux/lyra-ecosystem/tree/main/openspec/changes/add-security-review),
validada pelo mantenedor em 29/10/2026.

Este repositório entrega a interface que os requisitos das capacidades
`network-exposure` e `security-diagnostics` usam:

- `org.lyraos.Vega1.Firewall.ListListening() -> a(ssqsssss)`, com a capability
  `network-exposure-v1`;
- o teste `security` e os remédios `enable-firewall`, `close-port` e
  `apply-security-patches` no `Diagnostics`, com a capability
  `diagnostics-v3`.

As duas mudanças são aditivas: nenhum método ou assinatura existente muda.
