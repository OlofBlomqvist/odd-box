{
  description = "app";

  inputs = {
    flakeutils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flakeutils }:
    flakeutils.lib.eachDefaultSystem (system:
      let
        NAME = "app";
        VERSION = "0.1";

        pkgs = import nixpkgs {
          inherit system;
        };

      in
      rec {

        packages.${NAME} = pkgs.stdenv.mkDerivation {
          pname = NAME;
          version = VERSION;

          buildPhase = "echo 'no-build'";
        };

        defaultPackage = packages.${NAME};

        # For `nix run`.
        apps.${NAME} = flakeutils.lib.mkApp {
          drv = packages.${NAME};
        };
        defaultApp = apps.${NAME};

        devShell = pkgs.stdenv.mkDerivation {
          name = NAME;
          src = self;
          buildInputs = with pkgs; [
            pkg-config
            cmake
            xorg.libxcb
            xorg.libXfixes
            libxkbcommon
            fontconfig
            wayland
            libGL
            dejavu_fonts
            noto-fonts
            # egl-wayland

            # xdotool / libxdo
            xdotool
          ];
          runtimeDependencies = with pkgs; [
            xdotool
          ];

          shellHook = ''
            export FONTCONFIG_FILE=${pkgs.fontconfig.out}/etc/fonts/fonts.conf
            export XDG_DATA_DIRS=${pkgs.dejavu_fonts}/share:${pkgs.noto-fonts}/share:$XDG_DATA_DIRS
            if [ -z "$WINIT_UNIX_BACKEND" ]; then
              if [ -n "$WAYLAND_DISPLAY" ]; then
                export WINIT_UNIX_BACKEND=wayland
              elif [ -n "$DISPLAY" ]; then
                export WINIT_UNIX_BACKEND=x11
              fi
            fi

            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath (with pkgs; [
              xorg.libxcb
              xorg.libXfixes
              libxkbcommon
              fontconfig
              wayland
              libGL
              stdenv.cc.cc.lib
              freetype

              # GUI runtime libs
              xdotool
            ])}"

          '';
        };
      }
    );
}
