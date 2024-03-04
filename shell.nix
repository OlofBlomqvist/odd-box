{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (builtins.fetchTarball "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz"))
    ];
  }
}:

with pkgs;

mkShell {
  nativeBuildInputs = [
    latest.rustChannels.nightly.rust
    pkg-config
    openssl
  ];
}
