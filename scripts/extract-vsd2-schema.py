#!/usr/bin/env python3
"""Recovers the vsd2 protobuf schema from a Stream Deck desktop binary.

Why: the desktop app embeds serialized FileDescriptorProto blobs for the
protocol it speaks with Stream Deck Mobile. Reading that blob gives exact field
names and numbers, which is the difference between guessing and implementing.

Usage:
    python scripts/extract-vsd2-schema.py tmp/streamdeck/extracted/.../StreamDeck.exe

Requires the `protobuf` Python package. Output is a reconstructed `.proto` in
stdout, plus a short report on stderr.
"""

from __future__ import annotations

import sys
from pathlib import Path

from google.protobuf import descriptor_pb2

TARGET = b"vsd2.proto"


def read_varint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data):
            raise ValueError("truncated varint")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7
        if shift > 63:
            raise ValueError("varint too long")


def skip_field(data: bytes, offset: int, wire_type: int) -> int:
    if wire_type == 0:
        _, offset = read_varint(data, offset)
        return offset
    if wire_type == 1:
        return offset + 8
    if wire_type == 2:
        length, offset = read_varint(data, offset)
        return offset + length
    if wire_type == 5:
        return offset + 4
    raise ValueError(f"unsupported wire type {wire_type}")


def frame_end(data: bytes, start: int, limit: int = 64 * 1024) -> int:
    """Walks a protobuf message and returns the offset where it stops making sense."""
    offset = start
    while offset < len(data) and offset - start < limit:
        tag_start = offset
        try:
            tag, offset = read_varint(data, offset)
        except ValueError:
            return tag_start
        field_number = tag >> 3
        wire_type = tag & 0x7
        if field_number == 0 or field_number > 1000:
            return tag_start
        try:
            offset = skip_field(data, offset, wire_type)
        except ValueError:
            return tag_start
        if offset > len(data):
            return tag_start
    return offset


def candidates(data: bytes, name_offset: int) -> list[int]:
    starts = []
    for back in range(1, 6):
        start = name_offset - back
        if start < 0 or data[start] != 0x0A:
            continue
        try:
            length, after = read_varint(data, start + 1)
        except ValueError:
            continue
        if after == name_offset and length == len(TARGET):
            starts.append(start)
    return starts


def render(descriptor: descriptor_pb2.FileDescriptorProto) -> str:
    lines: list[str] = ['syntax = "proto3";', "", f"package {descriptor.package};", ""]

    def emit_message(message: descriptor_pb2.DescriptorProto, indent: int) -> None:
        pad = "  " * indent
        lines.append(f"{pad}message {message.name} {{")
        for field in message.field:
            type_name = field.type_name.split(".")[-1] if field.type_name else ""
            type_text = type_name or descriptor_pb2.FieldDescriptorProto.Type.Name(field.type).replace("TYPE_", "").lower()
            label = "repeated " if field.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED else ""
            lines.append(f"{pad}  {label}{type_text} {field.name} = {field.number};")
        for nested in message.nested_type:
            emit_message(nested, indent + 1)
        lines.append(f"{pad}}}")
        lines.append("")

    for message in descriptor.message_type:
        emit_message(message, 0)
    return "\n".join(lines)


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    data = path.read_bytes()
    print(f"reading {path} ({len(data)} bytes)", file=sys.stderr)

    offset = 0
    found = 0
    while True:
        index = data.find(TARGET, offset)
        if index == -1:
            break
        offset = index + 1
        for start in candidates(data, index):
            end = frame_end(data, start)
            blob = data[start:end]
            descriptor = descriptor_pb2.FileDescriptorProto()
            try:
                descriptor.ParseFromString(blob)
            except Exception:
                continue
            if descriptor.package != "vsd2" or not descriptor.message_type:
                continue
            found += 1
            print(f"descriptor at {start}, {len(blob)} bytes, {len(descriptor.message_type)} messages", file=sys.stderr)
            print(render(descriptor))
            return 0

    if not found:
        print("no vsd2 descriptor found", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
