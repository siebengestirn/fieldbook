#!/usr/bin/env bash
set -Eeuo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This test requires Linux loop devices and ext4." >&2
  exit 2
fi

if [[ "$(id -u)" != "0" ]]; then
  command -v sudo >/dev/null 2>&1 || {
    echo "Required tool not found: sudo" >&2
    exit 2
  }
fi

for tool in mkfs.ext4 mount umount truncate dd; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "Required tool not found: $tool" >&2
    exit 2
  }
done

run_root() {
  if [[ "$(id -u)" == "0" ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

work_dir="$(mktemp -d)"
image="$work_dir/journal-enospc.img"
mount_dir="$work_dir/mnt"
mkdir -p "$mount_dir"
mounted=0

cleanup() {
  if [[ "$mounted" == "1" ]]; then
    run_root umount "$mount_dir"
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

# 128 MiB ext4 volume, no reserved blocks. The preallocation leaves room
# for the manifest/header and then forces journal record writes to ENOSPC.
truncate -s 128M "$image"
mkfs.ext4 -F -m 0 "$image" >/dev/null
run_root mount -o loop "$image" "$mount_dir"
mounted=1
run_root chown "$(id -u):$(id -g)" "$mount_dir"
dd if=/dev/zero of="$mount_dir/filler" bs=1M count=80 conv=fsync status=none
mkdir "$mount_dir/journal"

export JOURNAL_ENOSPC_TEST_ROOT="$mount_dir/journal"
export JOURNAL_ENOSPC_FILLER="$mount_dir/filler"

test_name="journal::tests::storage_write_failure_marks_run_failed"
# A missing or renamed test makes `cargo test --exact` exit 0 with zero runs.
# Verify the name first, then require exactly one passing test.
list_output="$(cargo test --lib -- --list)"
printf '%s\n' "$list_output" | grep -Fxq "${test_name}: test" || {
  echo "ENOSPC drill test ${test_name} is missing" >&2
  exit 1
}
test_output="$(cargo test --lib "$test_name" -- --exact --nocapture 2>&1)" || {
  printf '%s\n' "$test_output"
  exit 1
}
printf '%s\n' "$test_output"
grep -Eq '^test result: ok\. 1 passed; 0 failed' <<<"$test_output" || {
  echo "ENOSPC drill did not run exactly one passing test" >&2
  exit 1
}
