#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
base_url=${KNOWLEDGEOS_BASE_URL:-http://127.0.0.1:3000}
relative_path=_docker_smoke.md
host_path="${repository_root}/knowledge/${relative_path}"
temporary_directory=$(mktemp -d)

cleanup() {
  rm -f "${host_path}"
  rm -rf "${temporary_directory}"
}
trap cleanup EXIT

rm -f "${host_path}"

health_payload=$(curl --fail --silent --show-error "${base_url}/api/health")
python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["status"] == "ok"' <<<"${health_payload}"

create_status=$(curl --silent --show-error \
  --output "${temporary_directory}/create.json" \
  --write-out '%{http_code}' \
  --header 'content-type: application/json' \
  --data '{"path":"_docker_smoke.md","content":"# Docker Smoke\n"}' \
  "${base_url}/api/files")
test "${create_status}" = "201"

base_hash=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["hash"])' "${temporary_directory}/create.json")

read_payload=$(curl --fail --silent --show-error "${base_url}/api/files/${relative_path}")
python3 -c 'import json,sys; payload=json.load(sys.stdin); assert payload["content"] == "# Docker Smoke\n"' <<<"${read_payload}"

update_status=$(curl --silent --show-error \
  --output "${temporary_directory}/update.json" \
  --write-out '%{http_code}' \
  --request PUT \
  --header 'content-type: application/json' \
  --data "{\"content\":\"# Docker Smoke Updated\\n\",\"base_hash\":\"${base_hash}\"}" \
  "${base_url}/api/files/${relative_path}")
test "${update_status}" = "200"

python3 -c 'import pathlib,sys; assert pathlib.Path(sys.argv[1]).read_text() == "# Docker Smoke Updated\n"' "${host_path}"

echo "KnowledgeOS Docker smoke test passed: health, create, read, update, bind mount"
