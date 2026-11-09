# Liberar espaço (`org.lyraos.Vega1.Cleanup`)

Anunciada pela capability `space-cleanup-v1`. Requisitos vigentes:
[lyra-ecosystem/openspec/specs/space-cleanup](https://github.com/lyra-os-linux/lyra-ecosystem/blob/main/openspec/specs/space-cleanup/spec.md);
histórico e design:
[archive/2026-10-30-add-space-cleanup](https://github.com/lyra-os-linux/lyra-ecosystem/tree/main/openspec/changes/archive/2026-10-30-add-space-cleanup).

## Categorias do sistema

| id | Mede | Limpa |
| --- | --- | --- |
| `coredumps` | arquivos em `/var/lib/systemd/coredump` | apaga os arquivos |
| `journal` | arquivos arquivados do journal acima de 500 MiB que terminam antes do boot anterior | apaga só esses arquivos, do mais antigo ao mais novo; o boot atual e o anterior ficam |
| `package-cache` | `/var/cache/zypp/packages` | `zypper clean`, que apaga só os pacotes baixados e mantém os metadados |
| `flatpak-unused` | os runtimes que `flatpak uninstall --unused --system` listaria | esse mesmo comando, sem perguntas |
| `old-kernels` | `zypper purge-kernels --dry-run`, pela regra `multiversion.kernels` | `zypper purge-kernels`, entre dois snapshots quando o Snapper está configurado (por tentativa: sem Snapper, como no Server, ou se o snapshot falhar, roda sem ele; no Desktop o `snapper-zypp-plugin` também cria um par); recusa se a lista tiver o kernel em uso ou o mais novo |
| `snapshots` | snapshots `number` e `timeline` acima dos limites do Snapper (pelo menos esse número; tamanho desconhecido sem quotas do btrfs) | `snapper cleanup number` e `snapper cleanup timeline` |

Lixeira, miniaturas, `~/.cache` e runtimes Flatpak do usuário são limpos pelo
frontend, na sessão, sem passar pelo vegad.

## `Scan() -> a(stbs)`

Cada linha traz `(id, bytes, canClean, detail)`. `Scan` só mede. O `detail` é
uma lista de tokens separados por `;`, e os desconhecidos são ignorados:

| Token | Significado |
| --- | --- |
| `count=<n>` | arquivos, runtimes, kernels ou snapshots |
| `size=unknown` | o tamanho não pode ser medido antes |
| `minimum` | `count` é um mínimo |
| `deferred` | os bytes só voltam ao disco quando os snapshots que ainda guardam os arquivos forem limpos (`old-kernels` com Snapper); o `CategoryFinished` dessa categoria informa 0 bytes liberados |
| `reason=<id>` | por que `canClean` é falso: `nothing`, `busy`, `missing-tool:<pacote>`, `not-configured`, `protected` (o plano tiraria o kernel em uso ou o mais novo), `failed` (a medição falhou) |
| `item=<texto>` | um item (versão do kernel, ref do runtime), repetido, no máximo 20 |

Polkit: `org.lyraos.vega.storage.read-admin`, com `allow_active` `yes`. Abrir a
seção não pede senha.

## `Clean(ids) -> jobId`

Limpa as categorias na ordem recebida. Sinais:

- `Progress(jobId, id, percent, message)`;
- `CategoryFinished(jobId, id, result, freedBytes)`, com `result` igual a `ok`,
  `failed` ou `skipped` (nada a limpar quando o job chegou nela);
- `Finished(jobId, result, freedBytes)`, com `result` igual a `ok`, `partial`
  ou `failed`, e o total liberado medido depois da limpeza.

Erros (`org.lyraos.Vega1.Error.*`):

- `Busy`: um diagnóstico, outra limpeza ou uma transação de software,
  snapshot ou backup está em andamento;
- `NotSupported`: lista vazia ou id desconhecido.

Polkit: `org.lyraos.vega.storage.cleanup` (`auth_admin_keep`).
