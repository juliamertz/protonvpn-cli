{ lib, makeRustPlatform, rust-bin, buildInputs, nativeBuildInputs, ... }:
let
  manifest = (lib.importTOML ../Cargo.toml).package;
  rustPlatform = makeRustPlatform {
    cargo = rust-bin.stable.latest.minimal;
    rustc = rust-bin.stable.latest.minimal;
  };
in rustPlatform.buildRustPackage rec {
  inherit buildInputs nativeBuildInputs;
  inherit (manifest) name version;

  src = ../.;

  postInstall = # sh
    "ln -sf $out/bin/${manifest.name} $out/bin/pvpn";

  cargoLock = {
    lockFile = ../Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  meta.mainProgram = manifest.name;
}
