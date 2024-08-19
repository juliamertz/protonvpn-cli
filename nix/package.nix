{ lib, openssl, pkg-config, makeRustPlatform, rust-bin, ... }:
let
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config ];

  rustPlatform = makeRustPlatform {
    cargo = rust-bin.stable.latest.minimal;
    rustc = rust-bin.stable.latest.minimal;
  };

  manifest = (lib.importTOML ../Cargo.toml).package;

in rustPlatform.buildRustPackage rec {
  inherit buildInputs nativeBuildInputs;

  src = ../.;
  inherit (manifest) name version;

  postInstall = # sh
    "ln -sf $out/bin/${manifest.name} $out/bin/pvpn";

  cargoLock = {
    lockFile = ../Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  meta.mainProgram = manifest.name;
}
