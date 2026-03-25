{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  packages = with pkgs; [
    # Strictly pinned to match project requirements.
    gradle_9
    jdk21_headless
  ];
}
