#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 5 ]]; then
  echo "usage: $0 CORE SHA256 LICENSE DESTINATION FILE_NAME" >&2
  exit 2
fi

core="$1"
expected_sha256="$2"
license="$3"
destination="$4"
file_name="$5"

if [[ ! -f "${core}" ]]; then
  echo "bundled Core does not exist: ${core}" >&2
  exit 1
fi
if [[ ! "${expected_sha256}" =~ ^[[:xdigit:]]{64}$ ]]; then
  echo "bundled Core SHA-256 must contain 64 hexadecimal characters" >&2
  exit 1
fi
if [[ ! -f "${license}" ]]; then
  echo "bundled Core license does not exist: ${license}" >&2
  exit 1
fi
# The license is named after the Core it covers, so the two ship side by side
# without one overwriting the other.
case "${file_name}" in
  sing-box | sing-box.exe) license_name="LICENSE-sing-box" ;;
  xray | xray.exe) license_name="LICENSE-xray" ;;
  *)
    echo "unsupported bundled Core file name: ${file_name}" >&2
    exit 1
    ;;
esac

if command -v shasum >/dev/null 2>&1; then
  actual_sha256="$(shasum -a 256 "${core}" | awk '{print $1}')"
else
  actual_sha256="$(sha256sum "${core}" | awk '{print $1}')"
fi
expected_sha256="$(printf '%s' "${expected_sha256}" | tr '[:upper:]' '[:lower:]')"

if [[ "${actual_sha256}" != "${expected_sha256}" ]]; then
  echo "bundled Core SHA-256 mismatch" >&2
  exit 1
fi

mkdir -p "${destination}"
cp "${core}" "${destination}/${file_name}"
chmod +x "${destination}/${file_name}"
cp "${license}" "${destination}/${license_name}"
