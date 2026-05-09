{
  pkgs ? import <nixpkgs> {
    config.allowUnfree = true;
    config.android_sdk.accept_license = true;
  },
}:
let
  buildToolsVersion = "36.1.0";
  android = pkgs.androidenv.composeAndroidPackages {
    includeNDK = "if-supported";
    ndkVersions = [ "latest" ];
    buildToolsVersions = [
      buildToolsVersion
    ];
    platformVersions = [ "36" ];
  };

  androidSdk = android.androidsdk;
in
pkgs.mkShell {
  packages = with pkgs; [
    gradle_9
    jdk21_headless
    androidSdk
  ];

  JAVA_HOME = pkgs.jdk21_headless.home;
  ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
  ANDROID_HOME = "${androidSdk}/libexec/android-sdk";

  shellHook = ''
    export GRADLE_OPTS="-Dorg.gradle.project.android.aapt2FromMavenOverride=$ANDROID_SDK_ROOT/build-tools/${buildToolsVersion}/aapt2"
  '';
}
