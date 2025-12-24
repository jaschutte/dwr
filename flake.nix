{
  description = "Rust Development";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = inputs @ { self, nixpkgs, rust-overlay, flake-utils, ... }:
  flake-utils.lib.eachDefaultSystem (system:
    let
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };
    in {
      devShells.default = pkgs.mkShell rec {
        buildInputs = [
          pkgs.cargo
          pkgs.clippy
          pkgs.rustfmt
          pkgs.rustPackages.clippy
          pkgs.rust-analyzer

          pkgs.wayland
          pkgs.egl-wayland
          pkgs.libGL
          pkgs.pkg-config
        ];

        LD_LIBRARY_PATH = "$LD_LIBRARY_PATH:/run/opengl-driver/lib:/run/opengl-driver-32/lib:${builtins.toString (pkgs.lib.makeLibraryPath buildInputs)}";
        # buildInputs = with pkgs; [
        #   gcc
        #   cargo
        #   rustc
        # ];
      };
    }
  );
}
