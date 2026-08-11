#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stage_script="${repository}/scripts/stage-bundled-wintun.sh"
sandbox="$(mktemp -d)"
trap 'rm -rf "${sandbox}"' EXIT

dll="${sandbox}/downloaded-wintun.dll"
license="${sandbox}/LICENSE.txt"
printf 'signed Wintun fixture' > "${dll}"
printf 'fixture license' > "${license}"

destination="${sandbox}/artifact"
"${stage_script}" "${dll}" "${license}" "${destination}"

cmp "${dll}" "${destination}/wintun.dll"
cmp "${license}" "${destination}/LICENSE-wintun"

if "${stage_script}" "${sandbox}/missing.dll" "${license}" \
  "${sandbox}/missing-dll-artifact" 2>/dev/null; then
  echo "missing Wintun DLL unexpectedly passed validation" >&2
  exit 1
fi

if "${stage_script}" "${dll}" "${sandbox}/missing-license" \
  "${sandbox}/missing-license-artifact" 2>/dev/null; then
  echo "missing Wintun license unexpectedly passed validation" >&2
  exit 1
fi
