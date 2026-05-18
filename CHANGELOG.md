# Changelog

## Unreleased

- Hardened remote transport defaults: relay-only web clients unless direct
  runner mode is explicitly enabled.
- Added pinned self-signed TLS enforcement and real TLS signature verification
  for remote transport.
- Completed QUIC command framing, frame-size limits, typed approval decisions,
  and parity with HTTP approval relay/commit/publish behavior.
- Fixed Windows test stack limits for Codex CLI/app-server binaries and reduced
  test debug artifact pressure.
- Isolated app-server integration tests from host home directories and disabled
  unrelated Apps MCP startup in account auth-refresh tests.
- Added release workflow support for Web/PWA relay artifacts and Windows NSIS
  desktop installer artifacts.
- Added one-line relay installer and Windows installer helper scripts.
- Added English/Chinese quick-start docs, security policy, contribution guide,
  and root license notice.
