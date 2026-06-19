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
            "llvm-tools-preview" # for cargo-llvm-cov (coverage)
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
        linuxNativeLibraries = with pkgs; [
          atk
          cairo
          gdk-pixbuf
          glib
          gtk3
          libsoup_3
          openssl
          pango
          webkitgtk_4_1
        ];
        linuxLibraryPath = pkgs.lib.makeLibraryPath linuxNativeLibraries;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo-llvm-cov
            cargo-mutants
            cargo-nextest
            cargo-watch
            dpkg
            file
            git-cliff
            nodejs_22
            pkg-config
            rpm
            rustToolchain
            sqlite
            zip
            webkitgtk_4_1
          ];

          buildInputs = linuxNativeLibraries;

          shellHook = ''
            export RUST_BACKTRACE=1
            export LD_LIBRARY_PATH="${linuxLibraryPath}:''${LD_LIBRARY_PATH:-}"
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
            file
            imagemagick
            lld
            llvm
            nodejs_22
            nsis
            pkg-config
            rustWindowsToolchain
            zip
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
