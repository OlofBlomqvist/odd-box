{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, fenix, nixpkgs, flake-utils,... }@inputs:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs { inherit system; };
      rustToolchain = fenix.packages.${system}.toolchainOf {
        channel = "nightly";
        date = "2024-03-04";
        sha256 = "AhaXmpuEQKbeHbG5tB/UamfItWiidsEWfKQfbKTKH1Y=";
      };
    in {

      devShell = pkgs.mkShell {
        nativeBuildInputs = [
          rustToolchain.cargo
          rustToolchain.rustc          
          pkgs.openssl
          pkgs.pkg-config
        ];
      };
    });
}