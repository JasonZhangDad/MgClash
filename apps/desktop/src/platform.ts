import { invoke } from "@tauri-apps/api/core";

export interface PlatformSummary {
  artifactIdentifier: string;
  tunAvailability:
    | "requiresElevation"
    | "unavailableInUnsignedBuild";
}

export function loadPlatformSummary(): Promise<PlatformSummary> {
  return invoke<PlatformSummary>("platform_summary");
}
