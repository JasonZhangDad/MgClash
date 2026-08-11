#!/usr/bin/env bash
#
# Verifies the complete V0.1 unsigned release set and writes SHA256SUMS.
#
# Usage: scripts/verify-unsigned-release.sh <release-directory> <version>

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: $0 <release-directory> <version>" >&2
  exit 2
fi

release_directory="$1"
version="$2"

if [[ ! -d "${release_directory}" ]]; then
  echo "release directory does not exist: ${release_directory}" >&2
  exit 1
fi
if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid release version: ${version}" >&2
  exit 1
fi

artifacts=(
  "mgclash-${version}-macos-x86_64-unsigned.tar.gz"
  "mgclash-${version}-macos-aarch64-unsigned.tar.gz"
  "mgclash-${version}-windows-x86_64-unsigned.zip"
  "mgclash-${version}-windows-x86_64-unsigned-setup.exe"
  "mgclash-${version}-linux-x86_64-unsigned.tar.gz"
)

checksums="$(mktemp "${release_directory}/.SHA256SUMS.XXXXXX")"
trap 'rm -f "${checksums}"' EXIT

for artifact in "${artifacts[@]}"; do
  artifact_path="${release_directory}/${artifact}"
  sidecar="${artifact_path}.sha256"
  [[ -f "${artifact_path}" ]] || {
    echo "release artifact is missing: ${artifact}" >&2
    exit 1
  }
  [[ -f "${sidecar}" ]] || {
    echo "release checksum is missing: ${artifact}.sha256" >&2
    exit 1
  }

  read -r recorded_digest recorded_name < "${sidecar}"
  recorded_name="${recorded_name%$'\r'}"
  recorded_name="${recorded_name#\*}"
  if [[ "${recorded_name}" != "${artifact}" ]]; then
    echo "release checksum names the wrong artifact: ${artifact}.sha256" >&2
    exit 1
  fi

  if command -v shasum >/dev/null 2>&1; then
    actual_digest="$(shasum -a 256 "${artifact_path}" | awk '{print $1}')"
  else
    actual_digest="$(sha256sum "${artifact_path}" | awk '{print $1}')"
  fi
  if [[ "${actual_digest}" != "${recorded_digest}" ]]; then
    echo "release checksum mismatch: ${artifact}" >&2
    exit 1
  fi

  printf '%s  %s\n' "${recorded_digest}" "${artifact}" >> "${checksums}"
done

LC_ALL=C sort "${checksums}" > "${release_directory}/SHA256SUMS"
echo "verified ${#artifacts[@]} unsigned release artifacts for ${version}"
