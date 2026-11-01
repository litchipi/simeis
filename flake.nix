{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cicd.url = "git+https://codeberg.org/litchipi/cicd-templates";
  };

  outputs = inputs: with inputs; flake-utils.lib.eachDefaultSystem (system: let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    };

    rust_version = "1.99.1";
    rustpkg = pkgs.rust-bin.stable.${rust_version}.default.override {
      extensions = [ "rust-src" ];
    };

    deps = with pkgs; [
      gcc
      rustpkg
      cargo-audit
      cargo-vet
      mold
      (python313.withPackages (p: with p; [
        requests
      ]))
    ];

    mkScript = script: { type = "app"; program = builtins.toString script; };
    scripts = import ./.forgejo/scripts.nix { inherit pkgs deps; };
    simeis = pkgs.stdenv.mkDerivation {
      pname = "simeis-manual";
      version = "0.1.0";
      src = ./doc;
      buildInputs = [ pkgs.typst pkgs.bash ];
      phases = ["unpackPhase" "buildPhase"];
      buildPhase = ''
        export HOME=$(realpath ./.home)
        ls -lhaR
        mkdir -p $out
        typst compile --root "$PWD" "./manual.typ" "$out/manual.pdf"
      '';
    };
    cicd = inputs.cicd.packages;
  in {
    packages = {
      default = simeis;
      ci = {
        propagate = cicd.fjPropagate pkgs {};
        automerge = cicd.fjAutomerge pkgs {
          labels = [ "propagation" "merge" ];
        };
        dependabot = cicd.fjDependabot pkgs {
          rust = {
            enable = true;
            version = rust_version;
          };
        };
      };
    };

    apps = builtins.mapAttrs (name: val: mkScript val) scripts;
    devShells.default = pkgs.mkShell {
      buildInputs = deps ++ [ pkgs.typst ];
      shellHook = ''
      '';
    };
  });
}
