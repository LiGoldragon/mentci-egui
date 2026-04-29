{
  description = "mentci-egui — first incarnation of the mentci interaction surface (egui shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo" "rustc" "rustfmt" "clippy" "rust-analyzer" "rust-src"
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          name = "mentci-egui";
          packages = [ pkgs.jujutsu pkgs.pkg-config toolchain ];
        };
      }
    );
}
