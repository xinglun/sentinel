#!/usr/bin/env python3
"""Validate the language-neutral Trust Layer schema contracts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCHEMA_DIR = Path(__file__).resolve().parents[1] / ".ai" / "trust" / "schema"
SCHEMA_NAMES = {
    "approval",
    "repository_capabilities",
    "success_criteria",
    "human_decision_request",
    "human_decision_evidence",
    "baseline_evidence",
}


class ValidationError(ValueError):
    """Raised when an instance does not satisfy a Trust Layer schema."""


def _path(path: str, fragment: str) -> str:
    return f"{path}.{fragment}" if path else fragment


def _type_matches(value: Any, expected: str | list[str]) -> bool:
    if isinstance(expected, list):
        return any(_type_matches(value, option) for option in expected)
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    raise ValidationError(f"unsupported schema type {expected!r}")


def validate(instance: Any, schema: dict[str, Any], path: str = "$", *, root: bool = True) -> None:
    """Validate the supported strict JSON Schema subset used by Trust Layer records."""
    if "const" in schema and instance != schema["const"]:
        raise ValidationError(f"{path}: expected constant {schema['const']!r}")
    if "enum" in schema and instance not in schema["enum"]:
        raise ValidationError(f"{path}: expected one of {schema['enum']!r}")

    expected = schema.get("type")
    if expected is not None and not _type_matches(instance, expected):
        raise ValidationError(f"{path}: expected {expected}, got {type(instance).__name__}")

    if isinstance(instance, dict):
        required = schema.get("required", [])
        for key in required:
            if key not in instance:
                raise ValidationError(f"{_path(path, key)}: required field is missing")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            unknown = sorted(set(instance) - set(properties))
            if unknown:
                raise ValidationError(f"{path}: unknown field(s): {', '.join(unknown)}")
        for key, value in instance.items():
            if key in properties:
                validate(value, properties[key], _path(path, key), root=False)

    if isinstance(instance, list):
        minimum = schema.get("minItems")
        if minimum is not None and len(instance) < minimum:
            raise ValidationError(f"{path}: requires at least {minimum} item(s)")
        if schema.get("uniqueItems") and len(
            {json.dumps(item, sort_keys=True) for item in instance}
        ) != len(instance):
            raise ValidationError(f"{path}: items must be unique")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, value in enumerate(instance):
                validate(value, item_schema, f"{path}[{index}]", root=False)

    if isinstance(instance, str):
        minimum = schema.get("minLength")
        if minimum is not None and len(instance) < minimum:
            raise ValidationError(f"{path}: requires at least {minimum} character(s)")


def load_schemas(schema_dir: Path = SCHEMA_DIR) -> dict[str, dict[str, Any]]:
    """Load the five checked-in Trust Layer schema documents by stable name."""
    schemas: dict[str, dict[str, Any]] = {}
    for path in sorted(schema_dir.glob("*.schema.json")):
        name = path.name.removesuffix(".schema.json")
        if name not in SCHEMA_NAMES:
            raise ValidationError(f"unexpected Trust Layer schema file: {path.name}")
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise ValidationError(f"cannot load {path}: {exc}") from exc
        if not isinstance(data, dict):
            raise ValidationError(f"{path}: schema document must be an object")
        schemas[name] = data
    missing = sorted(SCHEMA_NAMES - set(schemas))
    if missing:
        raise ValidationError(f"missing Trust Layer schema(s): {', '.join(missing)}")
    return schemas


def validate_schema_documents(schema_dir: Path = SCHEMA_DIR) -> dict[str, dict[str, Any]]:
    """Validate schema metadata and every embedded example, returning loaded schemas."""
    schemas = load_schemas(schema_dir)
    for name, schema in schemas.items():
        for key in (
            "$schema",
            "$id",
            "title",
            "type",
            "required",
            "properties",
            "additionalProperties",
            "examples",
        ):
            if key not in schema:
                raise ValidationError(f"{name}: schema metadata field {key!r} is required")
        if schema["type"] != "object" or schema["additionalProperties"] is not False:
            raise ValidationError(f"{name}: root schema must be a strict object")
        examples = schema["examples"]
        if not isinstance(examples, list) or not examples:
            raise ValidationError(f"{name}: examples must contain at least one object")
        for index, example in enumerate(examples):
            validate(example, schema, f"{name}.examples[{index}]")
    return schemas


def validate_payload(name: str, payload: Any, schema_dir: Path = SCHEMA_DIR) -> None:
    """Validate one payload against a named Trust Layer schema."""
    schemas = validate_schema_documents(schema_dir)
    if name not in schemas:
        raise ValidationError(f"unknown Trust Layer schema: {name}")
    validate(payload, schemas[name])


def main(argv: list[str] | None = None) -> int:
    """Run the schema suite or validate one JSON payload from the command line."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=SCHEMA_DIR)
    parser.add_argument("--check", action="store_true", help="validate all schemas and examples")
    parser.add_argument("--schema", choices=sorted(SCHEMA_NAMES))
    parser.add_argument("--input", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.check:
            validate_schema_documents(args.schema_dir)
            print(f"Trust Layer schema check passed: {len(SCHEMA_NAMES)} schema(s)")
            return 0
        if args.schema and args.input:
            payload = json.loads(args.input.read_text(encoding="utf-8"))
            validate_payload(args.schema, payload, args.schema_dir)
            print(f"Trust Layer payload valid: {args.schema}")
            return 0
        parser.error("use --check or provide both --schema and --input")
    except (OSError, json.JSONDecodeError, ValidationError) as exc:
        print(f"Trust Layer schema check failed: {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
