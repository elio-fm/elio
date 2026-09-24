{
  description = "elio terminal file manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      # Keep these outputs aligned with the native Nix CI matrix.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          elio = pkgs.callPackage ./nix/package.nix { };
        in
        {
          inherit elio;
          default = elio;
        }
      );

      checks = forAllSystems (system: {
        inherit (self.packages.${system}) elio;
      });
    };
}
