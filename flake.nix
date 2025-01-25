{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }: 
  let 
    forEachSystem = f: with nixpkgs.lib; genAttrs systems.flakeExposed (system: f nixpkgs.legacyPackages.${system});
  in 
  {
    devShells = forEachSystem (pkgs: {
      default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          cargo
          rustfmt
          clippy
          rustc
          sqlite-interactive
          cargo-watch
        ];
        buildInputs = with pkgs; [
          sqlite
        ];
        env.RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });
  };
}
