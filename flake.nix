{
  description = "COSMIC panel applet with shortcuts to your most recently used apps";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        cosmic-applet-recent-apps = pkgs.callPackage ./package.nix { };
        default = cosmic-applet-recent-apps;
      });

      # pkgs.cosmic-applet-recent-apps, built against your own nixpkgs
      overlays.default = final: _prev: {
        cosmic-applet-recent-apps = final.callPackage ./package.nix { };
      };

      nixosModules.default =
        { config, lib, pkgs, ... }:
        let
          cfg = config.programs.cosmic-applet-recent-apps;
        in
        {
          options.programs.cosmic-applet-recent-apps = {
            enable = lib.mkEnableOption "the COSMIC Recent Apps panel applet";
            package = lib.mkOption {
              type = lib.types.package;
              default = pkgs.callPackage ./package.nix { };
              defaultText = lib.literalMD "built from this flake against your `pkgs`";
              description = "The applet package to install.";
            };
          };

          # The panel finds applets through the .desktop files on XDG_DATA_DIRS,
          # and runs the `Exec=` binary from PATH, so installing it system-wide is enough.
          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
          };
        };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.default ];
          packages = with pkgs; [
            cargo
            clippy
            just
            rust-analyzer
            rustc
            rustfmt
          ];
          # So `cargo run` can find the GPU/windowing libs.
          LD_LIBRARY_PATH = lib.makeLibraryPath (
            with pkgs;
            [
              libglvnd
              libxkbcommon
              vulkan-loader
              wayland
            ]
          );
        };
      });
    };
}
