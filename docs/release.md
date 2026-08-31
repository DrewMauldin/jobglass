# Release process

JobGlass releases separate reviewed source, hosted checks, packages, signing, and live documentation into independent gates.

## Prepare

1. Update the changelog and ensure every manifest reports the intended semantic version.
2. Run `node scripts/check-version.mjs vX.Y.Z` and `npm run quality -- full` from a clean checkout.
3. Build and inspect the native package on at least the primary macOS runtime.
4. Record accessibility, responsive, performance, package, audit, and signing evidence in `docs/verification/vX.Y.Z.md`.
5. Obtain fresh-context correctness, security, UX/accessibility, tests, packaging, and maintainability review.

## Merge and tag

Merge through a reviewed pull request without force-pushing `main`. Re-read the merge commit, then create an annotated `vX.Y.Z` tag on that exact commit and push it.

## Automated release

The tag workflow:

- verifies tag/manifests parity;
- requires an annotated tag whose commit is reachable from `origin/main`;
- reruns frontend, browser, Rust, coverage, audit, secret, workflow, and benchmark gates;
- retests and builds unsigned macOS, Linux, and Windows packages;
- creates a CycloneDX SBOM and SHA-256 checksums;
- records GitHub build provenance;
- creates a draft release, re-downloads all assets, and verifies checksums;
- publishes only after verification succeeds.

The workflow may replace assets only while a release remains a draft. It fails rather than mutate an already published release.

If a mutation times out, inspect the exact tag, release, and assets before retrying. Never assume an unknown remote response did or did not commit.

## Signing status

Unsigned packages may be published only with prominent installation warnings. This is a release-review state, not signed-production completion. A future signed release must separately prove certificate identity, hardened runtime where applicable, notarisation, and platform trust checks.

## Documentation and rollback

The Pages workflow publishes the static `site/` source from `main`. Verify HTTPS, primary routes, image assets, and release links after deployment.

A source rollback reverts the merge with a new commit. A release is immutable: do not move a published tag. When an artifact is unsafe, mark the release and documentation clearly, publish a corrected patch version, and retain the original verification record.
