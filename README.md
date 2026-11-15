# COSMIC Recent Apps applet

A panel applet for the COSMIC desktop that shows shortcuts to your 3 most recently used apps.

- "Used" means a window of the app had focus. The list is saved with cosmic-config,
  so it survives restarts and is shared between panels.
- Clicking an icon focuses that app's window if it is open, otherwise launches it.
- Only apps with a `.desktop` entry are shown.

Written against the applets in [pop-os/cosmic-applets](https://github.com/pop-os/cosmic-applets)
(same libcosmic revision, same patches). `cargo check` passes with Rust 1.97; it has not been
run on a live COSMIC session yet.

## Build

    just build                       # cargo build --release
    just prefix=$HOME/.local install # binary, .desktop entry, icon

Needs Rust (see `rust-toolchain.toml`), `just`, and the usual COSMIC build libraries
(`libxkbcommon`, `wayland`, `libinput`, `udev`, `glib`, `pkg-config`).

## Nix / NixOS

1. In a git repo run `git add -A` first: flakes ignore untracked files.
2. `nix build`. It stops with a hash mismatch: paste the printed `got: sha256-...`
   into `cargoHash` in `package.nix` and build again. Repeat whenever `Cargo.lock` changes.
3. Use it from your system flake:

       inputs.recent-apps.url = "path:/path/to/cosmic-applet-recent-apps";
       inputs.recent-apps.inputs.nixpkgs.follows = "nixpkgs";

       # nixosSystem modules:
       inputs.recent-apps.nixosModules.default
       { programs.cosmic-applet-recent-apps.enable = true; }

   With home-manager, use `nixpkgs.overlays = [ inputs.recent-apps.overlays.default ];`
   and `home.packages = [ pkgs.cosmic-applet-recent-apps ];` instead.
4. Log out and back in, then add it under COSMIC Settings > Desktop > Panel >
   Configure panel applets.

`nix develop` gives a shell with cargo, clippy, rustfmt, rust-analyzer and just.

## Notes

- The app id `com.system76.CosmicAppletRecentApps` follows upstream's convention; rename it in
  `src/config.rs` and the files under `data/` if you want your own namespace.
- Launching does not request an activation token, so a newly launched app may not take focus by itself.
