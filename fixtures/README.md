# Reference Fixtures

This directory holds read-only compatibility fixtures captured from the legacy
TypeScript `remote-code` workspace (external reference, not included in this repo).

Rules:

- Never modify the source workspace while collecting fixtures.
- Regenerate captures with `scripts/collect_reference_fixtures.py` or
  `scripts/collect_reference_fixtures.ps1`.
- Keep generated artifacts deterministic where possible; dynamic fields should
  be normalized in tests rather than edited by hand.