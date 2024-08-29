{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, self, ... }@inputs:
    let
      supportedSystems =
        nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ];
      overlays = [ (import inputs.rust-overlay) ];
    in {
      packages = supportedSystems (system:
        let
          pkgs = import inputs.nixpkgs { inherit system overlays; };
          protonvpn-rs = pkgs.callPackage ./package.nix { };
        in {
          default = protonvpn-rs;
          inherit protonvpn-rs;
        });

      devShell = supportedSystems (system:
        let
          pkgs = import inputs.nixpkgs { inherit system overlays; };
          tools = with pkgs; [
            nixfmt
            deadnix
            statix
            markdownlint-cli
            nodePackages.prettier
          ];
        in pkgs.mkShell {
          name = "protonvpn-rs-shell";
          nativeBuildInputs = with pkgs; [ pkg-config ];

          buildInputs = with pkgs;
            [ openssl openvpn ] ++ tools ++ (with pkgs.rust-bin; [
              (stable.latest.minimal.override {
                extensions = [ "clippy" "rust-src" ];
              })

              nightly.latest.rustfmt
              nightly.latest.rust-analyzer
            ]);
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

