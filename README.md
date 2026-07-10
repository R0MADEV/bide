# bide

A deterministic workflow engine that consults specialized agents when it needs
judgement. **The AI reasons; Rust controls.**

*bide* means "path" in Basque — it drives a task down an explicit path of steps
(plan → implement → verify → review) and stops for you at the checkpoints that
matter. The core is deterministic Rust; agents produce plans, critiques and
reviews but never control the flow. There is **no leader agent** — the engine is
the leader.

```
you → bide → engine (Rust) → step by step:
        context (lexis) → plan → critic → implement (Claude Code) → verify → diff → review
                                                                              → branch + commit + PR
```

## Install

```sh
cargo install --path .      # puts `bide` on your PATH (~/.cargo/bin)
bide doctor                 # checks git / claude / lexis / gh and your bide.toml
```

External tools bide drives (only what you use is required): **git**, **claude**
(Claude Code — does the editing), **lexis** (code search, optional), **gh**
(pull requests, optional).

## Use it

```sh
bide                        # interactive workspace: type a task, or a question
bide run "add jwt auth"     # run once, line-based output
bide tui "add jwt auth"     # run a task in the terminal UI
bide doctor | bide help
```

In the interactive workspace you just type. **bide decides** whether it is a
task (runs the workflow) or a question about the code (answers it with
Claude + lexis) — no prefix needed.

### Flags (each also has a `BIDE_*` env var)

| flag | env | meaning |
|---|---|---|
| `--yes`, `-y` | `BIDE_YES=1` | run straight through, no interactive checkpoints |
| `--agent <name>` | `BIDE_AGENT` | reasoning backend: `claude` \| `stub` (else `[agent]` in bide.toml) |
| `--context <name>` | `BIDE_CONTEXT` | code context: `claude` (Claude Code + lexis) \| `lexis` |
| `--branch` | `BIDE_BRANCH=1` | move the run's changes onto a `bide/<slug>` branch + commit |
| `--pr` | `BIDE_PR=1` | push the branch and open a pull request with `gh` |
| `--resume <id>` | — | continue a previous run from where it stopped |

## Who does what

- **Reasoning** (`plan`, `critic`, `review`, and answering questions) → the
  configured agent: the **Claude CLI**, **OpenAI (GPT)** or **Anthropic API**.
- **Editing code** (`implement`) → **Claude Code**. The OpenAI/Anthropic APIs
  return text; they cannot touch your files — Claude Code does the actual edits.
- **Context** (`--context claude`) → Claude Code uses the **lexis** tools to
  fetch the real code relevant to the task, which the reasoning agents then see.
- **Everything else** — flow, retries, checkpoints, branch, verify — is Rust.

## Configure (`bide.toml`)

Everything is optional. Without a `bide.toml`, bide uses a sensible default
recipe and detects the test command (`Cargo.toml` → `cargo test`, etc.).

```toml
# Reasoning backend for plan/critic/review. Give the API key either securely
# via api_key_env (the NAME of an env var) or directly via api_key.
[agent]
provider = "openai"          # openai | anthropic
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
# api_key = "sk-..."         # alternative: inline (bide.toml is gitignored)
max_tokens = 4096

# Extra security rules ON TOP of the built-ins (rm -rf, .env, ssh keys, ...).
[policy]
deny_commands = ["terraform destroy"]
secret_paths = ["config/master.key"]

# Override the external tool binaries (default: the plain name on PATH).
[tools]
claude = "claude"

# The workflow: an ordered, composable list of steps.
[workflow]
max_retries = 3

[[workflow.step]]
name = "plan"
on_failure = "abort"
pause = true                 # a checkpoint: bide stops so you can review/re-plan

[[workflow.step]]
name = "critic"
on_failure = { retry_from = "plan" }     # reject → re-plan

[[workflow.step]]
name = "implement"
on_failure = "abort"

[[workflow.step]]
name = "verify"
command = "cargo test"       # steps with a command run it behind the policy gate
on_failure = { retry_from = "implement" }

[[workflow.step]]
name = "diff"
command = "git diff"         # feeds the reviewer the changes via the blackboard

[[workflow.step]]
name = "review"
on_failure = { retry_from = "implement" }
```

See [`bide.example.toml`](bide.example.toml) for the annotated template.

## Safety

- **Interactive by default.** Steps marked `pause = true` stop so you review the
  plan/diff and choose continue / re-plan (with feedback) / abort. `--yes` skips.
- **Policy Engine** (untouchable): every command runs behind a gate that blocks
  `rm -rf`, `git reset --hard`, reading secrets (`.env`, ssh keys), etc.
- **Isolated changes.** `--branch` keeps `main` clean; a run only branches from a
  clean tree and only when it actually produced changes.

## Artifacts

Each run that changed something (or failed) is recorded under `.bide/runs/<id>/`:

```
context.md   the code lexis/Claude captured for the run
report.md    task · per-step outcome + the prompt sent to the AI · diff · result
steps/       each step's output on its own
state.json   for --resume
```

No-op runs are not saved, and old runs are pruned automatically. `.bide/` is
gitignored.

## Development

Built test-first (strict TDD), minimal code, no speculative modules. Ports
(traits) everywhere so real tools plug in and fakes drive the tests.

```sh
just check   # fmt + clippy -D warnings + test
```

Modules: `core` (engine/state/task), `dispatch`, `agents`, `context`, `tools`,
`policy`, `git`, `report`, `board`, `config`, `doctor`, `detect`, `route`,
`exec`, `tui`, `cli`. See [docs/architecture.md](docs/architecture.md).
