# Diagnóstico de saúde (`org.lyraos.Vega1.Diagnostics`)

Interface aditiva, anunciada pela capability `diagnostics-v1` em
`Metadata.Capabilities`. O `vegad` roda os testes como root e transmite o
progresso. Os frontends (`vega-gtk` agora; `vega-cli` e `vega-web` depois)
consomem tudo pelo mesmo contrato. A especificação completa está em
[vega#165](https://github.com/lyra-os-linux/vega/issues/165).

## Testes

| id | O que faz | Tomada |
| --- | --- | --- |
| `packages` | `rpm -Va` e `zypper verify`: arquivos alterados e dependências quebradas | não |
| `disk-health` | SMART (`smartctl -j`) de cada disco físico e `btrfs device stats` | não |
| `fs-scrub` | `btrfs scrub` somente leitura (`-r`) | não |
| `cpu-stress` | `stress-ng --cpu` com verificação, temperatura e throttling | sim |
| `memory-stress` | `stress-ng --vm` com verificação, até 70% de `MemAvailable` | sim |
| `disk-bench` | `fio`, sequencial, aleatório e latência QD1, até ~5 GiB escritos | sim |
| `logs` | erros por boot, coredumps de 7 dias e unidades que falharam | não |
| `platform` | temperaturas, bateria, firmware, NTP, Secure Boot, kernel *tainted*, SELinux | não |
| `security` | firewall, portas expostas, patches de segurança, SSH, SELinux, Secure Boot, criptografia da raiz, contas sem senha, `sudo` (`diagnostics-v3`) | não |

O teste de GPU roda no frontend, na sessão do usuário, e não passa pelo
contrato. Um teste que não pode rodar aparece em `ListTests` com
`available = false` e um `reason` estável, que o frontend traduz:

| `reason` | Significado |
| --- | --- |
| `missing-tool:<pacote>` | o pacote da ferramenta não está instalado (ex.: `missing-tool:smartmontools`) |
| `not-btrfs` | `fs-scrub` com a raiz fora do btrfs |
| `not-implemented` | o daemon conhece o teste, mas ainda não o executa |

Um `reason` desconhecido deve aparecer como "indisponível". Pedir um teste
indisponível em `Start` não é erro: a etapa termina com `unavailable`.

## Ciclo de um job

1. O frontend assina os sinais (`subscribe`) **antes** de `Start`.
2. `Start(tests, options)` devolve o `jobId`. Os testes rodam na ordem pedida.
3. Durante a execução:
   - `Progress` informa a etapa (`stage` = id do teste) e o percentual;
   - `Sample` traz a telemetria a cada 5 s nos testes de carga;
   - `StageFinished` dá o veredito de cada etapa.
4. `Finished` encerra o job com o resultado agregado.
5. `Report(jobId, redact)` devolve o relatório JSON.

Um frontend aberto no meio de um job chama `Current` para saber o `jobId`, os
testes, a etapa e o percentual. Com isso ele acompanha os sinais e pode
cancelar. `jobId = 0` significa que não há job em andamento.

`Cancel(jobId)` mata o grupo de processos do teste em curso. O job termina com
`Finished(result = "cancelled")` e o relatório parcial é guardado.

## Vereditos e resultados

| Valor | Etapa | Job | Significado |
| --- | --- | --- | --- |
| `ok` | ✓ | ✓ | saudável |
| `attention` | ✓ | ✓ | algo merece atenção, sem falha |
| `fail` | ✓ | ✓ | falha real (erro de verificação, SMART crítico, checksum) |
| `unavailable` | ✓ | | ferramenta ausente ou não se aplica (ex.: scrub fora do btrfs) |
| `cancelled` | ✓ | ✓ | cancelado pelo usuário |
| `aborted` | ✓ | ✓ | uma salvaguarda parou o teste (temperatura, tomada) |

O resultado do job é o pior veredito entre as etapas, desconsiderando
`unavailable`. Frontends devem tolerar valores novos (o crate os mantém em
`DiagnosticVerdict::Other`).

## Erros

| Nome | Quando |
| --- | --- |
| `org.lyraos.Vega1.Error.Busy` | outro diagnóstico, ou uma transação Zypper, de snapshot ou de backup, está em andamento |
| `org.lyraos.Vega1.Error.OnBattery` | teste de carga pedido sem tomada |
| `org.lyraos.Vega1.Error.InsufficientResources` | `MemAvailable` < 2 GiB, ou < 10 GiB livres para o `disk-bench` |
| `org.lyraos.Vega1.Error.NotSupported` | id de teste desconhecido ou opção fora da faixa |

A exclusividade vale nos dois sentidos: enquanto um diagnóstico roda, as
transações de Software, Snapshots e Backup também respondem `Busy`.

## Polkit

| Membro | Ação |
| --- | --- |
| `ListTests`, `Current` | nenhuma (leitura) |
| `Start`, `Cancel` | `org.lyraos.vega.hardware.diagnose` (`auth_admin_keep`) |
| `Report` | `org.lyraos.vega.logs.read-admin` |

## Relatório JSON (schema 1)

Guardado em `/var/lib/vega/diagnostics/<jobId>.json` (0600), com os 10 jobs
mais recentes.

```json
{
  "schema": 1,
  "jobId": 7,
  "started": "2026-10-28T15:52:12-03:00",
  "finished": "2026-10-28T16:01:40-03:00",
  "host": {
    "product": "Nitro AN515-57",
    "bios": "V1.20",
    "kernel": "6.12.0-160100.7-default",
    "os": "Lyra OS 16.1",
    "vegad": "5.2.0"
  },
  "result": "attention",
  "stages": [
    {
      "id": "disk-health",
      "verdict": "ok",
      "summary": "SMART PASSED, 2% usado",
      "durationSeconds": 3,
      "findings": [
        {
          "severity": "info",
          "code": "smart.poh-unreliable",
          "text": "Contador de horas ligado incoerente com o volume gravado",
          "raw": "Power On Hours: 87; Data Units Written: 23,0 TB"
        }
      ],
      "metrics": { "percentUsed": 2, "mediaErrors": 0, "tempC": 34 }
    }
  ]
}
```

- Um achado consertável tem `"remedy": "<id>"` (ver **Reparo**). Os detalhes
  do conserto ficam em `"remedyArgs"`, só para leitura: o vegad usa a cópia
  que ele gravou, nunca a que o frontend enviar.
- `severity` vale `info`, `attention` ou `fail`. `info` agrupa o ruído
  conhecido (bugs de firmware ACPI, avisos comuns de boot e afins).
- `code` é estável. Os frontends traduzem e agrupam por ele, nunca pelo `text`.
- `storage.low-space` (atenção, teste `platform`): a raiz tem menos de 10% ou
  menos de 5 GiB livres. Não tem remédio; com `space-cleanup-v1`, o frontend
  oferece o atalho para a limpeza (ver [cleanup.md](cleanup.md)).
- `metrics` varia por teste. As chaves ausentes não foram medidas.
- Com `redact = true`, saem os números de série, endereços MAC, SSIDs, o
  hostname e o nome das pastas em `/home`.
- Enquanto o job roda, `Report` devolve o relatório parcial, sem `result` nem
  `finished`. O relatório é gravado a cada etapa.
- Um campo novo não muda o `schema`. Remover ou mudar o significado de um campo
  exige `schema: 2`.

## Reparo (`diagnostics-v2`)

`Repair(jobId, remedies)` aplica consertos seguros propostos pelo relatório de
`jobId`. Só existem estes, todos reversíveis pelo snapshot criado antes:

| `remedy` | Origem | O que faz |
| --- | --- | --- |
| `reinstall-packages` | `packages.binary-changed`, `packages.missing` | reinstala os pacotes donos dos arquivos (`zypper install -f`) |
| `verify-dependencies` | `packages.broken-deps` | instala o que o `zypper verify` aponta |
| `restart-units` | `logs.failed-units` | `systemctl reset-failed` e reinício das unidades |
| `enable-ntp` | `platform.ntp-off` | liga a sincronização de hora |
| `enable-firewall` | `security.firewall-off` | `systemctl enable --now firewalld` (`diagnostics-v3`) |
| `close-port` | `security.exposed-port` | tira o serviço ou a porta da zona, permanente e em execução (`diagnostics-v3`) |
| `apply-security-patches` | `security.patches-pending` | `zypper patch --category security`, dentro do snapshot (`diagnostics-v3`) |
| `reconnect` | `network.no-link` | liga o Wi-Fi se preciso e conecta o dispositivo (`diagnostics-v4`, sem snapshot) |
| `renew-dhcp` | `network.no-address` | reaplica a conexão, o que renova o DHCP (`diagnostics-v4`, sem snapshot) |
| `disable-wait-online` | `boot.wait-online` | desabilita o `NetworkManager-wait-online` se nada mais quer o `network-online.target` (`diagnostics-v5`) |
| `disable-service` | `boot.slow-service` | desabilita serviços que o preset da imagem não habilita (`diagnostics-v5`) |
| `temporary-dns` | `network.dns-failed` | DNS público (1.1.1.1 e 9.9.9.9) só na ativação atual; o perfil salvo não muda (`diagnostics-v4`, sem snapshot) |

Nada que mexa em disco, partição ou sistema de arquivos tem conserto
automático: erros de SMART, scrub ou btrfs pedem backup e orientação.

O reparo é um job como o de `Start`, com os mesmos sinais:
1. Etapas `repair:<remedy>`: o veredito `ok` significa que o conserto foi
   aplicado, e `fail`, que não foi.
2. Os testes afetados rodam de novo e dizem se o problema sumiu.

O relatório do reparo leva `"repairOf": <jobId>`.

Erros: `Busy` (outro job ou transação em curso) e `NotSupported` (job
desconhecido, ou remédio que não está no relatório dele).

Polkit: `org.lyraos.vega.hardware.repair` (`auth_admin`, sem retenção: altera
o sistema).

## Ações por achado (`diagnostics-actions-v1`)

- `Remedies(jobId) -> a(sus)`: `(stage, findingIndex, remedy)` de cada achado
  com remédio do relatório (`jobId` 0 = o mais recente). `findingIndex` é a
  posição do achado dentro da etapa. Sem textos, dados brutos nem argumentos;
  leitura pública, sem polkit. Serve para o frontend mostrar **Corrigir** na
  lista de testes sem abrir o relatório.
- `RepairFindings(jobId, findings a(su)) -> repairJobId`: como `Repair`, mas
  só para os achados `(stage, findingIndex)` indicados, cada um com os
  `remedyArgs` que tem no relatório gravado. Fechar uma porta não fecha as
  outras. `NotSupported` para job desconhecido, índice fora do intervalo ou
  achado sem remédio. Polkit `org.lyraos.vega.hardware.repair`.
- `MarkLogsSeen(jobId)`: o teste `logs` dos jobs seguintes ignora erros do
  journal, core dumps e serviços com falha anteriores ao fim da etapa `logs`
  desse job. O corte fica no vegad
  (`/var/lib/vega/diagnostics/logs-seen.json`) e vale para todos os
  frontends; o relatório do `logs` traz a métrica `seenUntil`.
  `NotSupported` se o job não tem a etapa `logs` concluída. Polkit
  `org.lyraos.vega.hardware.diagnose`.

## Teste de rede (`diagnostics-v4`)

O teste `network` verifica, em ordem, enlace, endereço, gateway, DNS e
internet (a checagem do NetworkManager, que detecta portal cativo). A métrica
`steps` lista `{id, status, detail}` com `status` igual a `ok`, `fail`, `warn`
ou `skipped`; os passos depois da primeira falha ficam `skipped`. A métrica
`portalUrl` traz o endereço de checagem, que o frontend abre no navegador no
achado `network.captive-portal`.

`Start`, `Cancel` e `Report` de um job que é só o teste `network` (ou os
remédios dele) usam `org.lyraos.vega.network.diagnose`, que a sessão local
ativa tem sem senha. Os consertos continuam em
`org.lyraos.vega.hardware.repair`.

## Teste de boot (`diagnostics-v5`)

O teste `boot` lê o último boot: a métrica `phases` traz os segundos de cada
fase do `systemd-analyze time` (`firmware`, `loader`, `kernel`, `initrd`,
`userspace`; as ausentes não existem nesta máquina), `totalSeconds`,
`graphicalSeconds` (até o login, contado no userspace) e `slowest`, até cinco
`{unit, seconds}` que começaram antes do login gráfico. Achados:
`boot.slow`, `boot.wait-online`, `boot.missing-device` e `boot.slow-service`.
Os remédios conferem tudo de novo ao rodar e dizem no resumo como desfazer.
