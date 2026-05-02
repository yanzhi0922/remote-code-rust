# Provenance

`remote-code-rust` is a clean-room rewrite. This file defines what sources are acceptable for analysis, design, and implementation.

The rule is simple: behavior can be studied broadly, but implementation must come from provenance-clear sources.

## Allowed Implementation Sources

The following sources are approved as implementation inputs:

- the local read-only reference workspace (the original TypeScript `remote-code` project)
- official Claude Code documentation and other official vendor documentation
- public open-source repositories with clear provenance and acceptable licensing
- original code written directly in this repository

These sources may be used to:

- define required behavior
- extract compatibility expectations
- design crate boundaries and process models
- create tests and fixtures
- implement new Rust code

## Allowed Reference-Only Sources

Some sources may be used only for behavior and product-surface comparison, not as implementation inputs.

Examples:

- public parity projects when they are useful as behavior inventories but not authoritative specifications
- writeups, blog posts, demos, and issue discussions about similar tools
- provenance-unclear repositories used only to confirm that a feature exists in the ecosystem

These sources must not be used for:

- direct code copying
- structural translation of modules or files
- deriving implementation details that cannot be justified from approved sources

## Disallowed Implementation Sources

The following categories are disallowed as implementation inputs:

- leaked source repositories
- code with unclear authorship or redistribution rights
- private code obtained outside approved repository access
- generated bundles that obscure origin and license

This project may mention such sources when explaining why they are excluded, but it must not derive implementation from them.

## Reference Workspace Policy

The original TypeScript `remote-code` workspace is treated as a read-only reference.

Allowed actions:

- inspect source
- inspect docs
- capture behavior
- collect fixtures
- compare compatibility outputs

Disallowed actions:

- editing files
- copying files wholesale into this repository
- using it as a submodule or vendored dependency
- introducing hidden coupling where the Rust workspace depends on the TypeScript workspace at runtime

## Fixture Collection Rules

Fixtures collected from the reference workspace are allowed because they capture externally observable behavior, not implementation.

Fixture collection must:

- record how the fixture was produced
- avoid embedding secret material
- avoid rewriting or mutating the source workspace
- store enough metadata to explain the scenario and source command

If a fixture cannot be reproduced from an approved source, it should not be treated as authoritative.

## Documentation Sources

When official documentation influences behavior or terminology, the repository should preserve traceability in commit messages, issues, or adjacent docs.

Expected examples:

- protocol decisions justified by official Claude Code documentation
- permission model choices justified by reference workspace behavior plus docs
- transport and API choices justified by public vendor protocol documentation

This project does not need heavy citation inside code, but major compatibility choices must remain explainable.

## Third-Party Crates

Rust dependencies must satisfy normal engineering standards:

- actively maintained or widely trusted
- acceptable license
- clear benefit over a simpler in-house implementation
- no unnecessary transitive surface for security-critical paths

Every new dependency should answer a concrete architectural need. Convenience alone is not enough in runtime-critical or security-sensitive crates.

## Contributor Expectations

Anyone writing code in this repository is expected to follow these rules:

- do not copy from provenance-unclear sources
- do not paste chunks from the reference workspace
- prefer deriving behavior from fixtures, docs, and direct reimplementation
- document any compatibility-sensitive behavior that would be hard to justify later

If the origin of a design or code fragment is unclear, it should be treated as blocked until the source can be justified.

## Enforcement

Provenance is enforced through process, not just policy text.

Expected enforcement mechanisms:

- code review scrutiny for suspiciously translated code
- fixture-backed tests for compatibility instead of copy-based implementation
- design docs that explain major behavior choices
- repository history that keeps generated or imported material separate from hand-written Rust code

The value of the rewrite depends on this boundary holding.
