{
  description = "Brawler development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    # Unstable channel for fast-moving dev tools absent from (or too old in)
    # 24.11: claude-code (the MCP real-client verification, ADR 0078) and
    # nodejs_22 (jsdom 30 requires >=22.22.2; 24.11 stalls at 22.16.0). Not
    # used for anything shipped.
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    { self
    , nixpkgs
    , nixpkgs-unstable
    , flake-utils
    , rust-overlay
    }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        pkgsUnstable = import nixpkgs-unstable {
          inherit system;
          config.allowUnfree = true; # claude-code ships under an unfree license
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
        # LD_LIBRARY_PATH deliberately omits openssl: in-shell Rust builds get
        # an rpath to it already, and force-loading 24.11's OpenSSL 3.3 would
        # shadow the unstable Node's own 3.4+ (missing OPENSSL_3.4.0 symbols).
        # Do NOT swap openssl to pkgsUnstable instead — mixing nixpkgs
        # generations in buildInputs links against two glibcs and the test
        # binaries fail to load (GLIBC_ABI_DT_X86_64_PLT).
        linuxLibraryPath = pkgs.lib.makeLibraryPath
          (builtins.filter (p: p != pkgs.openssl) linuxNativeLibraries);
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            pkgsUnstable.cargo-deny
            cargo-llvm-cov
            cargo-mutants
            cargo-nextest
            cargo-watch
            dpkg
            file
            git-cliff
            pkgsUnstable.nodejs_22
            pkg-config
            rpm
            rustToolchain
            sqlite
            zip
            webkitgtk_4_1
            pkgsUnstable.claude-code
          ];

          buildInputs = linuxNativeLibraries;

          shellHook = ''
            export RUST_BACKTRACE=1
            export LD_LIBRARY_PATH="${linuxLibraryPath}:''${LD_LIBRARY_PATH:-}"
            # Greeting goes to STDERR: `nix develop -c cmd > file` captures
            # stdout, and the greeting polluted every such capture (the
            # v0.61.6/v0.61.7 release notes shipped this banner as their body).
            echo "Brawler dev shell" >&2
            echo "  npm install       # after scaffold dependency changes" >&2
            echo "  npm run dev       # start Tauri dev app" >&2
            echo "  npm run check     # run local frontend/Rust checks when dependencies are installed" >&2
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
            pkgsUnstable.nodejs_22
            nsis
            pkg-config
            rustWindowsToolchain
            zip
          ];

          shellHook = ''
            export RUST_BACKTRACE=1
            export XWIN_CACHE_DIR="$PWD/.xwin-cache"
            export PATH="$(printf '%s' "$PATH" | tr ':' '\n' | grep -v -x "$HOME/.local/bin" | grep -v -x "$HOME/.cargo/bin" | paste -sd: -)"
            echo "Brawler Windows-from-Linux packaging shell" >&2
            echo "  npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc" >&2
          '';
        };
      });
}
