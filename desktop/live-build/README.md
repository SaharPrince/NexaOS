# NexaOS desktop preview image

This directory defines the first bootable desktop prototype. It is a **live
preview image**, not an installer and not a release-ready operating system.

## Current choices

- Debian 13 (Trixie), amd64.
- Debian's maintained `task-lxqt-desktop` on its packaged X11 session.
- APT recommendations disabled to avoid pulling in optional recommended
  applications such as office suites.
- NetworkManager UI and a small initial set of common firmware packages.
- Debian Live's standard live-user/session behavior.
- No NexaOS Windows runtime, EXE association, executable launcher, custom
  privileged service, third-party extension framework, or anti-cheat integration.

The desktop is intentionally built from maintained distribution packages.
LXQt's existing panel, file manager, settings center, themes, and layout options
are the initial customization surface. NexaOS-specific settings and safer
customization boundaries will be implemented in later steps; this image does
not claim those controls are already hardened.

## Build

On a supported Debian/Ubuntu build host with root-capable `sudo`, install:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends live-build debootstrap xorriso squashfs-tools
```

From the repository root:

```sh
bash scripts/build-live-image.sh
```

The script clears generated live-build stages, configures Debian Trixie, builds
an amd64 hybrid ISO, and writes the image and SHA-256 checksum to
`artifacts/`. The GitHub Actions workflow then boots that ISO in an isolated
QEMU VM with networking disabled. The smoke test observes the VM for 150 seconds
and saves a framebuffer screenshot plus QEMU log in `artifacts/`. The workflow
uploads the ISO, checksum, screenshot, and log for a short retention period.

Run the VM smoke test locally after building (requires QEMU, Xvfb, x11-apps,
ImageMagick, and a Linux host):

```sh
bash scripts/smoke-boot-live-image.sh
```

This is an early boot/framebuffer smoke test, not proof of a usable LXQt login,
working input devices, networking, physical-hardware support, or Windows-app
compatibility. Inspect the captured screenshot and log; do not install the live
image to a host disk.

## Safety and limitations

- Test the preview in a virtual machine first. It is not approved for installing
  over an existing operating system or for production use.
- The image is a live environment and does not include a disk installer.
- The package set is a prototype baseline, not proof of compatibility with
  every GPU, Wi-Fi adapter, laptop, or desktop.
- Firmware package inclusion is subject to each package's license and
  redistribution terms. Review those terms before public distribution.
- No arbitrary Windows PE file is executed, and anti-cheat behavior is not
  tested or modified.
- Do not add automatic login, privilege elevation, or security-setting changes
  as a shortcut to desktop convenience.
