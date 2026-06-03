# Why `http.sslVerify = false` is set in `.git/config`

This is a per-host decision for a development environment that sits behind
a corporate proxy or VPN which terminates TLS with an internal CA.  When
`http.sslVerify = true` (the default), git rejects the proxy's cert
because it is not in the system trust store, and every `git fetch` / `git
push` fails with `SSL certificate problem: unable to get local issuer
certificate`.

## Risks

- A man-in-the-middle attacker on the local network could inject fake
  objects into a `git pull` or steal credentials.  This is acceptable on
  a sealed dev box; **never enable this on a workstation that connects
  to untrusted Wi-Fi.**
- `cargo-audit` and similar tools that hard-code `rustls` ignore this
  setting and verify TLS with their own Mozilla root store (webpki-roots).
  So RustSec advisories remain trustworthy even with this off.

## How to undo (when behind a non-proxy network)

```bash
git config --local --unset http.sslVerify
```

That is the only override — no other config needs to change.

## What we tried that did not work

- Setting `http.sslCAInfo` to a custom CA bundle: the proxy's
  per-session cert is dynamically generated, so a static bundle does
  not match.
- Switching to `http.sslBackend = schannel`: Windows schannel does not
  include the corporate proxy's intermediate CA without a KB
  patch, and the patch is not yet deployed in our image.
- Running with a system-wide cert update: same as above.
