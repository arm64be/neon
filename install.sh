#!/usr/bin/env bash
set -euo pipefail

repo="${NEON_INSTALL_REPO:-arm64be/neon}"
install_dir="${NEON_INSTALL_BIN_DIR:-$HOME/.local/bin}"
bin_path="${install_dir}/neon"
local_repo="${NEON_LOCAL_REPO:-}"

if [[ -n "${NEON_APP:-}" ]]; then
  app_dir="${NEON_APP}"
elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
  app_dir="${XDG_CONFIG_HOME}/neon"
else
  app_dir="${HOME}/.config/neon"
fi

os="$(uname -s)"
arch="$(uname -m)"

case "${os}-${arch}" in
  Linux-x86_64 | Linux-amd64)
    platform="linux-x86_64"
    ;;
  Darwin-*)
    echo "unsupported platform: macOS releases are not published yet." >&2
    echo "This installer currently supports only Linux x86_64." >&2
    exit 1
    ;;
  Linux-*)
    echo "unsupported Linux architecture: ${arch}." >&2
    echo "This installer currently supports only Linux x86_64." >&2
    exit 1
    ;;
  *)
    echo "unsupported platform: ${os}-${arch}." >&2
    echo "This installer currently supports only Linux x86_64." >&2
    exit 1
    ;;
esac

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

need curl
need find
need grep
need install
need sed
need tar
need mktemp

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp}"
}
trap cleanup EXIT

if [[ -n "${local_repo}" ]]; then
  if [[ ! -d "${local_repo}/.git" ]]; then
    echo "NEON_LOCAL_REPO must point to a local git repository" >&2
    exit 1
  fi
  if [[ ! -f "${local_repo}/Cargo.toml" ]]; then
    echo "NEON_LOCAL_REPO must point to the neon repository root" >&2
    exit 1
  fi
  need cargo

  source_dir="${tmp}/source"
  mkdir -p "${source_dir}"
  cp -R "${local_repo}/configs" "${source_dir}/configs"
else
  source_dir="${tmp}/source"

  api_url="https://api.github.com/repos/${repo}/releases/latest"
  release_json="${tmp}/release.json"
  curl -fsSL "${api_url}" -o "${release_json}"

  asset_url() {
    local pattern="$1"
    sed -n 's/.*"browser_download_url":[[:space:]]*"\([^"]*\)".*/\1/p' "${release_json}" | grep "${pattern}" | head -n1 || true
  }

  binary_url="$(asset_url "neon-.*-${platform}\\.tar\\.gz$")"
  binary_sha_url="$(asset_url "neon-.*-${platform}\\.tar\\.gz\\.sha256$")"
  source_url="$(asset_url "neon-.*-${platform}-source\\.tar\\.gz$")"
  source_sha_url="$(asset_url "neon-.*-${platform}-source\\.tar\\.gz\\.sha256$")"

  if [[ -z "${binary_url}" || -z "${source_url}" ]]; then
    echo "latest release for ${repo} does not include ${platform} binary and source assets" >&2
    exit 1
  fi

  binary_tar="${tmp}/$(basename "${binary_url}")"
  source_tar="${tmp}/$(basename "${source_url}")"
  curl -fsSL "${binary_url}" -o "${binary_tar}"
  curl -fsSL "${source_url}" -o "${source_tar}"

  verify_sha256() {
    local file="$1"
    local url="$2"
    if [[ -z "${url}" ]] || ! command -v sha256sum >/dev/null 2>&1; then
      return
    fi

    local sums="${tmp}/$(basename "${file}").sha256"
    curl -fsSL "${url}" -o "${sums}"
    (cd "${tmp}" && sha256sum -c "${sums}")
  }

  verify_sha256 "${binary_tar}" "${binary_sha_url}"
  verify_sha256 "${source_tar}" "${source_sha_url}"

  mkdir -p "${install_dir}"
  tar -xzf "${binary_tar}" -C "${tmp}"
  install -m 755 "${tmp}/neon" "${bin_path}"

  mkdir -p "${source_dir}"
  tar -xzf "${source_tar}" -C "${source_dir}" --strip-components=1
fi

if [[ ! -f "${source_dir}/configs/onboarding/config.lua" ]]; then
  echo "source tree is missing configs/onboarding/config.lua" >&2
  exit 1
fi

timestamp="$(date -u +%Y%m%d%H%M%S)"
preset_source="${app_dir}.preset-source"
mkdir -p "${app_dir}" "${preset_source}"

if [[ -e "${app_dir}/config.lua" ]]; then
  backup_dir="${app_dir}.backup.${timestamp}"
  cp -R "${app_dir}" "${backup_dir}"
  find "${app_dir}" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
  echo "backed up existing config to ${backup_dir}"
fi

cp -R "${source_dir}/configs/onboarding/." "${app_dir}/"
rm -rf "${app_dir}/themes"
cp -R "${source_dir}/configs/themes" "${app_dir}/themes"
rm -rf "${preset_source}/preset" "${preset_source}/themes"
cp -R "${source_dir}/configs/preset" "${preset_source}/preset"
cp -R "${source_dir}/configs/themes" "${preset_source}/themes"

if [[ -n "${local_repo}" ]]; then
  echo "running onboarding from local repo at ${local_repo}"
  echo "installed onboarding config to ${app_dir}"
  NEON_CONFIG_ROOT="${app_dir}" \
    NEON_REPOSITORY_ROOT="${local_repo}" \
    NEON_ONBOARDING_PRESET_SOURCE="${preset_source}/preset" \
    cargo run --manifest-path "${local_repo}/Cargo.toml"
else
  echo "installed neon to ${bin_path}"
  if [[ ":${PATH}:" != *":${install_dir}:"* ]]; then
    echo "note: ${install_dir} is not currently on PATH"
  fi
  echo "installed onboarding config to ${app_dir}"
  NEON_CONFIG_ROOT="${app_dir}" NEON_ONBOARDING_PRESET_SOURCE="${preset_source}/preset" "${bin_path}"
fi
