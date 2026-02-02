#!/usr/bin/env python3
"""
Bridge script for unrpyc integration.
This script provides a simplified interface for decompiling RPYC files.
"""

import sys
import json
from pathlib import Path

# Add vendor/unrpyc to path
SCRIPT_DIR = Path(__file__).parent.absolute()
VENDOR_DIR = SCRIPT_DIR.parent / "vendor" / "unrpyc"
sys.path.insert(0, str(VENDOR_DIR))

try:
    import decompiler
    from decompiler.renpycompat import pickle_safe_loads
    import zlib
    import struct
except ImportError as e:
    print(json.dumps({"error": f"Failed to import unrpyc: {e}"}))
    sys.exit(1)


def read_ast_from_file(in_file):
    """Read AST from RPYC file."""
    raw_contents = in_file.read()

    if not raw_contents.startswith(b"RENPY RPC2"):
        # RPYC V1: just compressed pickle
        contents = raw_contents
    else:
        # RPYC V2: parse archive structure
        position = 10
        chunks = {}

        for expected_slot in range(1, 0xFFFFFFFF):
            slot, start, length = struct.unpack(
                "III", raw_contents[position : position + 12]
            )

            if slot == 0:
                break

            position += 12
            chunks[slot] = raw_contents[start : start + length]

        if 1 not in chunks:
            raise ValueError("Unable to find data slot in RPYC file")

        contents = chunks[1]

    contents = zlib.decompress(contents)
    _, stmts = pickle_safe_loads(contents)
    return stmts


def decompile_file(input_path: Path, output_path: Path) -> dict:
    """Decompile a single RPYC file."""
    result = {"input": str(input_path), "output": str(output_path), "success": False}

    try:
        with open(input_path, "rb") as f:
            ast = read_ast_from_file(f)

        output_path.parent.mkdir(parents=True, exist_ok=True)

        with open(output_path, "w", encoding="utf-8") as out_file:
            options = decompiler.Options()
            decompiler.pprint(out_file, ast, options)

        result["success"] = True

    except Exception as e:
        result["error"] = str(e)

    return result


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Usage: decompile.py <input> [output]"}))
        sys.exit(1)

    input_path = Path(sys.argv[1])
    
    if len(sys.argv) >= 3:
        output_path = Path(sys.argv[2])
    else:
        # Default output: same name with .rpy extension
        if input_path.suffix == ".rpyc":
            output_path = input_path.with_suffix(".rpy")
        elif input_path.suffix == ".rpymc":
            output_path = input_path.with_suffix(".rpym")
        else:
            output_path = input_path.with_suffix(".rpy")

    result = decompile_file(input_path, output_path)
    print(json.dumps(result))
    sys.exit(0 if result["success"] else 1)


if __name__ == "__main__":
    main()
