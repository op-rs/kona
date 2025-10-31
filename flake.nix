{
  description = "Kona development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Use the specific Rust version from rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        # Native build inputs for compilation
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          git
          cargo-nextest
          clang
          llvm
        ];

        # Runtime dependencies
        buildInputs = with pkgs; [
          openssl
          zlib
          just
          libiconv
        ] ++ lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";
          
          # Clang/LLVM configuration for bindgen
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-I${pkgs.glibc.dev}/include";
          
          # OpenSSL configuration for cross-platform compatibility
          OPENSSL_NO_VENDOR = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

          shellHook = ''
            echo "🦀 Kona development environment loaded!"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo test          - Run all tests"
            echo "  cargo test --lib    - Run library tests only"
            echo "  cargo check         - Check compilation"
            echo "  cargo clippy        - Run linter"
            echo "  cargo fmt           - Format code"
            echo ""
          '';
        };

        # Provide the Rust toolchain as a package
        packages.rust = rustToolchain;
      });
}
