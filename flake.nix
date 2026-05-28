{
  description = "Brawler development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , rust-overlay
    }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "clippy"
            "rustfmt"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo-watch
            nodejs_22
            pkg-config
            rustToolchain
            sqlite
            webkitgtk_4_1
          ];

          buildInputs = with pkgs; [
            atk
            cairo
            gdk-pixbuf
            glib
            gtk3
            libsoup_3
            openssl
            pango
          ];

          shellHook = ''
            export RUST_BACKTRACE=1
            echo "Brawler dev shell"
            echo "  npm install       # after scaffold dependency changes"
            echo "  npm run dev       # start Tauri dev app"
            echo "  npm run check     # run local frontend/Rust checks when dependencies are installed"
          '';
        };
      });
}
