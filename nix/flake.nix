{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      supportedSystems =
        [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      dependencies = pkgs: {
        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs;
          [ pkg-config ] ++ lib.optionals (isDarwin pkgs.system)
          [ darwin.apple_sdk.frameworks.SystemConfiguration ];
      };

      isDarwin = system: nixpkgs.lib.strings.hasSuffix "-darwin" system;
      forEachSupportedSystem = f:
        nixpkgs.lib.genAttrs supportedSystems (system:
          f {
            pkgs = import nixpkgs {
              inherit system;
              overlays =
                [ rust-overlay.overlays.default self.overlays.default ];
            };
          });
    in {
      overlays.default = _: prev: {
        rustToolchain = let rust = prev.rust-bin;
        in if builtins.pathExists ./rust-toolchain.toml then
          rust.fromRustupToolchainFile ./rust-toolchain.toml
        else if builtins.pathExists ./rust-toolchain then
          rust.fromRustupToolchainFile ./rust-toolchain
        else
          rust.stable.latest.default.override {
            extensions = [ "rust-src" "rustfmt" ];
          };
      };

      packages = forEachSupportedSystem ({ pkgs }:
        let
          protonvpn-rs = pkgs.callPackage ./package.nix {
            inherit (dependencies pkgs) buildInputs nativeBuildInputs;
          };
        in {
          default = protonvpn-rs;
          inherit protonvpn-rs;
        });

      devShells = forEachSupportedSystem ({ pkgs }:
        let inherit (pkgs) mkShell;
        in {
          default = mkShell {
            packages = with pkgs; [
              rustToolchain
              cargo-deny
              cargo-edit
              cargo-watch
              rust-analyzer

              # tools
              nixfmt
              deadnix
              statix
              markdownlint-cli
              nodePackages.prettier
            ];

            inherit (dependencies pkgs) buildInputs nativeBuildInputs;

            env = {
              RUST_SRC_PATH =
                "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        });

      nixosModules.protonvpn = { pkgs, ... }: {
        imports = [
          (import ./module.nix {
            inherit (self.packages.${pkgs.system}) protonvpn-rs;
          })
        ];
      };
    };
}
