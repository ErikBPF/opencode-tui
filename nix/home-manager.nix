# Installs the opencode-tui client. It attaches to an already-running
# `opencode serve`; it never starts or manages one.
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.programs.opencode-tui;
in {
  options.programs.opencode-tui = {
    enable = lib.mkEnableOption "the opencode-tui client";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      description = "The opencode-tui package to install. Defaults to the pinned nixpkgs build.";
    };

    url = lib.mkOption {
      type = lib.types.str;
      default = "http://127.0.0.1:4096";
      description = "Default server the client attaches to (OPENCODE_URL).";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [
      (
        if cfg.package != null
        then cfg.package
        else pkgs.callPackage ../../nix/package.nix {}
      )
    ];

    home.sessionVariables.OPENCODE_URL = cfg.url;
  };
}
