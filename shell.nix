{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    (rustfmt.override { asNightly = true; })
    clippy
  ];
}
