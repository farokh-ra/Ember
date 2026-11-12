# NEXUS Logging Policy

NEXUS logs must help diagnose local failures without exposing the user's
identity, credentials, files, or social graph. This policy applies to Kotlin,
Rust, build scripts, tests, CI output, and bug reports.

Stage 03 establishes the policy and the redaction helpers. File logging, log
rotation, and user-facing log export arrive in later stages; the same rules apply
when they do.

## 1. Principles

1. **Collect the minimum.** Log the event, state transition, error code, and
   non-sensitive counts. Do not log payloads, file contents, key material, or
   personal data.
2. **Redact before formatting.** Pass values through a redaction helper before
   interpolation, not after the message is built.
3. **Debug is opt-in.** Debug and trace logging are disabled by default. Info
   logging must be safe to attach to a public bug report.
4. **Never log secrets, even at trace level.** Developers may enable verbose
   diagnostics for a local reproduction, but secret values remain forbidden.
5. **Treat identifiers as sensitive.** Peer IDs, identity IDs, IP addresses, and
   file names can identify a user or their social graph. Log them only when
   necessary, at debug level, and redact them in user-shared output.
6. **Fail closed.** If a value cannot be classified, omit it rather than guess.

## 2. Data classification

| Class | Examples | Logging rule |
|---|---|---|
| Secret | Private keys, seed phrases, passwords, passphrases, tokens, signing material, keystore passwords, API keys | Never log. Never include in errors, crash reports, or test snapshots. |
| Sensitive personal | File names, file paths, peer IDs, identity IDs, IP addresses, contact or profile data | Debug only, minimized, redacted in shared output. |
| Operational | Core state, error codes, queue depth, byte counts, duration, protocol version, ABI version | Allowed at info or debug with bounded values. |
| Public build data | App version, commit, toolchain version, platform | Allowed. |

## 3. Levels

| Level | Use |
|---|---|
| `error` | A user-visible operation failed and requires attention. Include a stable error code and a safe message. |
| `warn` | Degraded behavior, retry, fallback, or a condition worth investigating. Include a stable reason. |
| `info` | Lifecycle transitions and high-level operation outcomes. Safe for a bug report. |
| `debug` | Diagnostic detail for local development. Opt-in. Redact identifiers. |
| `trace` | Fine-grained flow tracing for a specific investigation. Opt-in. Redact identifiers. |

Avoid logging inside tight loops. Aggregate counts and emit one summary.

## 4. Kotlin rules

The shared module provides `io.nexus.model.Redaction`:

```kotlin
import io.nexus.model.Redaction

// Redact a value when the field name conventionally carries secret material.
val safeField = Redaction.field("access_token", token)

// Redact credential-shaped text before it reaches a logger or UI error.
val safeMessage = Redaction.text("request failed: password=$password")
```

Rules:

- Use `Redaction.field` for structured fields and `Redaction.text` for free-form
  strings.
- Do not build a message with a secret and then try to remove it. Redact first.
- Do not add a logging library that phones home, batches analytics, or uploads
  crash reports.
- Android shell logging must not include intents, URIs, file paths, or account
  data from the platform.
- Compose previews and UI tests must use fake data, never screenshots or logs
  from a real identity.

## 5. Rust rules

The `nexus-observability` crate owns global logger initialization and exposes
redaction helpers:

```rust
use nexus_observability::{field, text};

tracing::info!(
    event = "core_started",
    abi_version = nexus_core::ABI_VERSION,
    "{}",
    field("peer_id", peer_id)
);

let safe_message = text("request failed: password=hunter2");
tracing::warn!(event = "request_failed", "{safe_message}");
```

Rules:

- Call `nexus_observability::init(debug)` once per process. Do not initialize a
  second global subscriber.
- Use structured fields rather than interpolating values into the message.
- Use `field` for values named like secrets and `text` for free-form messages.
- Do not log raw `Debug` output for structs that may contain secrets. Implement a
  redacted representation or log selected fields.
- Do not log cryptographic material, even in tests. Use fixed public test
  vectors and obviously fake values.
- The `nexus-ffi` boundary logs error codes, not pointer values or caller
  buffers.

## 6. Build, CI, and test output

- Build scripts must not print environment variables wholesale. Print the tool
  version, not the environment.
- CI logs are public on open-source repositories. Treat every line as public.
- Do not echo `local.properties`, keystore paths, signing passwords, tokens, or
  full command lines that contain them.
- Test names and fixtures must not contain real user data or real credentials.
- If a failing command prints a secret, rotate the secret and fix the command
  before re-running in public CI.

## 7. User-facing errors

Errors shown in the UI or returned across the FFI boundary contain:

- a stable error code,
- a short, actionable message,
- an optional correlation ID.

They do not contain stack traces, SQL, file paths, peer addresses, or raw
exception text. Detailed diagnostics stay in local debug logs behind the
redaction rules.

Example:

```text
NEXUS-CORE-0042: The selected file could not be read. Check that it still exists and try again.
```

## 8. Log destinations and retention

Stage 03 writes diagnostics to standard error through `tracing-subscriber` and
does not create log files. Later stages may add local rotating files. When they
do:

- Store logs in the application's local data directory, never in the repository.
- Default to info level and a bounded size.
- Do not upload logs automatically.
- Provide an explicit, user-initiated export that applies redaction.
- Delete logs on uninstall when the user chooses to remove application data.

## 9. Incident response

If a secret is committed, logged, or published:

1. Revoke and rotate the secret immediately. Removing the commit is not enough.
2. Remove the value from the current tree and, where practical, from history.
3. Assume public CI logs and release artifacts are already exposed.
4. Record the incident and the corrective action in the stage or release notes
   without repeating the secret.
5. Add or strengthen a hygiene check so the same class of leak fails CI.

## 10. Verification

The redaction helpers are tested in:

- `shared/model/src/jvmTest/kotlin/io/nexus/model/RedactionTest.kt`
- `crates/nexus-observability/src/lib.rs`

The repository hygiene script also scans tracked and untracked non-ignored text
files for private key blocks, common provider tokens, and long credential
assignments. Tests use short fake values that do not match production token
shapes.
