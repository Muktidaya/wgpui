# Nix builds

The flake uses the stable Nixpkgs 26.05 package set, Crane v0.24.0 and
flake-utils v1.0.0. `flake.lock` fixes their source revisions. Fenix is a
revision-pinned toolchain provider; the selected compiler is the explicit
Rust 1.95.0 release, with the official release manifest's SHA-256 fixed in
`flake.nix`. Updating Fenix does not select a newer compiler or a nightly
toolchain. The provider's unused Rust Analyzer input is not part of this flake.

Use Nix 2.31.2 or newer with `nix-command` and `flakes` enabled:

```sh
nix build
nix flake check
nix develop
```

The default derivation builds all workspace libraries with all features using
the workspace's release profile and runs the workspace's library and integration
tests using Cargo's test profile. Test compilation explicitly uses optimization
level zero, no link-time optimization and no debug information. Cargo compiles
one job at a time, and the Rust test harness runs one test at a time. These
settings apply to Nix validation; the workspace's release profile and normal
development-shell settings remain separate. A successful test run exercises
the test profile, while the release libraries are validated by compilation.

The derivation retains Cargo artifacts
in the result because WGPUI is a library, rather than installing an application
executable. The development shell provides the same Rust release and native
build inputs. The package version comes from `Cargo.toml`.

Linux builds and the development shell use a Fontconfig configuration backed
by the pinned Nixpkgs font packages: Liberation, DejaVu, Noto Sans CJK and Noto
Color Emoji. This supplies Latin, CJK and emoji coverage for native shaping
tests. Font discovery uses only this declared closure; user font directories
and host configuration files are excluded. Applications can still register
their own font assets through WGPUI's text-system API.

Linux checks and the development shell expose the native runtime libraries on
the loader path, including Fontconfig, FreeType, GLib/GObject, OpenSSL and zlib. Tests execute
before Nix fixes executable runtime search paths; having a library available
to the compiler or pkg-config alone does not make it discoverable at that stage.

To limit Nix-level build concurrency as well, use:

```sh
nix build --max-jobs 1 --cores 1
```

This serializes scheduled builds and their advertised core allowance. The
flake's Cargo settings limit compilation and test concurrency; they do not
impose a hard memory limit on an individual compiler process.

Source filtering retains embedded icons, fonts, theme data, grammar queries and
test fixtures, in addition to Rust sources and shaders. Build outputs, archived
upstream sources and environment files are excluded. When building from a Git
checkout, newly added source files must be tracked so Nix includes them in the
flake's source snapshot.

The declared systems are `aarch64-linux`, `x86_64-linux`, `aarch64-darwin` and
`x86_64-darwin`. Evaluation of a system's derivation does not establish that it
builds or runs on that system. Linux native interaction additionally requires a
working display server, desktop portal and graphics driver; headless tests do
not verify those services.

Nixpkgs 26.05 is the final Nixpkgs release supporting `x86_64-darwin`. The
current pin preserves that output; a future Nixpkgs update must explicitly
resolve Intel macOS support. Crane's own flake outputs are not used: WGPUI calls
`crane.mkLib` with its selected package set.

The Nix Darwin build and shell explicitly request a macOS 11.0 deployment
target, consistent with the existing Nix SDK hook and Apple Silicon's platform
floor. Ordinary Cargo builds retain their separate 10.15.7 setting in
`.cargo/config.toml`. Neither setting establishes runtime acceptance on the
oldest macOS version. Keep native verification of each supported architecture
and operating-system version separate from a successful Nix evaluation or
build.

Evaluate the package, check and development-shell derivations for every
declared system without building them with:

```sh
nix flake check --no-build --all-systems
```

This checks evaluation of the selected source snapshot. Build and runtime
acceptance require separate verification on each target system.
