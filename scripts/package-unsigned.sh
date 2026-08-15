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
core="${MAGIES_BUNDLED_SING_BOX_BIN:-}"
core_sha256="${MAGIES_SING_BOX_SHA256:-}"
core_license="${MAGIES_BUNDLED_SING_BOX_LICENSE:-}"
xray="${MAGIES_BUNDLED_XRAY_BIN:-}"
xray_sha256="${MAGIES_XRAY_SHA256:-}"
xray_license="${MAGIES_BUNDLED_XRAY_LICENSE:-}"
xray_geoip="${MAGIES_BUNDLED_XRAY_GEOIP:-}"
xray_geosite="${MAGIES_BUNDLED_XRAY_GEOSITE:-}"
wintun="${MAGIES_BUNDLED_WINTUN_DLL:-}"
wintun_license="${MAGIES_BUNDLED_WINTUN_LICENSE:-}"

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
artifacts=()

scratch="$(mktemp -d)"
trap 'rm -rf "${scratch}"' EXIT
if [[ "${os}" == "windows" ]]; then
  core_file_name="sing-box.exe"
  xray_file_name="xray.exe"
else
  core_file_name="sing-box"
  xray_file_name="xray"
fi
bash "${repository}/scripts/stage-bundled-core.sh" \
  "${core}" "${core_sha256}" "${core_license}" \
  "${scratch}/core" "${core_file_name}"
# Both Cores ship so switching in settings works without the user finding a
# binary. Xray is staged only when it is configured: release.yml has no pinned
# Xray digest yet (ADR 0003), and failing the whole build over that would block
# releases that are otherwise fine. The skip is announced rather than silent,
# because an artifact missing a Core fails later and less clearly.
if [[ -n "${xray}" ]]; then
  # The geo databases travel with the binary: Xray refuses to start when a
  # geoip: or geosite: routing rule cannot find them, which is exactly what
  # Rule mode generates.
  bash "${repository}/scripts/stage-bundled-core.sh" \
    "${xray}" "${xray_sha256}" "${xray_license}" \
    "${scratch}/core" "${xray_file_name}" \
    "${xray_geoip}" "${xray_geosite}"
else
  echo "no MAGIES_BUNDLED_XRAY_BIN: this artifact ships without Xray" >&2
fi
if [[ "${os}" == "windows" ]]; then
  bash "${repository}/scripts/stage-bundled-wintun.sh" \
    "${wintun}" "${wintun_license}" "${scratch}/wintun"
fi
export MAGIES_SING_BOX_SHA256="${core_sha256}"
export MAGIES_XRAY_SHA256="${xray_sha256}"

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
  mkdir -p "${app}/Contents/Resources"
  cp "${scratch}/core/${core_file_name}" "${app}/Contents/Resources/"
  cp "${scratch}/core/LICENSE-sing-box" "${app}/Contents/Resources/"
  archive="${output_directory}/${name}.tar.gz"
  tar -czf "${archive}" -C "${bundle_directory}" "MgClash.app"

  # Built here rather than by `tauri build --bundles dmg`: that would rebuild
  # the .app and lose the Core copied into it above. hdiutil needs no signing
  # identity, and the /Applications symlink is what makes a .dmg an installer
  # rather than a folder.
  disk_image_root="${scratch}/dmg"
  mkdir -p "${disk_image_root}"
  cp -R "${app}" "${disk_image_root}/"
  ln -s /Applications "${disk_image_root}/Applications"
  disk_image="${output_directory}/${name}.dmg"
  rm -f "${disk_image}"
  hdiutil create \
    -volname "MgClash" \
    -srcfolder "${disk_image_root}" \
    -ov -format UDZO \
    -quiet \
    "${disk_image}"
  artifacts+=("${disk_image}")
else
  # Windows and Linux ship the portable executable: Tauri embeds the frontend,
  # so the binary is self-contained.
  if [[ "${os}" == "windows" ]]; then
    release_resources="${repository}/target/release-resources"
    mkdir -p "${release_resources}"
    cp "${scratch}/core/sing-box.exe" "${release_resources}/"
    cp "${scratch}/core/LICENSE-sing-box" "${release_resources}/"
    cp "${scratch}/wintun/wintun.dll" "${release_resources}/"
    cp "${scratch}/wintun/LICENSE-wintun" "${release_resources}/"
    npm run tauri -- build --bundles nsis \
      --config src-tauri/tauri.windows-release.conf.json
    binary="${repository}/target/release/MgClash.exe"
    [[ -f "${binary}" ]] || binary="${repository}/target/release/magies-desktop.exe"
  else
    release_resources="${repository}/target/release-resources"
    mkdir -p "${release_resources}"
    cp "${scratch}/core/sing-box" "${release_resources}/"
    cp "${scratch}/core/LICENSE-sing-box" "${release_resources}/"
    npm run tauri -- build --bundles deb,rpm,appimage \
      --config src-tauri/tauri.linux-release.conf.json
    binary="${repository}/target/release/MgClash"
    [[ -f "${binary}" ]] || binary="${repository}/target/release/magies-desktop"
  fi
  [[ -f "${binary}" ]] || {
    echo "expected a release binary in ${repository}/target/release" >&2
    exit 1
  }

  staging="${scratch}/staging"
  mkdir -p "${staging}/${name}"
  cp "${binary}" "${staging}/${name}/"
  cp "${repository}/README.md" "${staging}/${name}/"
  cp "${scratch}/core/${core_file_name}" "${staging}/${name}/"
  cp "${scratch}/core/LICENSE-sing-box" "${staging}/${name}/"
  if [[ "${os}" == "windows" ]]; then
    cp "${scratch}/wintun/wintun.dll" "${staging}/${name}/"
    cp "${scratch}/wintun/LICENSE-wintun" "${staging}/${name}/"
  fi

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

    installer_source="${repository}/target/release/bundle/nsis/MgClash_${version}_x64-setup.exe"
    [[ -f "${installer_source}" ]] || {
      echo "expected the NSIS installer at ${installer_source}" >&2
      exit 1
    }
    installer="${output_directory}/mgclash-${version}-windows-${cpu}-unsigned-setup.exe"
    cp "${installer_source}" "${installer}"
    artifacts+=("${installer}")
    # The portable executable on its own, which is what a user who does not
    # want an installer actually downloads.
    portable="${output_directory}/mgclash-${version}-windows-${cpu}-unsigned-portable.exe"
    cp "${binary}" "${portable}"
    artifacts+=("${portable}")
  else
    bundle_root="${repository}/target/release/bundle"
    deb_source="$(find "${bundle_root}/deb" -name '*.deb' -type f | head -n 1)"
    rpm_source="$(find "${bundle_root}/rpm" -name '*.rpm' -type f | head -n 1)"
    appimage_source="$(find "${bundle_root}/appimage" -name '*.AppImage' -type f | head -n 1)"
    for source in "${deb_source}" "${rpm_source}" "${appimage_source}"; do
      [[ -n "${source}" && -f "${source}" ]] || {
        echo "a Linux package is missing from ${bundle_root}" >&2
        exit 1
      }
    done

    # Checked rather than assumed: the app looks for its bundled Core under
    # `/usr/lib/<name>/`, and a package that puts it anywhere else installs an
    # app that cannot find its own Core. Failing here beats shipping that.
    if command -v dpkg-deb >/dev/null 2>&1; then
      # Captured before matching rather than piped into grep: `grep -q` closes
      # the pipe on its first match, dpkg-deb dies of SIGPIPE, and `pipefail`
      # then reports the whole pipeline as failed — so a package that *does*
      # carry the Core fails the check precisely because it passes it.
      deb_listing="$(dpkg-deb -c "${deb_source}")"
      # The leading `./` is optional: dpkg-deb prints it in some versions and
      # not in others, and requiring it turned a correct package into a build
      # failure once already.
      if ! grep -qE '(\./)?usr/lib/[^/]+/sing-box$' <<<"${deb_listing}"; then
        echo "the deb does not carry sing-box under /usr/lib/<name>/" >&2
        grep -i "sing-box" <<<"${deb_listing}" >&2 || true
        exit 1
      fi
    else
      echo "no dpkg-deb: the deb's Core layout was not checked" >&2
    fi

    cp "${deb_source}" "${output_directory}/${name}.deb"
    cp "${rpm_source}" "${output_directory}/${name}.rpm"
    cp "${appimage_source}" "${output_directory}/${name}.AppImage"
    artifacts+=(
      "${output_directory}/${name}.deb"
      "${output_directory}/${name}.rpm"
      "${output_directory}/${name}.AppImage"
    )

    archive="${output_directory}/${name}.tar.gz"
    tar -czf "${archive}" -C "${staging}" "${name}"
  fi
fi

# The digest is what a downloader can check in the absence of a signature.
artifacts+=("${archive}")
for artifact in "${artifacts[@]}"; do
  artifact_name="$(basename "${artifact}")"
  if command -v shasum >/dev/null 2>&1; then
    (cd "${output_directory}" && shasum -a 256 "${artifact_name}" > "${artifact_name}.sha256")
  else
    (cd "${output_directory}" && sha256sum "${artifact_name}" > "${artifact_name}.sha256")
  fi
  echo "packaged ${artifact}"
  cat "${artifact}.sha256"
done
