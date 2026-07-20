# AUR packaging

`PKGBUILD` and `.SRCINFO` for the [AUR](https://aur.archlinux.org/) package,
kept here for version control and review. This directory is *not* what gets
pushed to AUR directly — AUR's own git repo only ever holds `PKGBUILD` +
`.SRCINFO` (no source tree), so treat this as the source of truth and copy
from here.

Verified locally with `makepkg -f` (builds from the tagged GitHub release
tarball, runs `cargo test` in `check()`, produces a real installable
`.pkg.tar.zst`) before every push to AUR.

## Releasing a new version

1. Bump `version` in `Cargo.toml`, commit, tag `vX.Y.Z`, push the tag
   (`git push origin vX.Y.Z`), and create a GitHub release for it so the
   source tarball URL below resolves.
2. In this directory: bump `pkgver` (and reset `pkgrel=1`) in `PKGBUILD`,
   then run `updpkgsums` to refresh the checksum.
3. `makepkg -f` to build and confirm it still works.
4. `makepkg --printsrcinfo > .SRCINFO` to regenerate.
5. Copy `PKGBUILD` + `.SRCINFO` into a clone of
   `ssh://aur@aur.archlinux.org/cekhalal.git`, commit, and push.

A packaging-only fix (no new upstream version) bumps `pkgrel` instead of
`pkgver`.
