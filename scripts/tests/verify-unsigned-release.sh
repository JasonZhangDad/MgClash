#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
verify_script="${repository}/scripts/verify-unsigned-release.sh"
sandbox="$(mktemp -d)"
trap 'rm -rf "${sandbox}"' EXIT

release_directory="${sandbox}/release"
mkdir -p "${release_directory}"

write_digest() {
  local artifact="$1"
  local directory
  local name
  directory="$(dirname "${artifact}")"
  name="$(basename "${artifact}")"

  if command -v shasum >/dev/null 2>&1; then
    (cd "${directory}" && shasum -a 256 "${name}" > "${name}.sha256")
  else
    (cd "${directory}" && sha256sum "${name}" > "${name}.sha256")
  fi
}

create_artifact() {
  local name="$1"
  printf 'fixture for %s\n' "${name}" > "${release_directory}/${name}"
  write_digest "${release_directory}/${name}"
}

artifacts=(
  "mgclash-0.1.0-macos-x86_64-unsigned.tar.gz"
  "mgclash-0.1.0-macos-x86_64-unsigned.dmg"
  "mgclash-0.1.0-macos-aarch64-unsigned.tar.gz"
  "mgclash-0.1.0-macos-aarch64-unsigned.dmg"
  "mgclash-0.1.0-windows-x86_64-unsigned.zip"
  "mgclash-0.1.0-windows-x86_64-unsigned-setup.exe"
  "mgclash-0.1.0-windows-x86_64-unsigned-portable.exe"
  "mgclash-0.1.0-linux-x86_64-unsigned.tar.gz"
  "mgclash-0.1.0-linux-x86_64-unsigned.deb"
  "mgclash-0.1.0-linux-x86_64-unsigned.rpm"
  "mgclash-0.1.0-linux-x86_64-unsigned.AppImage"
)

for artifact in "${artifacts[@]}"; do
  create_artifact "${artifact}"
done

# Windows checksum tools may terminate the sidecar line with CRLF. The
# Linux publish job must accept that file without weakening name validation.
windows_sidecar="${release_directory}/${artifacts[0]}.sha256"
read -r windows_digest _ < "${windows_sidecar}"
printf '%s *%s\r\n' "${windows_digest}" "${artifacts[0]}" > "${windows_sidecar}"

"${verify_script}" "${release_directory}" "0.1.0"

# Written as an explicit failure rather than a bare `[[ ]]`: macOS ships bash
# 3.2, where `set -e` lets a failed test fall through, so a bare assertion is a
# no-op on a Mac and a hard failure on CI. That divergence hid a wrong count
# here until a release run caught it.
[[ -f "${release_directory}/SHA256SUMS" ]] || {
  echo "verification produced no SHA256SUMS" >&2
  exit 1
}
# One line per artifact, counted rather than assumed: SHA256SUMS is the file a
# downloader checks the whole release against, so a missing line is a download
# nobody can verify.
recorded_lines="$(wc -l < "${release_directory}/SHA256SUMS" | tr -d ' ')"
[[ "${recorded_lines}" == "${#artifacts[@]}" ]] || {
  echo "SHA256SUMS has ${recorded_lines} lines, expected ${#artifacts[@]}" >&2
  exit 1
}
for artifact in "${artifacts[@]}"; do
  grep -Fq "  ${artifact}" "${release_directory}/SHA256SUMS"
done

installer="${release_directory}/mgclash-0.1.0-windows-x86_64-unsigned-setup.exe"
rm "${installer}" "${installer}.sha256"
if "${verify_script}" "${release_directory}" "0.1.0" 2>/dev/null; then
  echo "release without the Windows installer unexpectedly passed" >&2
  exit 1
fi

create_artifact "$(basename "${installer}")"
printf 'tampered\n' >> "${release_directory}/mgclash-0.1.0-linux-x86_64-unsigned.tar.gz"
if "${verify_script}" "${release_directory}" "0.1.0" 2>/dev/null; then
  echo "release with a checksum mismatch unexpectedly passed" >&2
  exit 1
fi
