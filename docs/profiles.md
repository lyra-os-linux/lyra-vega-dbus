# Perfis do sistema (`org.lyraos.Vega1.Profiles`)

Anunciada pela capability `system-profiles-v1`. Um perfil é um conjunto
**fixo e conhecido** de ajustes do sistema que o vegad aplica e desfaz como
uma unidade. O vegad nunca aceita chaves ou valores livres, só os ids abaixo.
Origem: [vegad#122](https://github.com/lyra-os-linux/vegad/issues/122) e
[vegad#123](https://github.com/lyra-os-linux/vegad/issues/123).

## Perfis

| id | Ligar | Desligar | Efeito |
| --- | --- | --- | --- |
| `sap-abap-trial` | grava `/etc/sysctl.d/90-lyra-sap-abap-trial.conf` com `vm.max_map_count`, `fs.file-max` e `fs.aio-max-nr` para o SAP ABAP Platform Trial e aplica com `sysctl` | apaga o arquivo e restaura os valores padrão | imediato |
| `audio-lowlatency` | instala o `pipewire-jack`, grava `/etc/security/limits.d/90-lyra-audio.conf` (`@audio - rtprio 95`, `@audio - memlock unlimited`) e põe o usuário que chamou no grupo `audio`, se faltar | apaga o arquivo; remove o `pipewire-jack` e tira o usuário do grupo só se foi o perfil que instalou ou acrescentou | no próximo login |
| `audio-lowlatency-kernel` | acrescenta `preempt=full threadirqs` à linha de comando do kernel | restaura a linha de comando exata de antes | no próximo boot |

O perfil `audio-lowlatency-kernel` é separado de `audio-lowlatency` porque
mexe no boot do sistema inteiro: o frontend o oferece como opção, com aviso.
`memlock unlimited` vale só para quem está no grupo `audio`.

Ids desconhecidos são recusados com `NotSupported`.

## `ListSystemProfiles() -> a(sbb)`

Cada linha traz `(id, enabled, restartPending)`, na ordem acima:

- `enabled`: o perfil está aplicado na configuração;
- `restartPending`: a última mudança (ligar ou desligar) só vale por completo
  depois de reiniciar (`audio-lowlatency-kernel`) ou de entrar de novo na
  sessão (`audio-lowlatency`). É sempre falso para `sap-abap-trial`.

Sem polkit: é uma leitura, usada pelos cartões dos frontends e pelo
diagnóstico, que mostra os perfis ligados.

## `SetSystemProfile(id, enabled) -> jobId`

Liga ou desliga um perfil como um job:

1. o vegad cria um snapshot do Snapper antes, quando o Snapper está
   configurado;
2. aplica os passos do perfil;
3. se um passo falhar, desfaz os passos já feitos: o perfil fica todo ligado
   ou todo desligado.

Ligar um perfil já ligado ou desligar um já desligado não muda nada e termina
com `ok`. Cada mudança vai para o log com o uid de quem chamou.

Sinais:

- `ProfileProgress(jobId, id, percent, message)`;
- `ProfileFinished(jobId, id, result, detail)`, com `result` igual a `ok` ou
  `failed`. Em `failed`, os passos já feitos foram desfeitos e `detail` traz o
  motivo.

Erros (`org.lyraos.Vega1.Error.*`):

- `Busy`: outro job de perfil, um diagnóstico, uma limpeza ou uma transação
  de software, snapshot ou backup está em andamento;
- `NotSupported`: id desconhecido.

Polkit: `org.lyraos.vega.profiles.manage` (`auth_admin`). Sem autorização,
nada muda.
