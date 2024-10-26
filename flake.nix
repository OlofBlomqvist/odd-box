{
  description = "A Rust application using the nightly compiler";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  
  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustNightly = pkgs.rustChannelOf {
          channel = "stable";
        };

      in {

        packages.default = pkgs.rustPlatform.buildRustPackage rec {

            pname = "odd-box";
            version = "0.1.8";
            src = ./.;

            cargoLock = {
                lockFile = ./Cargo.lock;
            };

            meta = with pkgs.lib; {
                description = "dead simple reverse-proxy";
                homepage = "https://github.com/OlofBlomqvist/odd-box";
                license = licenses.mit;
                maintainers = ["olof@twnet.se"];
            };

            buildType = "release";
            buildInputs = [
              pkgs.openssl
              pkgs.pkg-config
              pkgs.vscode-extensions.rust-lang.rust-analyzer
              
            ];

            RUSTC = "${rustNightly.default}/bin/rustc";
            CARGO = "${rustNightly.default}/bin/cargo";

            nativeBuildInputs = [ rustNightly.default ];

            OPENSSL_DIR = "${pkgs.openssl.dev}";
            OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";

            installPhase = ''
                mkdir -p $out/bin
                cp target/*/release/odd-box $out/bin/
            '';

        };

        devShell = pkgs.mkShell {
            nativeBuildInputs = [
                rustNightly.default         
                pkgs.openssl
                pkgs.pkg-config
                pkgs.vscode-extensions.rust-lang.rust-analyzer
            ];
        };
      
      }

    );
}
