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
2. `nix build`. `cargoHash` in `package.nix` is already filled in for the committed `Cargo.lock`.
   If you change `Cargo.lock` (or a nixpkgs update changes how deps are vendored), the build
   fails with a hash mismatch: paste the printed `got: sha256-...` into `cargoHash` and build again.
3. Use it from your system flake:

       inputs.recent-apps.url = "path:/path/to/cosmic-applet-recent-apps";
       inputs.recent-apps.inputs.nixpkgs.follows = "nixpkgs";

       # nixosSystem modules:
       inputs.recent-apps.nixosModules.default
       { programs.cosmic-applet-recent-apps.enable = true; }

   With home-manager, use `nixpkgs.overlays = [ inputs.recent-apps.overlays.default ];`
   and `home.packages = [ pkgs.cosmic-applet-recent-apps ];` instead.
4. Log out and back in, then add it to your dock (see "Adding it to the dock" below).

`nix develop` gives a shell with cargo, clippy, rustfmt, rust-analyzer and just.

## Adding it to the dock

COSMIC Settings > Desktop > **Dock** > Configuration > **Configure dock applets**, then
**Add applet** (bottom right), search "Recent Apps" and press **Add**. Drag it within the
Start / Center / End segments to position it. (The top bar works the same way under
Desktop > Panel > Configure panel applets.)

- If the Dock page only shows **"Panel configuration is missing"** with a *Reset to default*
  button, the dock has no saved configuration yet: press the button, then continue as above.
- If the Dock page has a **Dock** switch, make sure it is on.

## If it doesn't show up

COSMIC Settings and the panel find applets only by scanning the `.desktop` files on
`XDG_DATA_DIRS`, then the panel runs the entry's `Exec=` line. So:

1. **Is it installed?** `nix build` only creates `./result`; that is not on any search path.
   Install it (the NixOS module, `home.packages`, `nix profile install`, or `just install`), then check:

       find -L $(echo $XDG_DATA_DIRS | tr : ' ') -name 'com.system76.CosmicAppletRecentApps.desktop' 2>/dev/null

   No output means no profile COSMIC reads contains it yet.
2. **Is it in the list?** Close and reopen COSMIC Settings (the list is scanned when the page opens),
   then Add applet > "Recent Apps" as described above (the list says "No applets found..."
   when nothing on `XDG_DATA_DIRS` matches). If it is still missing, log out and back in.
3. **Added but blank?** With nothing to show yet the applet draws a small "recent" icon with a
   tooltip. Once you have focused a few windows it is replaced by their icons. If the placeholder
   never goes away, the focused apps have no matching `.desktop` entry (see the log below).
4. **Logs.** The panel forwards the applet's output to the journal, tagged with its id:

       journalctl --user -b | grep CosmicAppletRecentApps

   Look for `recent apps applet started`, `most recently used app`, `no desktop entry found`,
   or `exited with code` (a crash, with its message just above). Set `RUST_LOG=debug` for more.

The installed `.desktop` file points `Exec=` at the absolute path of the binary, because the
panel starts applets with its own `PATH` (which on NixOS or with a `~/.local` prefix usually
does not contain the binary).

## Icons

The applet uses the icon theme the desktop is set to (Settings > Desktop > Appearance > Icons),
at startup and live when you change it. For every app it shows, it logs the desktop entry, the
`Icon=` name, the theme and the file it resolved:

    journalctl --user -b | grep "recent app icon"

If an icon still looks different from the one in the dock's app list, that line shows whether the
theme has no icon of that name (the lookup then falls back to the app's own `hicolor` icon).

## Notes

- The app id `com.system76.CosmicAppletRecentApps` follows upstream's convention; rename it in
  `src/config.rs` and the files under `data/` if you want your own namespace.
- Launching does not request an activation token, so a newly launched app may not take focus by itself.
