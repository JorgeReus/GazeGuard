# macOS Validation Logger Design

## Goal

Add config-driven runtime logging for desktop signal validation, with immediate focus on `macOS` idle/fullscreen verification. Logging must be controlled only through `config.yaml`, stay silent by default, and require no environment variables.

## Scope

In scope:

- add global `log_level` config field to shared runtime config
- parse and carry `log_level` through Rust runtime config state
- add small logger helper in `src-tauri/src/logger.rs`
- emit logs from desktop signal collection path and signal-application path
- support manual `macOS` validation with terminal-visible logs

Out of scope:

- full structured logging framework
- per-subsystem log routing or filtering
- file-backed logs
- UI for changing log level
- Linux/Windows behavior changes beyond sharing parsed config field

## Config Shape

Shared config adds one top-level field:

```yaml
log_level: off
```

Allowed values:

- `off`
- `error`
- `warn`
- `info`
- `debug`
- `trace`

Behavior:

- missing field defaults to `off`
- invalid field value falls back to `off`
- value applies globally so later subsystems can reuse same setting
- current implementation only emits logs from desktop signal path

## Architecture

### Config Parsing

`src-tauri/src/break_engine.rs` remains source of truth for shared config parsing. `BreakEngineConfig` gains parsed logging state derived from `log_level`. Raw YAML parsing accepts optional string value and normalizes it into typed internal representation.

This keeps runtime log decisions tied to already-loaded config rather than separate process state. Config reload continues to work through existing runtime config path.

### Logger Helper

Add `src-tauri/src/logger.rs` with:

- `LogLevel` enum ordered by severity
- parser from config string
- helper to test whether one level enables another
- small log function used by runtime call sites

Design intent:

- no external logging crate
- minimal API surface
- terminal/stderr output only
- messages formatted for debugging, not analytics

Expected shape:

- `LogLevel::Off`
- `LogLevel::Error`
- `LogLevel::Warn`
- `LogLevel::Info`
- `LogLevel::Debug`
- `LogLevel::Trace`

Logger helper should make call sites cheap and obvious:

- read current configured level
- compare requested level
- emit only if enabled

## Logging Boundaries

### `src-tauri/src/desktop_signals.rs`

Add logs around desktop signal collection, especially `macOS` path:

- fallback window snapshot used for desktop signal fallback
- native idle sample/result
- native fullscreen sample/result
- merged final `DesktopSignals` output
- fallback or lookup-failure cases where native signal unavailable

Focus is to answer:

- did native path run
- what did native path return
- what did fallback path return
- what final merged signal reached engine boundary

### `src-tauri/src/lib.rs`

Add logs where desktop signals are applied into engine via `sync_desktop_window_state`.

Emit:

- configured idle threshold used for collection
- collected `DesktopSignals`
- resulting engine idle/fullscreen state after apply

This separates “provider returned wrong signal” from “signal collected correctly but engine applied it wrong.”

## Error Handling

- invalid `log_level` in YAML must not fail config load; treat as `off`
- logging must never panic or change runtime behavior
- if logging helper cannot format rare debug payload, skip that message rather than fail signal flow

## Testing

### Rust Unit Tests

Add tests for:

- `log_level` missing defaults to `off`
- valid string values parse to expected enum
- invalid string value falls back to `off`
- logger level comparison works in expected direction

Keep tests focused and local:

- parser tests in `break_engine.rs` or `logger.rs`
- helper behavior tests in `logger.rs`

### Manual `macOS` Validation

Set `log_level: debug` in runtime `config.yaml`, launch app from terminal, then run checklist:

1. active typing/mouse movement keeps `idle_active=false`
2. real inactivity flips `idle_active=true` after expected delay
3. foreign fullscreen app sets `fullscreen_active=true`
4. GazeGuard own fullscreen does not count as foreign fullscreen
5. if native fullscreen lookup fails, fallback output remains sane

Expected logs should clearly show:

- fallback window state
- native idle observation
- native fullscreen observation
- merged final signal
- engine apply result

## File Plan

Modify:

- `src-tauri/src/break_engine.rs`
- `src-tauri/src/desktop_signals.rs`
- `src-tauri/src/lib.rs`

Create:

- `src-tauri/src/logger.rs`

## Risks

### Config Surface Growth

Global `log_level` touches shared config consumed by multiple platforms. Mitigation: default to `off`, keep parser permissive, avoid behavior changes beyond logging.

### Log Noise

Desktop signal polling can be chatty. Mitigation: default `off`, use `debug` for normal validation, reserve `trace` for highly detailed emissions if needed.

### Validation Ambiguity

Logs can still be hard to interpret if mixed with unrelated app output. Mitigation: prefix messages consistently from `logger.rs` so desktop signal lines are easy to grep.

## Success Criteria

- `config.yaml` can enable logging via global `log_level`
- app remains quiet by default
- `macOS` desktop signal path emits useful debug logs when enabled
- logs are enough to distinguish native provider issue vs. engine-apply issue
- no scheduler or signal behavior changes besides observability
