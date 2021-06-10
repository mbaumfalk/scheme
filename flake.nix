{
  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, rust-overlay, crate2nix, ... }: rust-overlay.inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      name = "lisp";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          rust-overlay.overlay
          (self: super: {
            rustc = self.rust-bin.stable.latest.default;
            cargo = self.rust-bin.stable.latest.default;
          })
        ];
      };
      inherit (import "${crate2nix}/tools.nix" { inherit pkgs; }) generatedCargoNix;

      project = pkgs.callPackage
        (generatedCargoNix {
          inherit name;
          src = ./.;
        }) {};
    in rec {
      packages.${name} = project.rootCrate.build;
      defaultPackage = packages.${name};

      app.${name} = rust-overlay.inputs.flake-utils.lib.mkApp {
        inherit name;
        drv = packages.${name};
      };
      defaultApp = app.${name};

      devShell = pkgs.mkShell {
        nativeBuildInputs = defaultPackage.nativeBuildInputs ++ [ pkgs.rust-analyzer ];
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
      };
    });
}
