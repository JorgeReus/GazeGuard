{
  pkgs ? import <nixpkgs> { },
}:
let
  hasJdk25 = pkgs ? jdk25;
  javaPkg = if hasJdk25 then pkgs.jdk25_headless else pkgs.jdk21_headless;
in
pkgs.mkShell {
  packages = with pkgs; [
    # Mirrors mise.toml as closely as nixpkgs allows.
    gradle_8
    javaPkg
  ];

  shellHook = ''
    if [ "${if hasJdk25 then "1" else "0"}" != "1" ]; then
      echo "warning: nixpkgs has no jdk25; using ${javaPkg.name} instead"
    fi
  '';
}
