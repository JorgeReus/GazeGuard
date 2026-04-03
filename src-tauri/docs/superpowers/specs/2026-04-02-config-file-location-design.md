# Config File Location Design

## Goal

Replace the current generated `gen/.../defaults.yaml` runtime dependency with a real platform-specific config file that is created on first run and then loaded from disk on subsequent runs.

## Scope

This design covers:

- moving the canonical default YAML out of `gen/`
- resolving platform-specific config file paths
- bootstrapping the config file when it does not exist
- loading runtime configuration from that file on desktop and Android
- preserving the existing YAML schema and parser behavior

This design does not change the config schema itself.

## Current State

- Rust desktop code loads defaults through `include_str!("../gen/android/app/src/main/assets/config/defaults.yaml")`
- Android code separately reads `config/defaults.yaml` from packaged assets
- the generated Android asset acts as the source of truth for all platforms

This is the wrong dependency direction. Desktop runtime behavior should not depend on generated Android output.

## Requirements

- The app must use a real config file on every platform.
- If the config file is missing, the app must create it with the default YAML contents.
- Desktop config location must be:
  - Windows: `%AppData%/GazeGuard/config.yaml`
  - macOS: `~/.config/GazeGuard/config.yaml`
  - Linux: `~/.config/GazeGuard/config.yaml`
- Android config location must be an app-private real file under `filesDir`, specifically `filesDir/config/config.yaml`.
- The YAML format and current parsing semantics must remain unchanged.
- The canonical default YAML must live in a normal source-controlled location, not under `gen/`.

## Approaches Considered

### 1. Keep `gen/android/.../defaults.yaml` as the canonical template

Pros:

- minimal file movement
- small code delta

Cons:

- keeps desktop coupled to generated Android output
- keeps the source of truth in a generated tree

Rejected.

### 2. Use a source-controlled canonical YAML file and materialize it into platform config locations

Pros:

- one source of truth
- one config format across platforms
- user-editable real file everywhere
- removes dependency on generated output

Cons:

- requires small path/bootstrap layer on each platform

Chosen.

### 3. Use platform-native key-value storage on Android

Pros:

- straightforward for Android-only access

Cons:

- different storage model from desktop
- extra conversion layer
- harder to keep schema evolution coherent

Rejected.

## Proposed Design

### Canonical Default File

Move the default YAML into a non-generated source-controlled path owned by the app. A suitable location is a dedicated config asset path in the repo, for example:

- `resources/config/defaults.yaml`

The exact path can be adjusted during implementation, but it must not be inside `gen/`.

### Runtime Config Path Resolution

Create a small config bootstrap layer that resolves the runtime config path and ensures the file exists.

Desktop:

- Windows: `%AppData%/GazeGuard/config.yaml`
- macOS: `~/.config/GazeGuard/config.yaml`
- Linux: `~/.config/GazeGuard/config.yaml`

Android:

- `context.filesDir/config/config.yaml`

The implementation should create missing parent directories before creating the file.

### Bootstrap Behavior

On config load:

1. Resolve the platform-specific runtime path.
2. If the file does not exist:
   - create parent directories
   - write the canonical default YAML contents to the file
3. Read the file contents.
4. Parse with the existing YAML parser.

This makes the default config visible and editable after first run.

### Rust Changes

Rust should stop using `include_str!` against the generated Android asset.

Instead:

- load the canonical default YAML from its new source-controlled location
- resolve the desktop runtime config path
- ensure the file exists
- read YAML from disk
- parse using the existing `BreakEngineConfig::from_yaml`

To keep tests focused, path resolution and bootstrap logic should be separated from parse logic.

### Android Changes

Android should stop reading `config/defaults.yaml` from app assets as the runtime config source.

Instead:

- bundle the canonical default YAML with the app
- resolve `filesDir/config/config.yaml`
- create the file from the bundled default contents if it does not exist
- read and parse the file for schedule values

Android keeps using a real YAML file, not `SharedPreferences`.

## Error Handling

- If directory creation fails, surface the error rather than silently falling back to defaults.
- If file creation or file read fails, surface the error clearly.
- If YAML parsing fails, preserve the current failure behavior unless implementation context requires converting it into an app error.

Silent fallback after a user-edited invalid file is explicitly undesirable because it hides configuration problems.

## Testing

Add tests for:

- creating the config file when it is missing
- preserving an existing config file without overwriting it
- loading config from the runtime file path
- parsing behavior remaining unchanged after the move
- Android helper behavior for bootstrap path creation and file reuse where practical

Existing config parsing tests should remain valid with minimal adjustment.

## Implementation Notes

- Keep the config bootstrap code small and isolated from engine logic.
- Avoid embedding generated paths anywhere in runtime loading.
- If Tauri exposes app-specific directories that conflict with the explicit `~/.config` requirement on macOS, prefer the user-requested explicit path for this change.

## Success Criteria

- No runtime config load depends on `gen/android/.../defaults.yaml`.
- First run creates a real YAML config file in the requested OS-specific location.
- Later runs load that same file.
- Android uses `filesDir/config/config.yaml`.
- Current config schema and behavior remain intact.
