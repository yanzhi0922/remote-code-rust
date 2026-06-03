# `probe_provider_model` error UX — TODO guide

The SettingsPanel `handleProbeModel` function (in
`apps/remote-code-gui/src/components/layout/SettingsPanel.tsx`)
currently has a `try { ... } catch (err) { void err; throw err; }`
scaffolding with a TODO marker.  This document explains the design
choices so the implementer can make an informed call.

## What the backend can return

`probe_provider_model` in
`apps/remote-code-gui/src-tauri/src/desktop/provider_commands.rs`
returns `Result<GuiProbeModelResultDto, String>`.  The `Err` cases
that are realistic in production:

| Message fragment | Cause | User impact |
|---|---|---|
| `probe rate-limited: wait 600 ms between probes` | 1-s minimum interval gate tripped (the most common error) | User clicked the Plug icon twice in a row |
| `model id cannot be empty` | FE sent `""` as the model id | Should not reach the user — this is a FE bug |
| `unknown provider config: <name>` | FE cached a stale provider name | Should not reach the user — same |
| `failed to build HTTP client: <details>` | reqwest builder failure (rare) | Network stack is broken; show a generic error |
| `provider has no <protocol> endpoint configured` | Provider saved without a base URL | User added a provider but didn't fill in the URL field |

## Three design axes

### 1. Surface — toast vs error store vs inline

```ts
// A) Synchronous toast (simplest)
import { useToast } from '@/hooks/useToast';
const toast = useToast();
catch (err) {
  toast.error(String(err));
  throw err;
}
```

```ts
// B) Global error store (lets other components subscribe)
const setLastError = useAppStore((s) => s.setLastError);
catch (err) {
  setLastError({ kind: 'probe', message: String(err) });
  throw err;
}
```

```ts
// C) Inline per-row chip in ProviderList
catch (err) {
  setRowError(modelId, String(err));
  // Do NOT re-throw; the row chip carries the message
  return undefined;
}
```

**Recommendation**: A (toast) for v1.  The error is global to the
probe subsystem, not per-row — only one model can be probed at a
time, so a per-row chip would show "0 errors" on every other row.

### 2. Coalescing — one toast per second

The 1-s rate-limit gate will fire 5+ toasts in 5 seconds if the
user spam-clicks.  The simplest mitigation: ignore identical toast
content for 2 seconds.

```ts
let lastToastAt = 0;
let lastToastMsg = '';
function showCoalescedToast(msg: string) {
  const now = Date.now();
  if (msg === lastToastMsg && now - lastToastAt < 2000) return;
  lastToastMsg = msg;
  lastToastAt = now;
  toast.error(msg);
}
```

### 3. Telemetry — should the backend see this too?

The `recordFrontendLog` Tauri command (in
`apps/remote-code-gui/src/lib/tauri.ts`) lets the FE write to the
backend tracing layer.  Calling it on probe errors is useful for
debugging user-reported issues, but it is **optional** — the
backend already records the Rust-side error in its own log before
returning the `Err` to the FE.

```ts
catch (err) {
  void recordFrontendLog({ level: 'warn', message: `probe failed: ${err}` });
  toast.error(String(err));
  throw err;
}
```

## i18n

The current Rust error messages are English-only.  If the project
ships with i18n, you have two options:

1. **Translate at the FE** (recommended): keep the Rust messages
   English, map them to a translation key in the FE.  The mapping
   table lives in `apps/remote-code-gui/src/i18n/locales/en.json`
   under `settings.probeErrors.<key>`.
2. **Localize in Rust**: the GUI command could load the user's
   locale from the settings store and substitute the message.  More
   work, and it splits the source of truth for the error string.

## What NOT to do

- Do **not** swallow the error.  The `throw err;` at the bottom of
  the catch is intentional — it lets React's error boundary pick up
  the failure if the consumer does not handle it.
- Do **not** log the API key in the toast or the `recordFrontendLog`
  payload.  The Rust side already wipes the `SecretString` before
  returning, so `String(err)` should never contain the key, but
  defense in depth: never log the entire error object blindly; pull
  out just the user-facing message.
- Do **not** retry automatically.  A retry on rate-limit would
  amplify the very behaviour the gate is trying to prevent.  The
  user can click the button again after the toast fades.
