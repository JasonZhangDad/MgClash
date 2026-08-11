#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stage_script="${repository}/scripts/stage-bundled-core.sh"
sandbox="$(mktemp -d)"
trap 'rm -rf "${sandbox}"' EXIT

core="${sandbox}/downloaded-sing-box"
license="${sandbox}/LICENSE"
printf 'pinned sing-box fixture' > "${core}"
printf 'fixture license' > "${license}"

if command -v shasum >/dev/null 2>&1; then
  digest="$(shasum -a 256 "${core}" | awk '{print $1}')"
else
  digest="$(sha256sum "${core}" | awk '{print $1}')"
fi

destination="${sandbox}/artifact"
"${stage_script}" "${core}" "${digest}" "${license}" "${destination}" "sing-box"

cmp "${core}" "${destination}/sing-box"
cmp "${license}" "${destination}/LICENSE-sing-box"

windows_destination="${sandbox}/windows-artifact"
"${stage_script}" "${core}" "${digest}" "${license}" \
  "${windows_destination}" "sing-box.exe"
cmp "${core}" "${windows_destination}/sing-box.exe"

if "${stage_script}" "${sandbox}/missing" "${digest}" "${license}" \
  "${sandbox}/missing-artifact" "sing-box" 2>/dev/null; then
  echo "missing Core unexpectedly passed validation" >&2
  exit 1
fi

if "${stage_script}" "${core}" \
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
  "${license}" "${sandbox}/tampered-artifact" "sing-box" 2>/dev/null; then
  echo "tampered Core unexpectedly passed validation" >&2
  exit 1
fi

if [[ -e "${sandbox}/tampered-artifact/sing-box" ]]; then
  echo "tampered Core was copied before validation" >&2
  exit 1
fi

if "${stage_script}" "${core}" "${digest}" "${sandbox}/missing-license" \
  "${sandbox}/unlicensed-artifact" "sing-box" 2>/dev/null; then
  echo "missing Core license unexpectedly passed validation" >&2
  exit 1
fi

if "${stage_script}" "${core}" "${digest}" "${license}" \
  "${sandbox}/wrong-name-artifact" "core" 2>/dev/null; then
  echo "unsupported Core file name unexpectedly passed validation" >&2
  exit 1
fi
