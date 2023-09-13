{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib;

        craneLib = crane.lib.${system}.overrideToolchain pkgs.rust-bin.stable.latest.default;
        src =
          craneLib.cleanCargoSource (craneLib.path ./.)
          # craneLib.path ./.
          ;

        dylibDeps = with pkgs; [
          # TODO: Separate linux-only inputs
          gtk3
          glib.out
          gdk-pixbuf
          webkitgtk_4_1
          zlib
          pango.out
          cairo
          harfbuzz
          libsoup_3
          at-spi2-atk
          fuse3
          bzip2.out
          openssl.out
        ];

        commonArgs = {
          inherit src;

          buildInputs = with pkgs; [
            rust-bin.stable.latest.default
            pkg-config
            rust-analyzer
            openssl
          ] ++ dylibDeps;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          GIO_MODULE_DIR = "${pkgs.glib-networking}/lib/gio/modules/";
          LD_LIBRARY_PATH = pkgs.lib.concatMapStringsSep ":" (pkg: "${pkg}/lib") dylibDeps;

          inherit (commonArgs) buildInputs;
        };
      });
}
