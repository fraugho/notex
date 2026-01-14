# notex

A fast, parallel note processor that uses local or cloud LLMs to categorize, enhance, and organize your notes.

## Features

- **Automatic categorization** - Extracts segments from notes and assigns categories/subcategories
- **Enhancement** - Fixes typos, adds missing equations, answers questions marked with `?`, suggests resources
- **Cross-filing** - Duplicates content that belongs in multiple categories
- **Reorganization pass** - Second pass to optimize file structure (move files to better locations, create subcategories)
- **Cross-referencing** - Links related notes together
- **Parallel processing** - Fully utilizes multi-slot inference servers (e.g., llama.cpp with `-np 32`)
- **Dual output format** - Markdown or plain text
- **Dry run mode** - Preview categorization before processing

## Installation

```bash
cargo build --release
```

## Usage

```bash
notex [OPTIONS] <INPUT_DIR>
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output <DIR>` | Output directory | `./compressed` |
| `-m, --model <MODEL>` | Model name | `gpt-3.5-turbo` |
| `-u, --url <URL>` | API base URL | `http://localhost:8080/v1` |
| `-k, --api-key <KEY>` | API key | `sk-no-key-required` |
| `-p, --parallel <N>` | Max concurrent requests | `8` |
| `-f, --format <FMT>` | Output format: `markdown` or `plain` | `markdown` |
| `-x, --exclude <PATTERN>` | Exclude glob patterns (repeatable) | |
| `--retries <N>` | Retry failed LLM calls | `3` |
| `--dry-run` | Preview categorization only | |
| `--reorganize` | Run reorganization pass | |
| `--cross-ref` | Add cross-references | |
| `-v, --verbose` | Verbose output | |

### Examples

**With local llama.cpp server:**

```bash
# Start llama-server with parallel slots
llama-server -m model.gguf -c 8192 -np 8 -cb

# Run notex
notex ./notes -o ./output -p 8 -m local-model -u http://localhost:8080/v1
```

**With OpenAI:**

```bash
notex ./notes -o ./output -m gpt-4o -u https://api.openai.com/v1 -k sk-your-key
```

**Dry run to preview:**

```bash
notex ./notes --dry-run
```

**Full processing with reorganization:**

```bash
notex ./notes -o ./output --reorganize --cross-ref
```

**Exclude patterns:**

```bash
notex ./notes -x "*.tmp" -x "drafts/*"
```

## How It Works

1. **Discovery** - Recursively scans input directory for notes
2. **Categorization** - LLM extracts segments and suggests categories/paths
3. **Enhancement** - LLM improves each segment (fixes errors, adds equations, answers `?` markers)
4. **Output** - Writes organized files to output directory
5. **Reorganization** (optional) - LLM reviews structure and moves files to better locations
6. **Cross-referencing** (optional) - LLM identifies related notes and adds links

## Categories

The following categories are available (LLM can also suggest custom ones):

- **Sciences**: mathematics, statistics, physics, chemistry, biology, computer_science
- **Applied**: machine_learning, engineering, finance
- **Humanities**: philosophy, history, literature, languages
- **Personal**: journal, ideas, todo
- **Media**: books, videos, articles, podcasts
- **Misc**: reference, links, uncategorized

## Question Markers

Notes containing `?` markers (e.g., `?logistic loss`) are treated as questions. The LLM will:
1. Attempt to answer or provide direction
2. Preserve the original question using format: `[Q: original question] Answer...`

## License

MIT
