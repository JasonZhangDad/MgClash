# ADR 0004: v2rayN Avalonia as UI / IA template

## Status

Accepted.

## Context

PRD V1.0 §3.3 item 2 stated that the UI would be native SwiftUI/AppKit and
would **not** use v2rayN as a visual template. The shipping client is instead a
Tauri 2 + React shell (`apps/desktop`), and the current left-sidebar multi-page
console diverged from the Core-manager workflow users expect from v2rayN.

Phase-one product success is still “replace v2rayN for daily proxy use.” Users
asked to realign the desktop shell so the information architecture and component
language match v2rayN’s Avalonia cross-platform client
([2dust/v2rayN](https://github.com/2dust/v2rayN)), not a dashboard-style product.

v2rayN is GPL-3.0. Copying its C#/XAML sources into this repository would create
a license conflict with a closed or differently licensed product.

## Decision

1. **UI / IA template**: MgClash’s desktop shell follows v2rayN Avalonia
   (`v2rayN.Desktop` + Semi.Avalonia-like control language): top menu bar,
   Profiles-first main view (subscription group bar + server DataGrid), modal
   dialogs for options / subscription / routing / DNS / server edit, status bar
   for system proxy and routing mode, optional message pane.
2. **PRD override**: PRD V1.0 §3.3 item 2 (“不以 v2rayN 界面为模板”) is
   superseded by this ADR for the Tauri desktop shell. Feature parity with
   v2rayN remains the phase-one bar; Shadowrocket-style Module / MITM work stays
   later-phase and out of scope for this template decision.
3. **No GPL source import**: Do not vendor, paste, or translate v2rayN source
   files. Reimplement layout and behaviour from public UX (screenshots, wiki,
   observed menus). Branding (product name, about box) remains MgClash.
4. **Stack unchanged**: Tauri 2 + React + Rust crates. The UI still never reads
   or writes Core JSON; it goes through Tauri commands and the capability matrix.

## Consequences

- Sidebar multi-page navigation is removed in favour of a Profiles-centric main
  window and dialogs.
- Visual CSS moves from a custom dark “console” skin toward a dense desktop
  DataGrid / menu / status-bar look with Light/Dark themes.
- Feature work is prioritized by v2rayN daily-use gaps (full server edit, PAC,
  inbound ports, tray, layout orientations) rather than dashboard chrome.
- Legal review still forbids shipping GPL-derived code; behavioural parity is
  fine, source copying is not.
