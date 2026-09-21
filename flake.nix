{
  description = "Rust/ratatui client for the opencode server REST+SSE API";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux"];

      perSystem = {pkgs, ...}: let
        opencode-tui = pkgs.callPackage ./nix/package.nix {};
      in {
        packages.default = opencode-tui;
        packages.opencode-tui = opencode-tui;

        apps.default = {
          type = "app";
          program = "${opencode-tui}/bin/opencode-tui";
        };

        formatter = pkgs.alejandra;

        devShells.default = pkgs.mkShellNoCC {
          packages = [
            pkgs.alejandra
            pkgs.deadnix
            pkgs.just
            pkgs.nil
            pkgs.statix
          ];
        };
      };

      flake = {
        homeManagerModules.default = import ./nix/home-manager.nix;
        homeManagerModules.withPackage = {pkgs, ...}: {
          imports = [inputs.self.homeManagerModules.default];
          programs.opencode-tui.package =
            inputs.self.packages.${pkgs.stdenv.hostPlatform.system}.opencode-tui;
        };
      };
    };
}
