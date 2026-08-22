#!/usr/bin/env python3
"""Extract the sole executable from one exact content-addressed scratch image."""

import hashlib
import gzip
import io
import json
import os
import re
import select
import signal
import subprocess
import sys
import tarfile
import tempfile
import time

MAX_IMAGE_ARCHIVE_BYTES = 160 * 1024 * 1024
MAX_EXECUTABLE_BYTES = 128 * 1024 * 1024
MAX_OUTER_MEMBERS = 16
DEADLINE_SECONDS = 120


def refuse(message: str) -> "NoReturn":
    raise SystemExit(message)


def read_member(archive: tarfile.TarFile, member: tarfile.TarInfo, maximum: int) -> bytes:
    if not member.isfile() or member.size <= 0 or member.size > maximum:
        refuse(f"invalid bounded image member: {member.name}")
    stream = archive.extractfile(member)
    if stream is None:
        refuse(f"image member was unreadable: {member.name}")
    value = stream.read(maximum + 1)
    if len(value) != member.size or len(value) > maximum:
        refuse(f"image member exceeded its bound: {member.name}")
    return value


def save_exact_image(image_id: str, destination: str) -> None:
    process = subprocess.Popen(
        ["docker", "image", "save", image_id],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    assert process.stdout is not None
    deadline = time.monotonic() + DEADLINE_SECONDS
    total = 0
    try:
        descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as output:
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    refuse("exact image save exceeded its deadline")
                ready, _, _ = select.select([process.stdout], [], [], min(remaining, 1.0))
                if not ready:
                    if process.poll() is not None:
                        break
                    continue
                chunk = os.read(process.stdout.fileno(), 65_536)
                if not chunk:
                    break
                total += len(chunk)
                if total > MAX_IMAGE_ARCHIVE_BYTES:
                    refuse("exact image save exceeded its byte bound")
                output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
        if process.wait(timeout=max(1.0, deadline - time.monotonic())) != 0 or total == 0:
            refuse("exact image save failed")
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def exact_outer_members(archive: tarfile.TarFile) -> dict[str, tarfile.TarInfo]:
    result = {}
    count = 0
    for member in archive:
        count += 1
        if count > MAX_OUTER_MEMBERS:
            refuse("exact image archive had an invalid member count")
        if member.name.startswith("/") or ".." in member.name.split("/"):
            refuse("exact image archive contained an unsafe path")
        if member.issym() or member.islnk() or member.isdev():
            refuse("exact image archive contained an unsupported member")
        if member.isfile():
            if member.name in result:
                refuse("exact image archive contained a duplicate member")
            result[member.name] = member
        elif not member.isdir():
            refuse("exact image archive contained an unsupported member")
    if count < 3:
        refuse("exact image archive had an invalid member count")
    return result


def extract(image_id: str, destination: str, archive_path: str) -> tuple[int, str]:
    expected_image_sha = image_id.removeprefix("sha256:")
    with tarfile.open(archive_path, mode="r:") as outer:
        members = exact_outer_members(outer)
        for required in ("manifest.json", "index.json", "oci-layout"):
            if required not in members:
                refuse("exact OCI image archive metadata was incomplete")
        if json.loads(read_member(outer, members["oci-layout"], 128)) \
                != {"imageLayoutVersion": "1.0.0"}:
            refuse("exact OCI image layout version was invalid")

        def blob(descriptor: dict, maximum: int) -> tuple[str, bytes]:
            if set(descriptor) - {"mediaType", "digest", "size", "annotations", "platform"}:
                refuse("exact OCI descriptor had unknown fields")
            digest = descriptor.get("digest")
            size = descriptor.get("size")
            if not isinstance(digest, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", digest) \
                    or not isinstance(size, int) or size <= 0 or size > maximum:
                refuse("exact OCI descriptor exceeded its bound")
            name = "blobs/sha256/" + digest.removeprefix("sha256:")
            if name not in members:
                refuse("exact OCI descriptor blob was absent")
            value = read_member(outer, members[name], maximum)
            if len(value) != size or hashlib.sha256(value).hexdigest() != digest.removeprefix("sha256:"):
                refuse("exact OCI descriptor digest disagreed")
            return name, value

        index = json.loads(read_member(outer, members["index.json"], 16_384))
        descriptors = index.get("manifests") if isinstance(index, dict) else None
        if index.get("schemaVersion") != 2 or not isinstance(descriptors, list) \
                or len(descriptors) != 1:
            refuse("exact OCI index did not describe one requested engine image")
        image_name, image_bytes = blob(descriptors[0], 65_536)
        image_record = json.loads(image_bytes)
        expected_blobs = {image_name}
        if image_record.get("mediaType") == "application/vnd.oci.image.index.v1+json":
            candidates = [
                descriptor for descriptor in image_record.get("manifests", [])
                if descriptor.get("platform") == {"architecture": "arm64", "os": "linux"}
            ]
            if len(candidates) != 1:
                refuse("exact OCI image index did not select one linux/arm64 subject")
            manifest_name, manifest_bytes = blob(candidates[0], 65_536)
            expected_blobs.add(manifest_name)
            image_manifest = json.loads(manifest_bytes)
        else:
            image_manifest = image_record
        if image_manifest.get("schemaVersion") != 2 \
                or image_manifest.get("mediaType") not in {
                    "application/vnd.oci.image.manifest.v1+json",
                    "application/vnd.docker.distribution.manifest.v2+json",
                } \
                or not isinstance(image_manifest.get("layers"), list) \
                or len(image_manifest["layers"]) != 1:
            refuse("exact OCI image manifest did not describe one layer")
        config_name, config_bytes = blob(image_manifest.get("config", {}), 65_536)
        config_digest = image_manifest.get("config", {}).get("digest")
        if descriptors[0].get("digest") != image_id and config_digest != image_id:
            refuse("exact OCI archive did not bind the requested engine image identity")
        layer_name, layer_bytes = blob(image_manifest["layers"][0], MAX_IMAGE_ARCHIVE_BYTES)
        expected_blobs.update((config_name, layer_name))

        legacy = json.loads(read_member(outer, members["manifest.json"], 16_384))
        if not isinstance(legacy, list) or len(legacy) != 1:
            refuse("exact image manifest did not describe one image")
        record = legacy[0]
        if set(record) != {"Config", "RepoTags", "Layers"}:
            refuse("exact image manifest shape was not canonical")
        tags = record["RepoTags"]
        if tags not in (None, []) and (
            not isinstance(tags, list) or len(tags) != 1 or not isinstance(tags[0], str)
            or len(tags[0]) > 256 or not re.fullmatch(r"[A-Za-z0-9._:/-]+", tags[0])
        ):
            refuse("exact image archive tags exceeded their bound")
        if record["Config"] != config_name or record["Layers"] != [layer_name]:
            refuse("exact image manifest was not bound to the requested image")
        metadata = {"manifest.json", "index.json", "oci-layout"}
        if set(members) != metadata | expected_blobs:
            refuse("exact scratch image archive contained unexpected files")
        config = json.loads(config_bytes)
        if config.get("rootfs", {}).get("type") != "layers" \
                or len(config.get("rootfs", {}).get("diff_ids", [])) != 1:
            refuse("exact scratch image config did not bind one layer")
        diff_id = config["rootfs"]["diff_ids"][0]
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", diff_id):
            refuse("exact scratch image diff identity was invalid")

    if layer_bytes.startswith(b"\x1f\x8b"):
        with gzip.GzipFile(fileobj=io.BytesIO(layer_bytes)) as compressed:
            layer_tar = compressed.read(MAX_IMAGE_ARCHIVE_BYTES + 1)
    else:
        layer_tar = layer_bytes
    if len(layer_tar) > MAX_IMAGE_ARCHIVE_BYTES \
            or hashlib.sha256(layer_tar).hexdigest() != diff_id.removeprefix("sha256:"):
        refuse("exact scratch image layer disagreed with its diff identity")

    layer_path = archive_path + ".layer"
    descriptor = os.open(layer_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as layer_file:
        layer_file.write(layer_tar)
        layer_file.flush()
        os.fsync(layer_file.fileno())
    try:
        with tarfile.open(layer_path, mode="r:") as layer:
            subject = layer.next()
            if subject is None or layer.next() is not None:
                refuse("exact scratch layer did not contain one subject")
            if subject.name not in {"canary", "./canary"} or not subject.isfile() \
                    or subject.issym() or subject.islnk() or subject.mode & 0o777 != 0o500:
                refuse("exact scratch layer subject was not canonical")
            executable = read_member(layer, subject, MAX_EXECUTABLE_BYTES)
    finally:
        os.unlink(layer_path)
    digest = hashlib.sha256(executable).hexdigest()
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o500)
    with os.fdopen(descriptor, "wb") as output:
        output.write(executable)
        output.flush()
        os.fsync(output.fileno())
    return len(executable), digest


def main() -> None:
    if len(sys.argv) != 3:
        refuse("usage: extract-pinned-image.py SHA256_IMAGE_ID EMPTY_DESTINATION")
    image_id, destination = sys.argv[1:]
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", image_id):
        refuse("exact image identity was not canonical")
    inspected = subprocess.run(
        ["docker", "image", "inspect", image_id, "--format", "{{.Id}}"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        timeout=10,
    ).stdout.strip()
    if inspected != image_id:
        refuse("engine image identity disagreed with its exact subject")
    if os.path.lexists(destination):
        refuse("exact image extraction destination already existed")
    directory = os.path.dirname(os.path.abspath(destination))
    with tempfile.NamedTemporaryFile(prefix=".fava-image-save-", dir=directory, delete=False) as temp:
        archive_path = temp.name
    os.unlink(archive_path)
    try:
        save_exact_image(image_id, archive_path)
        size, digest = extract(image_id, destination, archive_path)
    finally:
        if os.path.exists(archive_path):
            os.unlink(archive_path)
    print(f"subject_image_sha256={image_id.removeprefix('sha256:')}")
    print(f"bytes={size}")
    print(f"sha256={digest}")


if __name__ == "__main__":
    main()
