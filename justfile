name := 'cosmic-applet-recent-apps'
appid := 'com.system76.CosmicAppletRecentApps'

rootdir := ''
prefix := '/usr'
debug := '0'
vendor := '0'

target := if debug == '1' { 'debug' } else { 'release' }
vendor_args := if vendor == '1' { '--frozen --offline' } else { '' }
debug_args := if debug == '1' { '' } else { '--release' }
cargo_args := vendor_args + ' ' + debug_args

targetdir := env('CARGO_TARGET_DIR', 'target')
sharedir := rootdir + prefix + '/share'
iconsdir := sharedir + '/icons/hicolor'
bindir := rootdir + prefix + '/bin'

default: build

# Compiles the applet (release profile unless debug=1)
build *args:
    cargo build {{ cargo_args }} {{ args }}

# Type-checks without producing a binary
check *args:
    cargo check {{ args }}

# Installs the binary, desktop entry and icon (set prefix=... / rootdir=... as needed).
# The desktop entry gets an absolute Exec path: the panel starts applets with its own
# PATH, which does not contain e.g. ~/.local/bin or ~/.nix-profile/bin.
install:
    install -Dm0755 {{ targetdir }}/{{ target }}/{{ name }} {{ bindir }}/{{ name }}
    install -d {{ sharedir }}/applications
    sed 's|^Exec=.*|Exec={{ prefix }}/bin/{{ name }}|' data/{{ appid }}.desktop > {{ sharedir }}/applications/{{ appid }}.desktop
    chmod 0644 {{ sharedir }}/applications/{{ appid }}.desktop
    install -Dm0644 data/icons/scalable/apps/{{ appid }}.svg {{ iconsdir }}/scalable/apps/{{ appid }}.svg

uninstall:
    rm -f {{ bindir }}/{{ name }} {{ sharedir }}/applications/{{ appid }}.desktop {{ iconsdir }}/scalable/apps/{{ appid }}.svg

clean:
    cargo clean
