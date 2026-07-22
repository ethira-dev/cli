#!/usr/bin/env sh
# Verified Ethira CLI installer: downloads a GitHub Release archive and checks SHA256SUMS.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ethira-dev/cli/main/scripts/install.sh | sh
# Optional env:
#   ETHIRA_VERSION=0.1.0          # omit leading v; default: latest
#   ETHIRA_INSTALL_DIR=~/.local/bin
#   ETHIRA_REPO=ethira-dev/cli

set -eu

REPO="${ETHIRA_REPO:-ethira-dev/cli}"
INSTALL_DIR="${ETHIRA_INSTALL_DIR:-${HOME}/.local/bin}"
BASE_URL="https://github.com/${REPO}/releases"

umask 022

die() {
  echo "ethira-install: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

need_cmd curl
need_cmd tar
need_cmd mktemp

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "${OS}" in
  linux) os_part="unknown-linux-gnu" ;;
  darwin) os_part="apple-darwin" ;;
  *) die "unsupported OS: ${OS} (use Homebrew or build from source)" ;;
esac

case "${ARCH}" in
  x86_64 | amd64) arch_part="x86_64" ;;
  arm64 | aarch64) arch_part="aarch64" ;;
  *) die "unsupported architecture: ${ARCH}" ;;
esac

if [ "${OS}" = "linux" ] && [ "${arch_part}" = "aarch64" ]; then
  die "no linux/aarch64 release archive yet; use Homebrew (if available) or build from source"
fi

TARGET="${arch_part}-${os_part}"
ARCHIVE="ethira-${TARGET}.tar.gz"

resolve_tag() {
  if [ -n "${ETHIRA_VERSION:-}" ]; then
    case "${ETHIRA_VERSION}" in
      v*) echo "${ETHIRA_VERSION}" ;;
      *) echo "v${ETHIRA_VERSION}" ;;
    esac
    return
  fi
  # Follow redirect to the latest release tag.
  loc="$(curl -fsSL -o /dev/null -w '%{url_effective}' "${BASE_URL}/latest")"
  tag="${loc##*/}"
  [ -n "${tag}" ] && [ "${tag}" != "latest" ] || die "could not resolve latest release tag"
  echo "${tag}"
}

TAG="$(resolve_tag)"
VERSION="${TAG#v}"
DOWNLOAD_URL="${BASE_URL}/download/${TAG}"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT INT HUP TERM

echo "ethira-install: downloading ${ARCHIVE} (${TAG})"
curl -fsSL "${DOWNLOAD_URL}/${ARCHIVE}" -o "${TMP}/${ARCHIVE}"
curl -fsSL "${DOWNLOAD_URL}/SHA256SUMS" -o "${TMP}/SHA256SUMS"

verify_checksum() {
  expected="$(
    awk -v f="${ARCHIVE}" '$2 == f { print $1; exit }' "${TMP}/SHA256SUMS"
  )"
  [ -n "${expected}" ] || die "SHA256SUMS has no entry for ${ARCHIVE}"

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${TMP}/${ARCHIVE}" | awk '{ print $1 }')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${TMP}/${ARCHIVE}" | awk '{ print $1 }')"
  else
    die "need sha256sum or shasum to verify download"
  fi

  [ "${actual}" = "${expected}" ] || die "checksum mismatch for ${ARCHIVE} (got ${actual}, want ${expected})"
  echo "ethira-install: checksum ok"
}

verify_checksum

tar -xzf "${TMP}/${ARCHIVE}" -C "${TMP}"
[ -f "${TMP}/ethira" ] || die "archive missing ethira binary"

mkdir -p "${INSTALL_DIR}"
# Install atomically when possible.
if mv -f "${TMP}/ethira" "${INSTALL_DIR}/ethira" 2>/dev/null; then
  :
else
  # Cross-device fallback
  cp -f "${TMP}/ethira" "${INSTALL_DIR}/ethira"
fi
chmod 755 "${INSTALL_DIR}/ethira"

echo "ethira-install: installed ethira ${VERSION} -> ${INSTALL_DIR}/ethira"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "ethira-install: note: ${INSTALL_DIR} is not on PATH; add it or re-open your shell" >&2
    ;;
esac

"${INSTALL_DIR}/ethira" --version || true
