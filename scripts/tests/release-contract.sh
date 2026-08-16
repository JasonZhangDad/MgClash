#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
package_script="${repository}/scripts/package-unsigned.sh"
release_workflow="${repository}/.github/workflows/release.yml"
release_notes="${repository}/docs/releases/v0.1.0.md"
windows_release_config="${repository}/apps/desktop/src-tauri/tauri.windows-release.conf.json"

grep -Fq -- '--bundles nsis' "${package_script}"
grep -Fq -- 'windows-${cpu}-unsigned-setup.exe' "${package_script}"
grep -Fq -- 'dist/*.exe' "${release_workflow}"
grep -Fq -- 'actions/download-artifact@v4' "${release_workflow}"
grep -Fq -- 'scripts/verify-unsigned-release.sh' "${release_workflow}"
grep -Fq -- 'gh release create' "${release_workflow}"
grep -Fq -- 'contents: write' "${release_workflow}"
grep -Fqi -- 'unsigned' "${release_notes}"
grep -Fq -- '../../../target/release-resources/sing-box.exe' "${windows_release_config}"
grep -Fq -- '../../../target/release-resources/wintun.dll' "${windows_release_config}"

# Both Cores have to reach every artifact. 1.1.0 downloaded and verified Xray
# and then shipped without it, so picking the Xray Core failed at runtime on a
# release whose build had gone green.
linux_release_config="${repository}/apps/desktop/src-tauri/tauri.linux-release.conf.json"
for entry in xray LICENSE-xray geoip.dat geosite.dat; do
  grep -Fq -- "../../../target/release-resources/${entry}" "${linux_release_config}"
done
for entry in xray.exe LICENSE-xray geoip.dat geosite.dat; do
  grep -Fq -- "../../../target/release-resources/${entry}" "${windows_release_config}"
done
# … and the packaging script must place them, not just stage them.
grep -Fq -- 'copy_cores "${app}/Contents/Resources"' "${package_script}"
grep -Fq -- 'copy_cores "${staging}/${name}"' "${package_script}"
grep -Fq -- 'cp "${scratch}/core/${xray_file_name}" "${target}/"' "${package_script}"
grep -Fq -- 'MAGIES_BUNDLED_XRAY_BIN is not set' "${package_script}"
