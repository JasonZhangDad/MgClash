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
