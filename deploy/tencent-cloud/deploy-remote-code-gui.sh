#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-}"
TARGET_DIR="${2:-/opt/remote-code/frontend}"

if [[ -z "${SOURCE_DIR}" ]]; then
  echo "usage: $0 <built-dist-dir> [target-dir]" >&2
  exit 64
fi

if [[ ! -d "${SOURCE_DIR}" ]]; then
  echo "source directory does not exist: ${SOURCE_DIR}" >&2
  exit 66
fi

if [[ ! -f "${SOURCE_DIR}/index.html" ]]; then
  echo "source directory is missing index.html: ${SOURCE_DIR}" >&2
  exit 66
fi

if [[ ! -d "${SOURCE_DIR}/assets" ]]; then
  echo "source directory is missing assets/: ${SOURCE_DIR}" >&2
  exit 66
fi

target_parent="$(dirname "${TARGET_DIR}")"
target_name="$(basename "${TARGET_DIR}")"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
staging_dir="${target_parent}/.${target_name}.staging-${timestamp}"
backup_dir="${target_parent}/${target_name}.backup-${timestamp}"

mkdir -p "${target_parent}"
rm -rf "${staging_dir}"
mkdir -p "${staging_dir}"

cp -a "${SOURCE_DIR}/." "${staging_dir}/"

# nginx needs execute permission on directories and read permission on files.
find "${staging_dir}" -type d -exec chmod 755 {} +
find "${staging_dir}" -type f -exec chmod 644 {} +

if [[ -e "${TARGET_DIR}" ]]; then
  rm -rf "${backup_dir}"
  mv "${TARGET_DIR}" "${backup_dir}"
fi

mv "${staging_dir}" "${TARGET_DIR}"
echo "deployed ${SOURCE_DIR} -> ${TARGET_DIR}"
if [[ -d "${backup_dir}" ]]; then
  echo "backup saved at ${backup_dir}"
fi

# Clean up backups older than 7 days
find "${target_parent}" -maxdepth 1 -name "${target_name}.backup-*" -type d -mtime +7 -exec rm -rf {} +
echo "cleaned up backups older than 7 days in ${target_parent}"
