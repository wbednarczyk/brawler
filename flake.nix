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
        rustWindowsToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "clippy"
            "rustfmt"
          ];
          targets = [
            "x86_64-pc-windows-msvc"
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

        devShells.windows-cross = pkgs.mkShell {
          packages = with pkgs; [
            cargo-xwin
            clang
            imagemagick
            lld
            llvm
            nodejs_22
            nsis
            pkg-config
            rustWindowsToolchain
          ];

          shellHook = ''
            export RUST_BACKTRACE=1
            export XWIN_CACHE_DIR="$PWD/.xwin-cache"
            export PATH="$(printf '%s' "$PATH" | tr ':' '\n' | grep -v -x "$HOME/.local/bin" | grep -v -x "$HOME/.cargo/bin" | paste -sd: -)"
            echo "Brawler Windows-from-Linux packaging shell"
            echo "  npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc"
          '';
        };
      });
}
