{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    { nixpkgs, systems, ... }:
    let
      eachSystem = nixpkgs.lib.genAttrs (import systems);
      pkgsFor = nixpkgs.legacyPackages;
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = pkgsFor.${system};
          dlopenLibraries = with pkgs; [
            libxkbcommon

            # GPU backend
            vulkan-loader
            # libGL

            # Window system
            wayland
          ];
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              cargo
              rustc
              rust-analyzer
            ];

            # additional libraries that your project
            # links to at build time, e.g. OpenSSL
            buildInputs = [ ];

            env.RUSTFLAGS = "-C link-arg=-Wl,-rpath,${nixpkgs.lib.makeLibraryPath dlopenLibraries}";
            env.RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
        }
      );
    };
}
