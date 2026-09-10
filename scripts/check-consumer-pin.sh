#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <consumer-checkout> <vega|vega-web>" >&2
  exit 2
fi

consumer_root="$(readlink -f "$1")"
consumer="$2"
contract_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$contract_root/Cargo.toml" | head -1)"

case "$consumer" in
  vega) manifest="$consumer_root/vega-gtk/Cargo.toml" ;;
  vega-web) manifest="$consumer_root/Cargo.toml" ;;
  *) echo "unsupported consumer: $consumer" >&2; exit 2 ;;
esac

lockfile="$consumer_root/Cargo.lock"
for file in "$manifest" "$lockfile"; do
  if [ ! -f "$file" ]; then
    echo "missing consumer contract file: $file" >&2
    exit 1
  fi
done

python3 - "$manifest" "$lockfile" "$version" <<'CHECK'
import re
import sys
import tomllib
from pathlib import Path

manifest, lockfile, expected_version = sys.argv[1:]
dep = tomllib.loads(Path(manifest).read_text())["dependencies"]["lyra-vega-dbus"]
packages = [p for p in tomllib.loads(Path(lockfile).read_text())["package"] if p["name"] == "lyra-vega-dbus"]
if len(packages) != 1:
    raise SystemExit("expected one locked lyra-vega-dbus package")
package = packages[0]
url = "https://github.com/lyra-os-linux/lyra-vega-dbus"
if dep.get("git") != url or package["version"] != expected_version:
    raise SystemExit("contract repository/version mismatch")
if set(dep) == {"git", "tag"} and dep["tag"] == "v" + expected_version:
    expected = "git+" + url + "?tag=" + dep["tag"] + "#"
    valid = package.get("source", "").startswith(expected) and re.fullmatch(r"[0-9a-f]{40}", package["source"][len(expected):])
elif set(dep) == {"git", "rev"} and re.fullmatch(r"[0-9a-f]{40}", dep["rev"]):
    valid = package.get("source") == "git+" + url + "?rev=" + dep["rev"] + "#" + dep["rev"]
else:
    valid = False
if not valid:
    raise SystemExit("contract must use a matching release tag or full immutable revision in manifest and lockfile")
print("consumer pins lyra-vega-dbus " + expected_version + " at an explicit matching Git reference")
CHECK
