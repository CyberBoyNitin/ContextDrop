## Summary

Describe the problem and the change in a few sentences.

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Compression or protection-rule change
- [ ] API or dashboard contract change
- [ ] Documentation
- [ ] Security hardening
- [ ] Build, CI, or dependency update

## Verification

List the exact commands you ran and their actual results.

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd web && npm run typecheck && npm run build
```

For compression or protection changes, include benchmark results before and after:

```text
Evidence checks:
Aggregate bytes/tokens:
Per-fixture regressions:
```

For dashboard changes, include a screenshot or short recording of the real loading, success, empty, error, and fallback states you exercised.

## Safety review

- [ ] The core library remains free of file, network, process, and model I/O.
- [ ] Behavior remains deterministic for the same input and configuration.
- [ ] Protected evidence and adjacent context are still validated.
- [ ] A failed validation or non-reducing candidate still returns the original.
- [ ] User preserve patterns remain literal and are never executed.
- [ ] No telemetry, content logging, remote fonts, or new outbound calls were added.
- [ ] Loopback binding, Host/Origin checks, CORS, JSON-only POST, and size limits remain intact.
- [ ] The CLI still refuses accidental output overwrites.
- [ ] Dashboard states reflect real API responses with no placeholder data or dead controls.

## Documentation

- [ ] README updated for user-visible behavior or verified results.
- [ ] Architecture, privacy, threat model, security, benchmark, or competitive docs updated where needed.
- [ ] API changes are mirrored in `web/src/types.ts`.
- [ ] No unverified test, audit, benchmark, or security claims were added.

## Related issues

Closes #
