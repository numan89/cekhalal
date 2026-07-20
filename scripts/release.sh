#!/usr/bin/env bash
#
# Cuts a release of cekhalal end to end: bumps the version, tests, tags,
# pushes to GitHub, creates a GitHub release, updates + rebuilds the AUR
# PKGBUILD against that release (a real `makepkg -f`, not just a checksum
# edit), and pushes the result to AUR.

set -euo pipefail

REPO_OWNER="numan89"
REPO_NAME="cekhalal"
AUR_SSH_KEY="$HOME/.ssh/id_ed25519"
MAINTAINER_NAME="Muhammad Nu'man"
MAINTAINER_EMAIL="numany2k2005@gmail.com"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)"
PKGBUILD_PATH="$REPO_ROOT/packaging/aur/PKGBUILD"
SRCINFO_PATH="$REPO_ROOT/packaging/aur/.SRCINFO"

log()  { printf '\n\033[1;32m==>\033[0m %s\n' "$*"; }
err()  { printf '\033[1;31mERROR:\033[0m %s\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

usage() {
  cat <<'EOF'
Usage:
  scripts/release.sh
      No version given: auto-bumps the patch version (X.Y.Z -> X.Y.(Z+1))
      from whatever's currently in Cargo.toml, and releases that.

  scripts/release.sh <X.Y.Z>
      Releases under an explicit version instead (e.g. for a minor/major
      bump).

  scripts/release.sh --pkgrel-only
      Packaging-only fix, no code change: no Cargo.toml/tag/GitHub
      release, just bumps the AUR pkgrel and republishes.

Safety: refuses to run with a dirty working tree, requires the active gh
account to match the configured owner, requires SSH to AUR to work, and
requires 'cargo test' (twice: once locally, once inside the isolated
makepkg build) to pass before anything gets pushed anywhere.
EOF
}

PKGREL_ONLY=0
NEW_VERSION=""
case "${1:-}" in
  --pkgrel-only) PKGREL_ONLY=1 ;;
  -h|--help) usage; exit 0 ;;
  "") ;; # no version given -- auto-bump patch, resolved after preflight checks
  *) NEW_VERSION="$1" ;;
esac

cd "$REPO_ROOT"

# ---- Preflight checks -------------------------------------------------

log "Checking required tools are installed"
for tool in git gh cargo makepkg updpkgsums ssh; do
  command -v "$tool" &>/dev/null || die "Required tool '$tool' not found on PATH."
done

log "Checking working tree is clean"
[[ -z "$(git status --porcelain)" ]] || die "Working tree is dirty -- commit or stash first."

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[[ "$BRANCH" == "master" ]] || die "On branch '$BRANCH', expected 'master'. Switch branches first."

log "Checking gh is authenticated as $REPO_OWNER"
ACTIVE_GH_USER="$(gh api user --jq .login 2>/dev/null || true)"
if [[ "$ACTIVE_GH_USER" != "$REPO_OWNER" ]]; then
  die "gh is authenticated as '$ACTIVE_GH_USER', not '$REPO_OWNER'. Run: gh auth switch -h github.com -u $REPO_OWNER"
fi

log "Checking SSH access to AUR"
[[ -f "$AUR_SSH_KEY" ]] || die "AUR SSH key not found at $AUR_SSH_KEY"
if ! ssh -i "$AUR_SSH_KEY" -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=10 \
      aur@aur.archlinux.org help &>/dev/null; then
  die "SSH to aur@aur.archlinux.org failed with key $AUR_SSH_KEY. Check the key is registered on your AUR account."
fi

if [[ $PKGREL_ONLY -eq 0 ]]; then
  CURRENT_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)"
  [[ -n "$CURRENT_VERSION" ]] || die "Could not read the current version from Cargo.toml."

  if [[ -z "$NEW_VERSION" ]]; then
    [[ "$CURRENT_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] \
      || die "Cargo.toml version '$CURRENT_VERSION' isn't X.Y.Z -- pass an explicit version."
    NEW_VERSION="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.$(( BASH_REMATCH[3] + 1 ))"
    log "No version given -- auto-bumping patch: $CURRENT_VERSION -> $NEW_VERSION"
  fi

  [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Version must look like X.Y.Z, got '$NEW_VERSION'."
  [[ "$NEW_VERSION" != "$CURRENT_VERSION" ]] || die "Cargo.toml is already at $NEW_VERSION. Use --pkgrel-only for a packaging-only fix."
  git tag --list "v$NEW_VERSION" | grep -q . && die "Tag v$NEW_VERSION already exists."
fi

# ---- 1. Cargo.toml version bump, test, tag, GitHub release -----------

if [[ $PKGREL_ONLY -eq 0 ]]; then
  log "Bumping Cargo.toml: $CURRENT_VERSION -> $NEW_VERSION"
  sed -i "s/^version = \".*\"\$/version = \"$NEW_VERSION\"/" Cargo.toml

  log "Running cargo test (local build cache)"
  cargo test

  log "Committing and tagging v$NEW_VERSION"
  git add Cargo.toml Cargo.lock
  git commit -m "Bump to v$NEW_VERSION"
  git tag -a "v$NEW_VERSION" -m "v$NEW_VERSION"

  log "Pushing to GitHub"
  git push origin master
  git push origin "v$NEW_VERSION"

  log "Creating GitHub release"
  gh release create "v$NEW_VERSION" --title "$REPO_NAME v$NEW_VERSION" --generate-notes
else
  NEW_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)"
  log "Packaging-only fix for already-released v$NEW_VERSION"
fi

# ---- 2. Update PKGBUILD ------------------------------------------------

if [[ $PKGREL_ONLY -eq 0 ]]; then
  log "Setting PKGBUILD pkgver=$NEW_VERSION, pkgrel=1"
  sed -i "s/^pkgver=.*\$/pkgver=$NEW_VERSION/" "$PKGBUILD_PATH"
  sed -i "s/^pkgrel=.*\$/pkgrel=1/" "$PKGBUILD_PATH"
else
  CURRENT_PKGREL="$(sed -n 's/^pkgrel=//p' "$PKGBUILD_PATH")"
  NEW_PKGREL=$((CURRENT_PKGREL + 1))
  log "Bumping PKGBUILD pkgrel=$CURRENT_PKGREL -> $NEW_PKGREL"
  sed -i "s/^pkgrel=.*\$/pkgrel=$NEW_PKGREL/" "$PKGBUILD_PATH"
fi

log "Recomputing source checksum against the GitHub release tarball"
( cd "$REPO_ROOT/packaging/aur" && updpkgsums && rm -f ./*.tar.gz )

# ---- 3. Real build+test in an isolated directory -----------------------

BUILD_DIR="$(mktemp -d)"
cleanup() { rm -rf "$BUILD_DIR"; }
trap cleanup EXIT

log "Building the package for real (makepkg -f, includes cargo test) in $BUILD_DIR"
cp "$PKGBUILD_PATH" "$BUILD_DIR/PKGBUILD"
( cd "$BUILD_DIR" && makepkg -f )

PKG_FILE="$(find "$BUILD_DIR" -maxdepth 1 -name "${REPO_NAME}-*.pkg.tar.zst" ! -name "*debug*" | head -1)"
[[ -n "$PKG_FILE" ]] || die "makepkg did not produce a package -- something is wrong."
log "Built: $(basename "$PKG_FILE")"

log "Regenerating .SRCINFO"
( cd "$BUILD_DIR" && makepkg --printsrcinfo > "$SRCINFO_PATH" )

# ---- 4. Commit packaging update to the main repo ------------------------

log "Committing packaging update to $REPO_NAME"
git add packaging/aur/PKGBUILD packaging/aur/.SRCINFO
git commit -m "$([[ $PKGREL_ONLY -eq 0 ]] && echo "Bump AUR package to $NEW_VERSION" || echo "AUR packaging fix (pkgrel bump, no version change)")"
git push origin master

# ---- 5. Push to AUR -----------------------------------------------------

log "Pushing to AUR"
AUR_DIR="$(mktemp -d)"
cleanup() { rm -rf "$BUILD_DIR" "$AUR_DIR"; }
trap cleanup EXIT

GIT_SSH_COMMAND="ssh -i $AUR_SSH_KEY -o IdentitiesOnly=yes" \
  git clone ssh://aur@aur.archlinux.org/"$REPO_NAME".git "$AUR_DIR"
cp "$PKGBUILD_PATH" "$SRCINFO_PATH" "$AUR_DIR/"
(
  cd "$AUR_DIR"
  git add PKGBUILD .SRCINFO
  if git diff --cached --quiet; then
    echo "No change to push to AUR (already up to date)."
  else
    git -c user.name="$MAINTAINER_NAME" -c user.email="$MAINTAINER_EMAIL" \
      commit -m "$REPO_NAME $(sed -n 's/^[[:space:]]*pkgver = //p' "$SRCINFO_PATH" | head -1)-$(sed -n 's/^[[:space:]]*pkgrel = //p' "$SRCINFO_PATH" | head -1)"
    GIT_SSH_COMMAND="ssh -i $AUR_SSH_KEY -o IdentitiesOnly=yes" git push origin master
  fi
)

log "Done."
echo "  GitHub: https://github.com/$REPO_OWNER/$REPO_NAME"
echo "  AUR:    https://aur.archlinux.org/packages/$REPO_NAME"
echo "Note: AUR's search index (used by yay/paru) can lag a few minutes behind this push."
