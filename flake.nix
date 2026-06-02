{
  description = "jotdown-rs - A minimalist, command-line jotting utility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" ];
        };

        jotdown-rs = pkgs.rustPlatform.buildRustPackage rec {
          pname = "jotdown-rs";
          version = "0.2.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          doCheck = true;

          nativeBuildInputs = [ pkgs.git ];

          meta = with pkgs.lib; {
            description = "A minimalist, command-line jotting utility that's fast, private, and git-friendly";
            homepage = "https://github.com/bgreenwell/jotdown-rs";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.all;
          };
        };

      in
      {
        packages.default = jotdown-rs;
        packages.jotdown-rs = jotdown-rs;

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-watch
            cargo-edit
            cargo-audit
            git
            rust-analyzer
          ];

          TERM = "xterm-256color";
        };

        apps.default = {
          type = "app";
          program = "${jotdown-rs}/bin/jd";
        };

        checks = {
          build = jotdown-rs;
        };
      });
}
