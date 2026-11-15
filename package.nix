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

  # Hash of the vendored dependencies (derived from Cargo.lock only). When Cargo.lock
  # changes, set this to `lib.fakeHash`, build once, and paste the "got: sha256-..." value.
  cargoHash = "sha256-i/uaFgV1Q2IB3ohCLSkxfJbjDpK4Oprlw+DKH7yFyNs=";

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
    # The panel starts applets with its own PATH, so point Exec at the store path.
    substituteInPlace $out/share/applications/${appId}.desktop \
      --replace-fail "Exec=${crate}" "Exec=$out/bin/${crate}"
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
