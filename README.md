# Kiyo Control for macOS

The Razer Kiyo Pro is a capable UVC camera, but macOS exposes almost none of
its controls and the camera resets standard settings after power loss. This
project provides a local `kiyo` utility that:

- controls exposure, white balance, focus, zoom, brightness, contrast,
  saturation, sharpness, gamma, hue, gain, backlight compensation, and
  power-line frequency;
- controls the Kiyo Pro's write-only Razer settings: HDR, HDR mode, field of
  view, and autofocus responsiveness;
- stores one plain-JSON profile in
  `~/Library/Application Support/KiyoControl/profile.json`;
- installs a per-user launch agent that reapplies the profile at login and
  whenever the webcam reconnects;
- sends the Kiyo Pro's vendor save command so Razer-specific settings are also
  written to the camera's own NVRAM.

No kernel extension, admin access, cloud service, or Razer Synapse is required.
USB control transfers use the default endpoint and do not claim the camera's
video-streaming interface.

## Build

The Rust `rusb` dependency builds its own libusb, so Homebrew libusb is not
required.

```sh
cargo build --release
./target/release/kiyo diagnose
```

## Signed release

The release script signs the CLI with a Developer ID Application certificate,
creates a DMG, submits it to Apple's notarization service, staples the ticket,
and verifies the finished disk image with Gatekeeper:

```sh
./scripts/release-macos.sh
```

It uses the signing identity
`Developer ID Application: Benjamin De St Paer-Gotch (89N7ZG42ZM)` and the
notarytool Keychain profile `kiyo-notary` by default. Set
`KIYO_SIGNING_IDENTITY` or `KIYO_NOTARY_PROFILE` to override them.

## Use

```sh
# Inspect live values and hardware ranges
kiyo show

# Changes apply immediately and are remembered
kiyo set brightness=140 white_balance_auto=off white_balance=4500
kiyo set auto_exposure=manual exposure_time=250
kiyo set hdr=on hdr_mode=bright fov=wide autofocus_mode=responsive

# Snapshot standard controls or reapply the profile
kiyo save
kiyo apply

# Keep the profile applied after login and reconnects
kiyo install
kiyo uninstall
```

Some controls depend on an automatic mode being disabled. Profile application
sets `white_balance_auto`, `focus_auto`, and `auto_exposure` first so dependent
manual values are accepted.

## About 1080p

The connected Kiyo Pro advertises `1920x1080` at up to `60 fps` through
AVFoundation. A UVC control utility cannot force Zoom, Teams, browsers, or
other applications to request that format: macOS makes capture format a
per-client decision. Enforcing 1080p globally requires a signed CoreMediaIO
camera extension that captures the physical Kiyo and republishes a virtual
camera. That is a separate, higher-risk component; this first version fixes
the settings persistence problem without placing a virtual camera in the
video path.

## Acknowledgements

The Razer extension-unit protocol was documented by the MIT-licensed
[`soyersoyer/kiyoproctrls`](https://github.com/soyersoyer/kiyoproctrls)
project. Standard controls follow the USB Video Class specification.

## License

MIT