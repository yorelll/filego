# Dependency and license policy

QuickFolder source code is MIT-licensed. Every dependency must be reviewed before it is added, pinned in `Cargo.lock`, and distributable with the resulting Windows application.

## Automated gate

CI downloads pinned, checksummed release binaries for `cargo-deny` and `cargo-about` through an immutable `taiki-e/install-action` commit with source-build fallback disabled. It then runs:

```text
cargo deny --locked check advisories bans licenses sources
```

`deny.toml` denies wildcards and unknown registries/Git sources. The allow-list contains permissive licenses used by the Windows x86-64 release dependency graph plus Slint's desktop royalty-free license reference. The audit explicitly targets `x86_64-pc-windows-msvc`: Cargo.lock can retain platform-conditional crates for other systems, but their presence is not evidence that they ship in the only supported `0.0.1` artifact. Adding another release target requires extending this target list and independently re-auditing its graph. A changed dependency graph must pass this gate and receive independent review.

## Slint licensing decision

QuickFolder uses Slint 1.18.0 under `LicenseRef-Slint-Royalty-free-2.0` for a desktop application. The project satisfies both available attribution paths: the public README shows the Slint attribution badge, and the application embeds Slint's `AboutSlint` widget. The production settings/about page must keep that widget accessible from the top-level tray menu before release. Removing either path requires an explicit compliance review; at least one valid path is mandatory.

Slint's exact v1.18.0 royalty-free license text is vendored at `third-party/slint/LicenseRef-Slint-Royalty-free-2.0.md` and copied into every portable package. Other dependency notices are generated into `THIRD_PARTY_LICENSES.html`. A build missing either notice source is suitable only as a CI diagnostic artifact, never as a release candidate.

## Inventory generation

CI uses the pinned `cargo-about` release and `about.toml`/`about.hbs` to render `THIRD_PARTY_LICENSES.html` with `--locked --fail --all-features --target x86_64-pc-windows-msvc` so the inventory reflects the shipped Windows x86-64 artifact's resolution. The ordered acceptance list places Slint's royalty-free license first so `OR` expressions select the project's documented desktop-license basis. The generator must resolve the checked-in `Cargo.lock`; it must not silently update dependencies. Resolution diagnostics fail the job where supported; CI sanity-checks only stable HTML markers (title and non-empty content; no unreliable slice-literal substring assertions like `slint`), and CI/independent review must additionally verify that every package in the configured Windows x86-64 release dependency graph appears with usable notice text because tool limitations can still report some missing metadata as warnings. Generated inventory is uploaded with CI artifacts and reviewed before a release.

## Human checks

Automated SPDX detection is not legal advice. Reviewers must check:

- newly introduced custom, copyleft, source-available, or unknown licenses;
- attribution, NOTICE, source-offer, and redistribution obligations;
- embedded fonts, icons, native binaries, and other non-Cargo assets;
- duplicate native libraries and unmaintained dependencies;
- whether a dependency introduces telemetry, networking, or file-system behavior outside the product scope.
