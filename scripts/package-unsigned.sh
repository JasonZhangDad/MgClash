#!/usr/bin/env bash
#
# Builds the unsigned release artifact for the host platform.
#
# PRD V1.1 section 5 fixes the artifact shape per platform, and Definition of
# Done item 5 fixes the file name: it must carry the OS, the CPU, the version,
# and the word `unsigned`. Nothing here signs, notarizes, or staples anything —
# "unsigned" is the product decision recorded in ADR 0001/0002, not an omission.
#
# Usage: scripts/package-unsigned.sh [output-directory]

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
desktop="${repository}/apps/desktop"
output_directory="${1:-${repository}/dist}"

version="$(
  sed -n 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    "${desktop}/src-tauri/tauri.conf.json" | head -n 1
)"
if [[ -z "${version}" ]]; then
  echo "could not read the version from tauri.conf.json" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin) os="macos" ;;
  Linux) os="linux" ;;
  MINGW* | MSYS* | CYGWIN*) os="windows" ;;
  *)
    echo "unsupported build host: $(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) cpu="aarch64" ;;
  x86_64 | amd64) cpu="x86_64" ;;
  *)
    echo "unsupported build CPU: $(uname -m)" >&2
    exit 1
    ;;
esac

name="mgclash-${version}-${os}-${cpu}-unsigned"
mkdir -p "${output_directory}"

echo "building ${name}"
cd "${desktop}"
npm run build

if [[ "${os}" == "macos" ]]; then
  # Only macOS gets a bundle: the .app layout is what Gatekeeper prompts about,
  # and a bare Mach-O binary is not a usable desktop artifact.
  npm run tauri -- build --bundles app
  bundle_directory="${repository}/target/release/bundle/macos"
  app="${bundle_directory}/MgClash.app"
  [[ -d "${app}" ]] || {
    echo "expected ${app} to exist after bundling" >&2
    exit 1
  }
  archive="${output_directory}/${name}.tar.gz"
  tar -czf "${archive}" -C "${bundle_directory}" "MgClash.app"
else
  # Windows and Linux ship the portable executable: Tauri embeds the frontend,
  # so the binary is self-contained.
  npm run tauri -- build --no-bundle
  if [[ "${os}" == "windows" ]]; then
    binary="${repository}/target/release/MgClash.exe"
    [[ -f "${binary}" ]] || binary="${repository}/target/release/magies-desktop.exe"
  else
    binary="${repository}/target/release/MgClash"
    [[ -f "${binary}" ]] || binary="${repository}/target/release/magies-desktop"
  fi
  [[ -f "${binary}" ]] || {
    echo "expected a release binary in ${repository}/target/release" >&2
    exit 1
  }

  staging="$(mktemp -d)"
  trap 'rm -rf "${staging}"' EXIT
  mkdir -p "${staging}/${name}"
  cp "${binary}" "${staging}/${name}/"
  cp "${repository}/README.md" "${staging}/${name}/"

  if [[ "${os}" == "windows" ]]; then
    archive="${output_directory}/${name}.zip"
    # Git Bash on a Windows runner does not reliably ship `zip`, so fall back
    # to the tools the image is guaranteed to have.
    if command -v zip >/dev/null 2>&1; then
      (cd "${staging}" && zip -qr "${archive}" "${name}")
    elif command -v 7z >/dev/null 2>&1; then
      (cd "${staging}" && 7z a -bso0 -bsp0 "${archive}" "${name}" >/dev/null)
    else
      powershell.exe -NoProfile -Command \
        "Compress-Archive -Path '${staging}\\${name}' -DestinationPath '${archive}' -Force"
    fi
  else
    archive="${output_directory}/${name}.tar.gz"
    tar -czf "${archive}" -C "${staging}" "${name}"
  fi
fi

# The digest is what a downloader can check in the absence of a signature.
if command -v shasum >/dev/null 2>&1; then
  (cd "${output_directory}" && shasum -a 256 "$(basename "${archive}")" > "$(basename "${archive}").sha256")
else
  (cd "${output_directory}" && sha256sum "$(basename "${archive}")" > "$(basename "${archive}").sha256")
fi

echo "packaged ${archive}"
cat "${archive}.sha256"
