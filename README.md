# Derenpy

A comprehensive Ren'Py game reverse engineering and translation toolkit.

## Features

- **RPA Unpack** - Extract files from Ren'Py archive files (.rpa)
- **RPA Repack** - Create RPA archives from files
- **RPYC Decompile** - Decompile compiled Ren'Py scripts (.rpyc)
- **AI Translation** - Translate game scripts using LLM APIs
- **Translation Patch** - Generate Ren'Py-compatible translation patches

## Installation

### From Source

```bash
# Clone with submodules
git clone --recursive https://github.com/yourusername/derenpy.git
cd derenpy

# Build
cargo build --release

# The binary will be at target/release/derenpy
```

### Dependencies

- Rust 1.70+
- Python 3.9+ (for RPYC decompilation)

## Usage

### Unpack RPA Archives

```bash
# Unpack a single file
derenpy unpack game.rpa

# Specify output directory
derenpy unpack game.rpa -o ./extracted

# Process directory recursively
derenpy unpack ./game_folder -r -o ./output
```

### Repack into RPA

```bash
# Create RPA from directory
derenpy repack ./extracted -o game.rpa

# Specify RPA version
derenpy repack ./extracted --version 3.0
```

### Decompile RPYC Scripts

```bash
# Decompile a single file
derenpy decompile script.rpyc

# Batch decompile
derenpy decompile ./game/scripts -r -o ./output
```

### AI Translation

```bash
# Translate to Simplified Chinese (default)
derenpy translate script.rpy --api openai

# Specify target language
derenpy translate script.rpy -l ja --api openai

# Use local Ollama
derenpy translate script.rpy --api ollama --model llama3
```

### Generate Translation Patch (Recommended)

The `patch` command generates Ren'Py-compatible translation files that don't modify the original game.

```bash
# Generate translated patch
derenpy patch ./game --api openai -l chinese

# Generate template only (no translation)
derenpy patch ./game -l chinese --template-only

# Translate from RPA directly
derenpy patch game.rpa --api openai -l japanese
```

This creates a `tl/<language>/` directory structure that can be copied directly to the game's `game` folder.

## Complete Translation Workflow

1. **Extract** game files:
   ```bash
   derenpy unpack game.rpa -o ./extracted
   ```

2. **Decompile** scripts (if only .rpyc files exist):
   ```bash
   derenpy decompile ./extracted -r
   ```

3. **Generate translation patch**:
   ```bash
   derenpy patch ./extracted --api openai -l chinese -o ./patch
   ```

4. **Install patch**: Copy `patch/tl/` to `game/tl/` in the original game directory.

5. The game will automatically detect the translation!

## Supported API Providers

| Provider | Environment Variable | Default Model |
|----------|---------------------|---------------|
| OpenAI   | `OPENAI_API_KEY`    | gpt-4o-mini   |
| Claude   | `ANTHROPIC_API_KEY` | claude-sonnet-4-20250514 |
| Ollama   | (none required)     | llama3        |

## Project Structure

```
derenpy/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli/                 # CLI definitions
│   ├── unpack/              # RPA extraction
│   ├── repack/              # RPA creation
│   ├── decompile/           # RPYC decompilation
│   ├── translate/           # AI translation
│   └── patch/               # Translation patch generator
├── scripts/
│   └── decompile.py         # Python bridge for unrpyc
├── vendor/
│   ├── unrpa/               # UnRPA (git submodule)
│   └── unrpyc/              # UnRPYC (git submodule)
└── README.md
```

## License

MIT License

## Credits

- [unrpa](https://github.com/Lattyware/unrpa) - RPA extraction reference
- [unrpyc](https://github.com/CensoredUsername/unrpyc) - RPYC decompilation
