{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    devshell.url = "github:numtide/devshell";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, devshell, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ devshell.overlays.default ];
        };
        pkgs-unstable = import nixpkgs-unstable { inherit system; };
      in
      {
        devShells.local = pkgs.devshell.mkShell {
          name = "bun-sys";
          packages = [
            pkgs-unstable.bun
            pkgs-unstable.cmake
            pkgs-unstable.ninja
            pkgs-unstable.pkg-config
            pkgs-unstable.ccache
            pkgs-unstable.clang_21
            pkgs-unstable.llvm_21
            pkgs-unstable.lld_21
            pkgs-unstable.python3
            pkgs-unstable.libtool
            pkgs-unstable.ruby
            pkgs-unstable.perl
            pkgs-unstable.openssl
            pkgs-unstable.zlib
            pkgs-unstable.libxml2
            pkgs-unstable.libiconv
            pkgs.git
            pkgs.rustup
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs-unstable.apple-sdk_15
          ];

          devshell.startup.setup = {
            text = ''
              if [[ "$OSTYPE" == "darwin"* ]]; then
                export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
                export CMAKE_OSX_SYSROOT="$SDKROOT"
                export NIX_CFLAGS_COMPILE="''${NIX_CFLAGS_COMPILE:-} -isysroot $SDKROOT"
              fi
            '';
          };
        };
      });
}
