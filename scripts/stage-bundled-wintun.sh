#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 3 ]]; then
  echo "usage: $0 WINTUN_DLL LICENSE DESTINATION" >&2
  exit 2
fi

dll="$1"
license="$2"
destination="$3"

if [[ ! -f "${dll}" ]]; then
  echo "bundled Wintun DLL does not exist: ${dll}" >&2
  exit 1
fi
if [[ ! -f "${license}" ]]; then
  echo "bundled Wintun license does not exist: ${license}" >&2
  exit 1
fi

mkdir -p "${destination}"
cp "${dll}" "${destination}/wintun.dll"
cp "${license}" "${destination}/LICENSE-wintun"
