{
  description = "mentci-egui — first incarnation of the mentci interaction surface (egui shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane/ad8b31ad0ba8448bd958d7a5d50d811dc5d271c0";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
        src = craneLib.cleanCargoSource ./.;

        # Native deps for eframe / egui — windowing (wayland +
        # x11), input (libxkbcommon), accessibility (atk),
        # rendering (libGL + Vulkan loader). Linker also needs
        # libxcb and friends; pkg-config + python3 are wanted by
        # some of the C-side build steps.
        guiBuildInputs = with pkgs; [
          libxkbcommon
          libGL
          vulkan-loader
          wayland
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          xorg.libxcb
          fontconfig
        ];
        guiNativeBuildInputs = with pkgs; [
          pkg-config
        ];
        # Runtime LD_LIBRARY_PATH for the binary on Linux —
        # eframe loads libwayland-client / libxkbcommon at
        # runtime via dlopen, so they need to be locatable.
        runtimeLibPath = pkgs.lib.makeLibraryPath guiBuildInputs;

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = guiNativeBuildInputs;
          buildInputs = guiBuildInputs;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in
      {
        packages.default = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          # Wrap the binary so dlopen-loaded libs resolve at
          # runtime. Without this, the window opens then panics
          # the first time it tries to load wayland/xkb.
          postInstall = ''
            wrapProgram $out/bin/mentci-egui \
              --prefix LD_LIBRARY_PATH : "${runtimeLibPath}"
          '';
          nativeBuildInputs = guiNativeBuildInputs ++ [ pkgs.makeWrapper ];
        });

        checks.default = craneLib.cargoTest (commonArgs // {
          inherit cargoArtifacts;
        });

        devShells.default = pkgs.mkShell {
          name = "mentci-egui";
          packages = [
            pkgs.jujutsu
            toolchain
          ] ++ guiNativeBuildInputs ++ guiBuildInputs;
          # devshell also exports LD_LIBRARY_PATH so
          # `cargo run` works for development.
          LD_LIBRARY_PATH = runtimeLibPath;
        };
      }
    );
}
