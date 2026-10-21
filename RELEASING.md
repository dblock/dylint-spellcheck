# Releasing

Releases are Git tags and GitHub releases, consumed through Dylint's Git configuration. This crate has `publish = false`: do not run `cargo publish`. There is no release workflow or binary distribution.

## 1. Decide the version number

Use [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`. Use a patch for fixes, a minor for backward-compatible features, and a major for breaking changes. Before 1.0, breaking changes may use a minor bump; describe them explicitly in the changelog. The first release is `0.1.0`, matching the current manifest.

Consider compatibility changes to configuration, checked content, dictionaries, and the pinned compiler.

## 2. Prepare the changelog and manifest

Start from current `main` with a clean working tree and create a release branch:

```sh
git switch main
git pull --ff-only origin main
git switch -c release-v0.1.0
```

If `[Unreleased]` is empty, stop: there is nothing to release. Rename it to the version and today's date, for example `## [0.1.0] - YYYY-MM-DD`, then add a fresh empty `[Unreleased]` section above it. Preserve the linked PRs and contributors.

Add link references at the bottom of `CHANGELOG.md`:

```markdown
[Unreleased]: https://github.com/dblock/dylint-spellcheck/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dblock/dylint-spellcheck/releases/tag/v0.1.0
```

For later releases, link the version to a comparison from the previous tag and update the existing `[Unreleased]` reference.

Set `package.version` in `Cargo.toml`, then run `cargo check` to update `Cargo.lock`. The first release's version may already match. Avoid unrelated dependency/toolchain updates.

## 3. Verify locally

Follow [CONTRIBUTING.md](CONTRIBUTING.md#development-setup) first. Run from the repository root using the pinned nightly toolchain:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
RUSTFLAGS="-D warnings" cargo dylint --all -- --all-targets --all-features --locked
cargo test --locked
```

Review changed UI diagnostics and dictionary license/provenance notices.

## 4. Open and merge the release PR

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release v0.1.0"
git push -u origin release-v0.1.0
gh pr create --base main --title "Release v0.1.0" --body "Prepare v0.1.0; see CHANGELOG.md for release notes."
```

Wait for all PR checks, then squash-merge the release PR. Update local `main` and confirm the Test, Lint, and Coverage workflows for the exact release commit pass before tagging:

```sh
git switch main
git pull --ff-only origin main
git log -1 --oneline
gh run list --branch main --commit "$(git rev-parse HEAD)"
```

Verify this is the merged release commit and its version/changelog match. Do not tag unrelated changes that landed afterward.

## 5. Tag and create the GitHub release

Substitute the chosen version throughout:

```sh
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
gh release create v0.1.0 --verify-tag --title "v0.1.0" --generate-notes
```

Review generated notes against the changelog, including compatibility changes and the required toolchain. GitHub provides source archives automatically.

## 6. Verify consumption

```sh
gh release view v0.1.0
git ls-remote origin refs/tags/v0.1.0 'refs/tags/v0.1.0^{}'
```

In a separate representative consumer project, configure the released tag:

```toml
[workspace.metadata.dylint]
libraries = [
    { git = "https://github.com/dblock/dylint-spellcheck", tag = "v0.1.0" },
]
```

Run `cargo dylint --all -- --workspace --all-targets --all-features` and confirm the library builds, dictionary resources load, and expected spelling diagnostics appear. Consumers may instead pin `rev` to the release commit for an immutable dependency.

Never force-push release tags or rewrite released history. If a published release is wrong, ship a new patch version.
