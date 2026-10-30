#!/usr/bin/env bash
# Validate packaging without contacting S3. Usage: smoke-image.sh IMAGE BINARY
set -euo pipefail
image="$1"
binary="$2"
container=$(docker create -p 127.0.0.1::8000 \
  -e S3_ACCESS_KEY=smoke-test -e S3_SECRET_KEY=smoke-test \
  -e S3_ENDPOINT=http://127.0.0.1:9000 "$image")
scratch_dir=$(mktemp -d)
cleanup() {
  docker rm -f "$container" >/dev/null
  rm -rf "$scratch_dir"
}
trap cleanup EXIT
docker cp "$container:/usr/bin/decay" "$scratch_dir/decay"
cmp "$binary" "$scratch_dir/decay"
docker cp "$container:/etc/ssl/certs/ca-certificates.crt" "$scratch_dir/ca-certificates.crt"
test -s "$scratch_dir/ca-certificates.crt"
docker start "$container" >/dev/null
address=$(docker port "$container" 8000/tcp)
for attempt in {1..30}; do
  if curl --fail --silent "http://$address/management/health" >/dev/null; then
    exit 0
  fi
  sleep 1
done
docker logs "$container"
echo 'Runtime health check failed' >&2
exit 1
