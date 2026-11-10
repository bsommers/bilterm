#!/usr/bin/env bash
# Create git worktrees (and their cargo build output) on roomy storage instead of a quota'd /tmp.
#
#   scripts/worktree.sh new <name> [branch] [base]   add worktree <root>/bilterm-<name> on <branch> (default: <name>)
#                                                    from <base> (default: origin/main); relocates target/ if configured
#   scripts/worktree.sh relocate-target [path]       move <path>/target (default: this worktree) to <target-root>/<worktree-name>
#                                                    and leave a symlink behind (`/target` in .gitignore also matches the link)
#   scripts/worktree.sh where                        print the resolved roots
#
# Roots (first match wins; machine-local, never committed):
#   worktree root: $BILTERM_WORKTREE_ROOT | git config bilterm.worktreeRoot | parent dir of the main checkout
#   target root:   $BILTERM_TARGET_ROOT   | git config bilterm.targetRoot   | (unset: target/ stays in the worktree)
# One-time setup example:
#   git config bilterm.worktreeRoot /data/worktrees && git config bilterm.targetRoot /data/cargo-target
set -euo pipefail

die() { echo "worktree.sh: $*" >&2; exit 1; }

main_checkout() { dirname "$(git rev-parse --path-format=absolute --git-common-dir)"; }

worktree_root() {
  local r="${BILTERM_WORKTREE_ROOT:-$(git config --get bilterm.worktreeRoot || true)}"
  [ -n "$r" ] || r="$(dirname "$(main_checkout)")"
  printf '%s\n' "$r"
}

# Config is read from repo dir $1 (default: cwd) so the script works when run from outside a checkout.
target_root() { printf '%s\n' "${BILTERM_TARGET_ROOT:-$(git -C "${1:-.}" config --get bilterm.targetRoot || true)}"; }

safe_name() { [[ "$1" =~ ^[A-Za-z0-9._-]+$ && "$1" != .* ]] || die "invalid name '$1' (use [A-Za-z0-9._-])"; }

relocate_target() {
  local wt troot name dest
  wt="$(cd "${1:-.}" && git rev-parse --show-toplevel)"
  troot="$(target_root "$wt")"
  [ -n "$troot" ] || die "no target root (set BILTERM_TARGET_ROOT or git config bilterm.targetRoot)"
  name="$(basename "$wt")"
  dest="$troot/$name"
  if [ -L "$wt/target" ]; then
    echo "target already relocated: $wt/target -> $(readlink "$wt/target")"
    return 0
  fi
  [ -d "$troot" ] || { mkdir -p "$troot" && chmod 700 "$troot"; }  # never re-permission an existing dir
  [ ! -e "$dest" ] || die "$dest already exists; remove it or pick another worktree name"
  if [ -d "$wt/target" ]; then
    mv "$wt/target" "$dest"
  else
    mkdir -p "$dest" && chmod 700 "$dest"
  fi
  ln -s "$dest" "$wt/target"
  echo "target relocated: $wt/target -> $dest"
}

cmd_new() {
  [ $# -ge 1 ] || die "usage: new <name> [branch] [base]"
  local name="$1" branch="${2:-$1}" base="${3:-origin/main}" root path
  safe_name "$name"
  root="$(worktree_root)"
  path="$root/bilterm-$name"
  [ ! -e "$path" ] || die "$path already exists"
  [ -d "$root" ] || { mkdir -p "$root" && chmod 700 "$root"; }  # never re-permission an existing dir
  git fetch -q origin
  if git show-ref --verify --quiet "refs/heads/$branch"; then
    git worktree add -q "$path" "$branch"
  else
    git worktree add -q -b "$branch" "$path" "$base"
  fi
  echo "worktree: $path [$branch]"
  if [ -n "$(target_root)" ]; then relocate_target "$path"; fi
}

case "${1:-}" in
  new) shift; cmd_new "$@" ;;
  relocate-target) shift; relocate_target "$@" ;;
  where) echo "worktree root: $(worktree_root)"; echo "target root:   $(target_root || true)" ;;
  *) sed -n '2,15p' "$0"; exit 2 ;;
esac
