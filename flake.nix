{
  description = "Standalone WGPUI build";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    fenix = {
      url = "github:nix-community/fenix/dec0a3fee20fbe567c75362e1a527387b3d4a2c3";
      flake = false;
    };
    crane.url = "github:ipetkov/crane/v0.24.0";
    flake-utils.url = "github:numtide/flake-utils/v1.0.0";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
      flake-utils,
    }:
    flake-utils.lib.eachSystem [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ] (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;

        manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        rustToolchains = import fenix { inherit pkgs system; };
        rustManifest = builtins.fetchurl {
          url = "https://static.rust-lang.org/dist/channel-rust-1.95.0.toml";
          sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
        };
        toolchain = (rustToolchains.fromManifestFile rustManifest).withComponents [
          "cargo"
          "rustc"
          "rust-src"
          "rustfmt"
          "clippy"
        ];

        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        src = lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              name = builtins.baseNameOf path;
            in
            # Embedded assets, queries, themes, and test fixtures are build inputs too.
            lib.cleanSourceFilter path type
            && !(lib.elem name [
              ".agents"
              ".github"
              ".netrc"
              ".perf-runs"
              "AGENTS.md"
              "STATUS.md"
              "cargo-target"
              "flake.lock"
              "flake.nix"
              "result"
              "target"
              "upstream-kit"
            ])
            && !(lib.hasPrefix ".env" name);
        };

        linuxLibs = with pkgs; [
          alsa-lib
          libdrm
          mesa # provides libgbm
          libxkbcommon
          libva
          vulkan-loader
          wayland
          libx11
          libxcb
          libxcursor
          libxi
          libxinerama
          libxrandr
          pango
          cairo
          harfbuzz
        ];

        # Tests run before executable RPATH fixup, so direct runtime libraries must be discoverable.
        linuxRuntimeLibs = linuxLibs ++ (with pkgs; [
          fontconfig
          freetype
          glib
          openssl
          zlib
        ]);

        linuxFonts = with pkgs; [
          liberation_ttf
          dejavu_fonts
          noto-fonts-cjk-sans
          noto-fonts-color-emoji
        ];
        linuxFontConfig =
          let
            config = pkgs.makeFontsConf {
              fontDirectories = linuxFonts;
              impureFontDirectories = [ ];
              includes = [ ];
            };
          in
          pkgs.runCommand "wgpui-fonts.conf" { } ''
            # makeFontsConf adds XDG discovery even with no impure directories.
            substitute ${config} "$out" \
              --replace-fail '<dir prefix="xdg">fonts</dir>' ""
          '';

        commonArgs = {
          pname = "wgpui";
          version = manifest.package.version;

          inherit src;
          strictDeps = true;

          nativeBuildInputs = with pkgs; [
            cmake
            pkg-config
            rustPlatform.bindgenHook
          ] ++ lib.optionals stdenv.isLinux [ lld ];

          buildInputs =
            with pkgs;
            [
              fontconfig
              freetype
              openssl
              zlib
            ]
            ++ lib.optionals stdenv.isLinux (linuxLibs ++ linuxFonts)
            ++ lib.optionals stdenv.isDarwin [
              apple-sdk_15
              (darwinMinVersionHook "11.0")
            ];

          env = {
            CARGO_BUILD_JOBS = "1";
            RUST_TEST_THREADS = "1";
            CARGO_PROFILE_TEST_OPT_LEVEL = "0";
            CARGO_PROFILE_TEST_LTO = "false";
            CARGO_PROFILE_TEST_DEBUG = "0";
          } // lib.optionalAttrs pkgs.stdenv.isLinux {
            LD_LIBRARY_PATH = lib.makeLibraryPath linuxRuntimeLibs;
            FONTCONFIG_FILE = linuxFontConfig;
          } // lib.optionalAttrs pkgs.stdenv.isDarwin {
            MACOSX_DEPLOYMENT_TARGET = "11.0";
          };

          cargoExtraArgs = "--locked --workspace --all-features";
          cargoBuildExtraArgs = "--lib";
          cargoTestCommand = "cargo test --profile test";
          cargoTestExtraArgs = "--lib --tests";
        };

        cargoArtifacts = craneLib.buildDepsOnly (
          commonArgs // {
            cargoTestExtraArgs = "--no-run ${commonArgs.cargoTestExtraArgs}";
          }
        );

        wgpui = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doInstallCargoArtifacts = true;
          }
        );
      in
      {
        packages.default = wgpui;
        checks.workspace = wgpui;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ wgpui ];
          packages = [ toolchain ];
          env = lib.optionalAttrs pkgs.stdenv.isDarwin {
            inherit (commonArgs.env) MACOSX_DEPLOYMENT_TARGET;
          } // lib.optionalAttrs pkgs.stdenv.isLinux {
            inherit (commonArgs.env) FONTCONFIG_FILE;
          };

          shellHook = ''
            export RUST_BACKTRACE=1
            export RUST_SRC_PATH="${toolchain}/lib/rustlib/src/rust/library"
            ${lib.optionalString pkgs.stdenv.isLinux ''
              export LD_LIBRARY_PATH="${lib.makeLibraryPath linuxRuntimeLibs}:$LD_LIBRARY_PATH"
            ''}
          '';
        };
      }
    );
}
