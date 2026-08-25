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
expected_tag="v$version"

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

if ! grep -F "lyra-vega-dbus = { git = \"https://github.com/lyra-os-linux/lyra-vega-dbus\", tag = \"$expected_tag\" }" "$manifest" >/dev/null; then
  echo "$consumer does not pin lyra-vega-dbus at $expected_tag" >&2
  exit 1
fi
if ! grep -F "lyra-vega-dbus?tag=$expected_tag#" "$lockfile" >/dev/null; then
  echo "$consumer Cargo.lock does not resolve lyra-vega-dbus at $expected_tag" >&2
  exit 1
fi

echo "$consumer pins lyra-vega-dbus at $expected_tag"
