#!/usr/bin/env python3
"""Round trips against isolated MinIO using an already-built decay binary.

Requires Python 3, Docker, AWS CLI, and the selected MinIO image. Run:
    python3 tests/real_s3.py target/release/decay
No AWS account is used. The container, bucket, and server are removed on exit.
"""

import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import uuid


PART_SIZE = 8 * 1024 * 1024
BLOCK = bytes(range(256)) * 256
BUCKET = "decay-integration"
TEAM = "integration-team"
ACCESS_KEY = "decay-local-test"
SECRET_KEY = "decay-local-test-secret"


def run(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def chunks(size):
    while size:
        chunk = BLOCK[: min(size, len(BLOCK))]
        yield chunk
        size -= len(chunk)


def wait_until(predicate, message, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError(message)


def healthy(port, path):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=1)
    try:
        connection.request("GET", path)
        return connection.getresponse().status == 200
    except OSError:
        return False
    finally:
        connection.close()


def round_trip(port, s3, size, chunked):
    mode = "chunked" if chunked else "fixed"
    artifact = f"{mode}-{size}"
    path = f"/v8/artifacts/{artifact}?slug={TEAM}"
    metadata = {"x-artifact-tag": f"signature-{artifact}", "x-artifact-duration": "1234"}
    headers = dict(metadata)
    if not chunked:
        headers["Content-Length"] = str(size)
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=30)
    try:
        connection.request("PUT", path, body=chunks(size), headers=headers, encode_chunked=chunked)
        response = connection.getresponse()
        body = response.read()
        assert response.status == 201, (artifact, response.status, body)
        for method in ("GET", "HEAD"):
            connection.request(method, path)
            response = connection.getresponse()
            assert response.status == 200, (artifact, method, response.status)
            for key, value in metadata.items():
                assert response.getheader(key) == value, (artifact, method, key)
            if method == "GET":
                expected = hashlib.sha256()
                for chunk in chunks(size):
                    expected.update(chunk)
                actual = hashlib.sha256()
                count = 0
                while True:
                    chunk = response.read(len(BLOCK))
                    if not chunk:
                        break
                    actual.update(chunk)
                    count += len(chunk)
                assert count == size and actual.digest() == expected.digest(), artifact
            else:
                assert response.read() == b"", artifact
        head = s3("head-object", "--bucket", BUCKET, "--key", f"{TEAM}/{artifact}")
        assert head["ContentLength"] == size, (artifact, head)
        assert head["Metadata"] == metadata, (artifact, head)
        print(f"PASS {mode}: {size} bytes, GET/HEAD headers and S3 metadata", flush=True)
    finally:
        connection.close()


def cancelled_upload(port, s3):
    """Disconnect after a multipart upload exists, while its body is incomplete."""
    artifact = "cancelled-stream"
    path = f"/v8/artifacts/{artifact}?slug={TEAM}"

    def uploads():
        return s3("list-multipart-uploads", "--bucket", BUCKET).get("Uploads", [])

    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=30)
    try:
        connection.putrequest("PUT", path)
        connection.putheader("Transfer-Encoding", "chunked")
        connection.endheaders()
        for chunk in chunks(PART_SIZE + 1):
            connection.send(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
        wait_until(lambda: bool(uploads()), "Multipart upload never started")
    finally:
        connection.close()
    wait_until(lambda: not uploads(), "Disconnected upload was not aborted")
    objects = s3("list-objects-v2", "--bucket", BUCKET, "--prefix", f"{TEAM}/{artifact}")
    assert not objects.get("Contents"), objects
    print("PASS disconnect: active multipart upload aborted; no completed object", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--image", default="minio/minio:RELEASE.2025-04-22T22-12-26Z")
    parser.add_argument("--checksum-mode", choices=("when_required", "when_supported"),
                        default="when_required")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    name = f"decay-s3-test-{uuid.uuid4().hex[:12]}"
    server = None
    try:
        run("docker", "run", "--detach", "--rm", "--name", name,
            "--publish", "127.0.0.1::9000",
            "--env", f"MINIO_ROOT_USER={ACCESS_KEY}",
            "--env", f"MINIO_ROOT_PASSWORD={SECRET_KEY}", args.image, "server", "/data")
        s3_port = int(run("docker", "port", name, "9000/tcp").rsplit(":", 1)[1])
        wait_until(lambda: healthy(s3_port, "/minio/health/live"), "MinIO failed to start")
        endpoint = f"http://127.0.0.1:{s3_port}"
        with tempfile.TemporaryDirectory(prefix="decay-real-s3-") as directory:
            # Explicit local credentials and empty config prevent use of host AWS profiles.
            env = dict(os.environ)
            for key in list(env):
                if key.startswith(("AWS_", "S3_", "TURBO_")):
                    del env[key]
            env.update(AWS_ACCESS_KEY_ID=ACCESS_KEY, AWS_SECRET_ACCESS_KEY=SECRET_KEY,
                       AWS_DEFAULT_REGION="us-east-1", AWS_EC2_METADATA_DISABLED="true",
                       AWS_MAX_ATTEMPTS="1",
                       AWS_CONFIG_FILE=f"{directory}/config",
                       AWS_SHARED_CREDENTIALS_FILE=f"{directory}/credentials", AWS_PAGER="")

            def s3(*arguments):
                output = run("aws", "--endpoint-url", endpoint, "--region", "us-east-1",
                             "--cli-connect-timeout", "5", "--cli-read-timeout", "5",
                             "--output", "json", "s3api", *arguments, env=env)
                return json.loads(output) if output else {}

            s3("create-bucket", "--bucket", BUCKET)
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                server_port = listener.getsockname()[1]
            env.update(HOST="127.0.0.1", PORT=str(server_port), S3_ENDPOINT=endpoint,
                       S3_REGION="us-east-1", S3_BUCKET_NAME=BUCKET, S3_USE_PATH_STYLE="true",
                       S3_ACCESS_KEY=ACCESS_KEY, S3_SECRET_KEY=SECRET_KEY,
                       S3_CHECKSUM_MODE=args.checksum_mode,
                       OTEL_SDK_DISABLED="true")
            with open(f"{directory}/server.log", "w+") as log:
                server = subprocess.Popen([str(binary)], cwd=directory, env=env, stdout=log,
                                          stderr=subprocess.STDOUT)
                try:
                    wait_until(lambda: healthy(server_port, "/management/health"), "Server failed to start")
                    for chunked in (False, True):
                        for size in (0, 17, PART_SIZE - 1, PART_SIZE, PART_SIZE + 1,
                                     8_715_039, 2 * PART_SIZE + 17):
                            round_trip(server_port, s3, size, chunked)
                    cancelled_upload(server_port, s3)
                    assert not s3("list-multipart-uploads", "--bucket", BUCKET).get("Uploads")
                    print(f"All 14 round trips and multipart disconnect cleanup passed "
                          f"(checksum mode: {args.checksum_mode}).", flush=True)
                except BaseException:
                    log.seek(0)
                    print(log.read(), flush=True)
                    raise
                finally:
                    server.terminate()
                    try:
                        server.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                    server = None
    finally:
        if server is not None:
            server.kill()
            server.wait()
        subprocess.run(["docker", "rm", "--force", "--volumes", name], check=False,
                       stdout=subprocess.DEVNULL)


if __name__ == "__main__":
    main()
