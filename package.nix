{
  lib,
  rustPlatform,
  libcosmicAppHook,
  pkg-config,
  glib,
  libinput,
  libxkbcommon,
  udev,
  wayland,
}:

let
  crate = "cosmic-applet-recent-apps";
  appId = "com.system76.CosmicAppletRecentApps";
in
rustPlatform.buildRustPackage {
  pname = crate;
  version = (lib.importTOML ./Cargo.toml).package.version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.difference (lib.fileset.fromSource (lib.sources.cleanSource ./.)) (
      lib.fileset.unions [
        ./flake.nix
        ./package.nix
      ]
    );
  };

  # Build once with this left as `lib.fakeHash`; Nix prints the real hash
  # ("got: sha256-...") to paste here. Update it whenever Cargo.lock changes.
  cargoHash = lib.fakeHash;

  doCheck = false;

  nativeBuildInputs = [
    libcosmicAppHook # wraps the binary with the runtime libs/icon paths COSMIC apps need
    pkg-config
  ];

  buildInputs = [
    glib
    libinput
    libxkbcommon
    udev
    wayland
  ];

  postInstall = ''
    install -Dm0644 data/${appId}.desktop \
      -t $out/share/applications
    install -Dm0644 data/icons/scalable/apps/${appId}.svg \
      -t $out/share/icons/hicolor/scalable/apps
  '';

  meta = {
    description = "COSMIC panel applet with shortcuts to your most recently used apps";
    license = lib.licenses.gpl3Only;
    platforms = lib.platforms.linux;
    mainProgram = crate;
  };
}
