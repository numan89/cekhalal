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

Run [`scripts/release.sh`](../../scripts/release.sh) from the repo root —
it does everything below in one go, refusing to proceed if any step fails:

```sh
scripts/release.sh 0.1.2        # new version: code + packaging release
scripts/release.sh --pkgrel-only  # packaging-only fix, no code change
```

It bumps `Cargo.toml`, runs `cargo test`, commits, tags `vX.Y.Z`, pushes
to GitHub, creates a GitHub release, updates `PKGBUILD`'s `pkgver`/`pkgrel`
and checksum, does a *real* `makepkg -f` build (which runs `cargo test`
again inside the isolated build, against the actual release tarball) to
confirm it works before touching AUR at all, regenerates `.SRCINFO`,
commits the packaging update here, and pushes to the AUR git repo.

Preflight checks it runs first: working tree clean, on `master`, `gh`
authenticated as the right account, SSH to AUR working. It won't push
anything if `cargo test` fails at either point.

<details>
<summary>Doing it by hand (if the script doesn't fit your situation)</summary>

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

</details>
