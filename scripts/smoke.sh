#!/usr/bin/env bash
set -euo pipefail

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"

run() {
  echo "==> $*"
  "$@"
}

should_compile() {
  local feat="$1"
  case "${OS}" in
    darwin)
      [[ "$feat" == "android" ]] && return 1
      [[ "$feat" == "dshow" ]] && return 1
      [[ "$feat" == "v4l2" ]] && return 1
      return 0
      ;;
    linux)
      [[ "$feat" == "android" ]] && return 1
      [[ "$feat" == "avf" ]] && return 1
      [[ "$feat" == "dshow" ]] && return 1
      return 0
      ;;
    mingw*|msys*|cygwin*|windows*)
      [[ "$feat" == "android" ]] && return 1
      [[ "$feat" == "avf" ]] && return 1
      [[ "$feat" == "v4l2" ]] && return 1
      return 0
      ;;
    *)
      return 0
      ;;
  esac
}

features_from_toml() {
  local in_features=0
  while IFS= read -r line; do
    line="${line%%#*}"
    [[ -z "${line//[[:space:]]/}" ]] && continue

    if [[ "$line" =~ ^\[features\]$ ]]; then
      in_features=1
      continue
    fi

    if [[ $in_features -eq 1 && "$line" =~ ^\[.+\]$ ]]; then
      break
    fi

    if [[ $in_features -eq 1 && "$line" =~ ^([a-zA-Z0-9_-]+)[[:space:]]*= ]]; then
      local k="${BASH_REMATCH[1]}"
      [[ "$k" == "default" ]] && continue
      echo "$k"
    fi
  done < Cargo.toml
}

cargo_check_features() {
  local feats="$1"
  if [[ -z "$feats" ]]; then
    run cargo check --quiet --no-default-features
  else
    run cargo check --quiet --no-default-features --features "$feats"
  fi
}

echo "Host OS: $OS"

echo "==> Base checks"
run cargo check --quiet
cargo_check_features ""

echo "==> CLI bins explicitly"
run cargo check --quiet --no-default-features --features "cli"

FEATURES=""
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  FEATURES="${FEATURES} ${f}"
done < <(features_from_toml)

echo "==> Feature list:${FEATURES}"

echo "==> Single-feature checks"
for f in $FEATURES; do
  if should_compile "$f"; then
    cargo_check_features "$f"
  else
    echo "==> cargo check --no-default-features --features $f (skipped: not expected to compile on $OS)"
    run cargo check --quiet --no-default-features --features "$f" || true
  fi
done

echo "==> Pairwise feature checks"
for a in $FEATURES; do
  for b in $FEATURES; do
    [[ "$a" == "$b" ]] && continue
    combo="${a},${b}"

    if should_compile "$a" && should_compile "$b"; then
      cargo_check_features "$combo"
    else
      echo "==> cargo check --no-default-features --features $combo (skipped: not expected to compile on $OS)"
      run cargo check --quiet --no-default-features --features "$combo" || true
    fi
  done
done

echo "==> Canonical combos"
for combo in \
  "cli,std" \
  "cli,ffmpeg" \
  "cli,tracing" \
  "cli,pretty" \
  "all" \
  "all,cli" \
  "native" \
  "native,cli"
do
  ok=1
  IFS=',' read -r a b <<< "$combo"
  if [[ -n "${a:-}" ]] && ! should_compile "$a"; then ok=0; fi
  if [[ -n "${b:-}" ]] && ! should_compile "$b"; then ok=0; fi

  if [[ $ok -eq 1 ]]; then
    cargo_check_features "$combo"
  else
    echo "==> cargo check --no-default-features --features $combo (skipped: not expected to compile on $OS)"
    run cargo check --quiet --no-default-features --features "$combo" || true
  fi
done

echo "OK"
