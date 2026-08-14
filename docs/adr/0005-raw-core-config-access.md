# ADR 0005: The UI may read and supply raw Core configuration

## Status

Accepted for V0.1. **Supersedes PRD V1.0 constraint 4 in part.**

## Context

PRD constraint 4 says the UI never reads or writes Core JSON: everything goes
through Tauri commands, and the generators are the only thing that produces a
config. That constraint bought two things — the UI cannot desynchronize from the
domain model, and a user cannot hand the app a config it never validated.

v2rayN offers both of the things the constraint forbids:

- `FullConfigTemplateWindow` — a complete Core config the app injects its
  outbounds into, which is how users keep settings the generators do not model
  (custom inbounds, experimental fields, per-core tuning).
- `JsonEditor` — viewing and editing the generated config before it runs.

The project's goal is parity with v2rayN. Asked to choose between the
constraint and the parity goal, the owner chose parity, on 2026-08-14.

## Decision

Both are allowed, with the generator kept as the only path a config takes to
disk:

1. **A template is an input to generation, not a replacement for it.** The
   template is parsed, the generated sections are merged into it, and the result
   goes through the same `sing-box check` / `xray run -test` validation and the
   same `AtomicRuntimeConfig::write` as an untemplated config. A template that
   produces an invalid document fails before the Core starts, with the same
   typed error an invalid node produces.
2. **An override is stored as text and validated the same way.** Editing the
   generated config stores the edited text against the profile; it is validated
   by the Core binary before a session starts, never at the moment of typing.
3. **The UI still does not write Core JSON to disk.** It sends text to a Tauri
   command; the command owns parsing, merging, validating, and writing. What
   the constraint actually protected — one code path to disk, one validation
   point — is unchanged.
4. **Neither is on by default.** A profile with no template and no override
   generates exactly what it generated before this ADR.

## Consequences

- A user can now produce a config the domain model does not describe. The UI
  shows what was generated, not what the model believes; where the two disagree,
  the config wins, because it is what the Core runs.
- Diagnostics carry template and override text through `DiagnosticRedactor`
  like any other config — a hand-written config may contain credentials the
  domain model never saw.
- The generators remain the only producer of the non-templated parts, so adding
  a field still means adding it to a generator and its smoke test. A template is
  not a supported way to work around a missing generator feature; it is a way to
  carry settings the project does not model.
- PRD constraint 4 is now inaccurate as written. This ADR, not the PRD, is the
  current rule.
