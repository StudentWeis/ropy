# AUR packaging

`ropy-bin/` contains the recipe for installing official Ropy Linux release binaries on Arch Linux (`x86_64` only). ARM64 is not included in this package. The recipe does not compile Rust code. Its presence in this repository does not mean the package has been published to AUR.

## Build locally

Install `base-devel`, then build as a regular user in a temporary directory to keep downloaded sources and packages out of the repository:

```sh
workdir=$(mktemp -d)
cp packaging/aur/ropy-bin/{PKGBUILD,.SRCINFO,ropy.desktop} "$workdir/"
cd "$workdir"
makepkg -si
```

Review `PKGBUILD` before running it. `makepkg -si` uses pacman to install missing dependencies and the completed package; it may request administrator authentication. Launch Ropy from the application menu or run `ropy --show`.

The package installs the executable to `/usr/bin/ropy`, where Ropy disables in-app installation of updates. Update through the package manager instead. It does not enable autostart, launch the application during installation, or change compositor settings. See the [Wayland guide](../../docs/WAYLAND.md) for desktop-specific setup and capability limits. Global shortcuts through a portal require `xdg-desktop-portal` and a compatible desktop backend; installing the portal alone does not guarantee support.

## Validate before publishing

On the target architecture, install `namcap` and `desktop-file-utils`, then run these commands in the build directory:

```sh
bash -n PKGBUILD
desktop-file-validate ropy.desktop
makepkg --verifysource --force
makepkg --printsrcinfo > .SRCINFO.generated
diff -u .SRCINFO .SRCINFO.generated
namcap PKGBUILD
namcap ropy-bin-*.pkg.tar.zst
```

For an x86_64 clean-chroot build, install Arch's `devtools` and run `extra-x86_64-build`. Check the installed application's menu launch, tray, clipboard capture, activation and package-managed update behavior in an isolated desktop before publishing. See [release acceptance](../../docs/RELEASE_ACCEPTANCE.md) for the GUI validation procedure.

Dependencies include the release ELF's direct dependencies and the dynamically loaded EGL and Wayland libraries. `libglvnd` supplies EGL; a working graphics driver is still required. Inspect the x86_64 release binary again when upgrading rather than assuming their dependency set stays unchanged. Namcap may not recognize dynamically loaded dependencies.

## Publish to AUR

Prerequisites: an AUR account, its registered SSH public key, a verified AUR SSH host key, and a package name that is available or maintained by that account. Keep the AUR checkout separate from the Ropy checkout.

From the Ropy repository root:

```sh
aur_parent=$(mktemp -d)
git clone ssh://aur@aur.archlinux.org/ropy-bin.git "$aur_parent/ropy-bin"
cp packaging/aur/ropy-bin/{PKGBUILD,.SRCINFO,ropy.desktop} "$aur_parent/ropy-bin/"
cd "$aur_parent/ropy-bin"
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO ropy.desktop
git diff --cached
git commit -m "Add ropy-bin 0.6.2"
git push origin master
```

If authentication, host verification or the push fails, resolve that failure before announcing availability. Do not bypass SSH host verification. Do not commit downloaded archives, build directories or built packages to AUR. After the push succeeds and the AUR package page is visible, users can install with `yay -S ropy-bin` or `paru -S ropy-bin`.

## Update a release

1. Confirm that the new GitHub Release contains the x86_64 Linux archive and the tagged source contains `assets/logo.png`.
2. Update `pkgver` and reset `pkgrel` to `1`. For packaging-only changes to the same version, increment `pkgrel` instead.
3. Download the x86_64 archive and compare its SHA-256 digest with the release's published checksum. Inspect its contents and dependencies. Update the architecture-specific checksums and the versioned icon checksum in `PKGBUILD`; never use `SKIP`. Update the desktop file checksum if that file changes.
4. Build and validate the package, including the installed desktop behavior. Regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO` from the recipe directory.
5. Submit the repository changes through the contribution workflow, then copy the three distribution files into the separate AUR checkout, review, commit and push them.

## Initial validation

Version `0.6.2-1` was built from the published GitHub Release in an isolated, rootless Arch bootstrap environment. The official archive SHA-256 is `6e1abc7e0e98cd77d0df4e9d479e422d6c9ad7d60f26cbc1de6c545aa137a025`; the installed binary SHA-256 is `31ce510e0e7f73352cdc10ff8bb8222045464f6264c7b1686f380a2f7a8036e3`. Source checksums, desktop entry validation, generated `.SRCINFO` consistency and the package build passed. Pacman installation and upgrade checks reported 11 files with no altered files. All ELF dependencies resolved.

The installed `/usr/bin/ropy` passed these checks in an authenticated Xvfb/Openbox desktop with private HOME/XDG directories and D-Bus, without accessing the real desktop's clipboard or application data:

- Desktop entry activation and tray Settings, About and Quit actions.
- Clipboard capture, search, copy confirmation and immediate paste, checked against unique markers and the complete destination editor buffer.
- Three-cycle pointer-inside Escape/shortcut regression, the Settings/back/Escape/shortcut path, native window dragging, pinned focus loss/reactivation and unpinned auto-hide.
- Package-managed updater notice, language switching and language persistence after restart. The test desktop includes `noto-fonts-cjk` for Chinese text.
- A real pacman upgrade from `0.6.1-1` to `0.6.2-1`, preserving history and the exact configuration, followed by shortcut and copy/paste checks.

Namcap reports warnings for the intentional x86_64-only source naming, `libgcc` satisfied through `gcc-libs`, and the dynamically loaded EGL/Wayland dependencies. These were reviewed rather than treating a successful namcap exit as a warning-free result.

This validates the packaged application on software-rendered X11. It is not an `extra-x86_64-build` result and does not certify native Wayland, hardware graphics, ARM64, macOS or Windows GUI behavior. The original `0.6.1` GUI blocker is fixed by [#326](https://github.com/StudentWeis/ropy/pull/326); acceptance used the published `0.6.2` bytes, not a branch build.
