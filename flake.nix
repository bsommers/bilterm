{
  description = "Bilterm — IBM 3270 & Borland Turbo Vision inspired TUI multiplexer and composite-pane shell wrapper";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "bilterm";
            version = "1.10.2";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        });

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              cargo-hack
              rustc
              rustfmt
              clippy
              tmux
              zig_0_15 # for the optional `ghostty-vt` feature
            ];
          };
        });
    };
}
