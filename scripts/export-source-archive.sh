#!/bin/bash
# Export the committed tree as a reproducible source archive with provenance.
set -Eeuo pipefail

for tool in git tar zip shasum python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "Required tool not found: $tool" >&2
    exit 2
  }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Tracked working-tree changes must be committed before export." >&2
  exit 2
fi

commit="$(git rev-parse HEAD)"
tree="$(git rev-parse 'HEAD^{tree}')"
short="$(git rev-parse --short HEAD)"
output="${1:-/Users/tourist/Documents/fieldbook-${short}.zip}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

git archive "$commit" | tar -x -C "$work_dir"
file_count="$(find "$work_dir" -type f | wc -l | tr -d ' ')"

content_hash="$(
  cd "$work_dir" &&
    find . -type f -print0 |
      sort -z |
      xargs -0 shasum -a 256 |
      shasum -a 256 |
      awk '{print $1}'
)"

python3 - "$work_dir/SOURCE_PROVENANCE.json" "$commit" "$tree" "$file_count" "$content_hash" <<'PY'
import json
import sys
from datetime import datetime, timezone

path, commit, tree, count, content_hash = sys.argv[1:6]
record = {
    "schema_version": 1,
    "repository": "https://github.com/siebensterne/fieldbook",
    "source_commit": commit,
    "source_tree": tree,
    "file_count": int(count),
    "tracked_content_sha256": content_hash,
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "archive_sha256": "recorded in the sibling .sha256 file",
    "note": "All files except this manifest are the exact committed tree at source_commit; the archive digest is written to <archive>.sha256 to avoid a self-referential hash.",
}
with open(path, "w", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2)
    handle.write("\n")
PY

rm -f "$output" "$output.sha256"
(cd "$work_dir" && zip -q -r "$output" .)
shasum -a 256 "$output" | awk '{print $1}' > "$output.sha256"

echo "archive: $output"
echo "commit:  $commit"
echo "tree:    $tree"
echo "files:   $file_count"
echo "sha256:  $(cat "$output.sha256")"
