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

- `severity` vale `info`, `attention` ou `fail`. `info` agrupa o ruído
  conhecido (bugs de firmware ACPI, avisos comuns de boot e afins).
- `code` é estável. Os frontends traduzem e agrupam por ele, nunca pelo `text`.
- `metrics` varia por teste. As chaves ausentes não foram medidas.
- Com `redact = true`, saem os números de série, endereços MAC, SSIDs e o
  hostname.
- Um campo novo não muda o `schema`. Remover ou mudar o significado de um campo
  exige `schema: 2`.
