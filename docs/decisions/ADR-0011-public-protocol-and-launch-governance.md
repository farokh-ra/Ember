# ADR-0011 — Public protocol publication and launch governance

- **Status:** Proposed for Stage 20 review
- **Date:** 2026-11-26
- **Deciders:** Ember project owner and maintainers (pending)
- **Supersedes:** none
- **Related:** ADR-0006, ADR-0007, [Protocol specification](../protocol/PROTOCOL_SPEC.md), [Governance](../governance/GOVERNANCE.md)

## Context

Ember has reached the final roadmap stage. The implementation, release pipeline,
and security model exist, but the project has not published a protocol
specification, a compatibility policy, a community infrastructure policy, or a
security disclosure process. It also has no production signing material, no
Android runtime evidence, no independent client, and no independent operator.

The project needs public documentation that lets an outside implementer or
operator evaluate Ember without implying that a public launch has happened. Two
failure modes are equally bad:

- keeping the protocol private, so no independent implementation can exist;
- publishing a launch narrative that hides missing evidence and turns a
  documentation milestone into a false production claim.

## Decision

1. Publish [PROTOCOL_SPEC.md](../protocol/PROTOCOL_SPEC.md) as the normative
   description of protocol v1. The Rust implementation is the reference
   implementation; a disagreement is a bug in one of them, not permission to
   ignore the specification.
2. Publish versioning, deprecation, interoperability, and known-limitations
   documents alongside the specification.
3. Treat the conformance suite as **self-conformance**. Do not claim independent
   client interoperability until a separately built client and a named test
   result exist.
4. Publish a community infrastructure directory policy, but do not publish a
   directory until independent operators can be verified. A listing is a hint,
   not an authority.
5. Keep network-health measurement local-only. Any remote reporting requires
   explicit opt-in, local preview, aggregation, suppression, a privacy review,
   and owner approval.
6. Keep governance, moderation, disclosure, and maintenance documents as
   proposals until the project owner approves them.
7. Do not describe a test-signed artifact as a public release. Do not set a
   launch date. The launch gate remains closed until the blockers in
   [KNOWN_LIMITATIONS.md](../release/KNOWN_LIMITATIONS.md) are resolved.
8. Require an owner decision for a public launch, a protocol major change, a
   signing-authority change, or a governance change.

## Consequences

- The protocol becomes a public compatibility commitment. Changes must follow
  the versioning policy and keep old vectors valid or introduce a new major.
- Published vectors are intentionally brittle. A changed vector is a protocol
  review event, not a routine test update.
- The project must maintain the specification, conformance suite, and known
  limitations with every release.
- The project cannot use "self-tested" as a substitute for independent
  interoperability in public claims.
- A future directory creates a moderation and availability obligation. The
  project must either meet it or publish a limited-preview scope.
- The launch gate remains visibly open. That is the intended outcome of this
  stage, not a failure to finish the documentation.
- Governance remains owner-controlled until a public amendment changes it.

## Alternatives considered

- **Keep the protocol internal.** Rejected: it prevents independent clients and
  operators and contradicts the Stage 20 deliverable.
- **Publish the protocol and claim interoperability from the unit tests.**
  Rejected: self-conformance is not independent interoperability.
- **Publish a directory of project-operated nodes as "community"
  infrastructure.** Rejected: it would centralize the network and misrepresent
  independence.
- **Enable telemetry by default to measure launch health.** Rejected: it
  violates the privacy model and the existing no-telemetry commitment.
- **Mark Stage 20 complete and launch.** Rejected: production signing, Android
  runtime evidence, SBOM/locking, third-party notices, independent
  interoperability, and owner approval are missing.

## Verification

- `cargo test -p nexus-protocol --test conformance` passes 8 tests.
- `cargo test --workspace` passes 380 tests.
- `.\scripts\build.ps1 -WithDependencyScan` passes.
- Public documents link to the implementation and to each other.
- No document claims a public release, independent interoperability, or public
  infrastructure that does not exist.
- The pull request remains unmerged until project-owner review.
