# Setup Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `rcc setup` truly one-click capable by keeping interactive onboarding for humans while adding deterministic non-interactive input paths for Claude, smoke tests, and CI.

**Architecture:** Split setup into three layers: input resolution, install execution, and reporting. Interactive prompts become one frontend among several, while `--from-file` JSON and `RCC_SETUP_*` environment overrides feed the same `SetupInput` model and the same write/doctor engine.

**Tech Stack:** Rust (`anyhow`, `serde`, `serde_json`, `dotenvy`, `tokio`), existing `rcc` CLI, Markdown docs

---

## File structure

### Existing files to modify
- `crates/app/src/main.rs` — extend CLI parsing for setup flags and non-interactive options
- `crates/app/src/lib.rs` — add tests for input resolution, precedence, and fail-fast behavior
- `crates/app/src/setup.rs` — refactor setup into input resolution + execution layers, add file/env support, add non-interactive failure handling
- `README.md` — document `--from-file` and automation-friendly setup flow
- `docs/slack-setup.md` — explain JSON/env setup modes for Claude and smoke runs
- `docs/manual-smoke-test.md` — add deterministic setup smoke path

### New files to create
- `docs/setup.example.json` — example JSON payload for `rcc setup --from-file`

### Existing files to check
- `docs/superpowers/specs/2026-04-16-setup-hardening-design.md` — source-of-truth spec
- `data/channel-projects.example.json` — preserve shape alignment with generated mapping output
- `slack/app-manifest.json` — setup still references this exact path and flow

---

### Task 1: Refactor setup around a single resolved input model

**Files:**
- Modify: `crates/app/src/setup.rs`
- Modify: `crates/app/src/lib.rs:340-540`

- [ ] **Step 1: Write the failing test for partial input completion from prompts**

Add a test showing that already-supplied values are not re-prompted.

```rust
#[tokio::test]
async fn resolve_setup_input_only_prompts_for_missing_fields() {
    let partial = SetupInput {
        slack_bot_token: Some("xoxb-ready".into()),
        slack_signing_secret: None,
        slack_app_token: None,
        slack_allowed_user_id: Some("U123".into()),
        channel_id: None,
        project_root: Some("/tmp/project".into()),
        project_label: None,
    };

    let mut prompter = setup::FakePrompter::new(vec![
        setup::FakeAnswer::Secret("signing-secret".into()),
        setup::FakeAnswer::Secret("xapp-app".into()),
        setup::FakeAnswer::Prompt("C123".into()),
        setup::FakeAnswer::Prompt("demo".into()),
    ]);

    let resolved = setup::resolve_setup_input(partial, false, &mut prompter).await.expect("resolve input");

    assert_eq!(resolved.slack_bot_token.as_deref(), Some("xoxb-ready"));
    assert_eq!(resolved.slack_signing_secret.as_deref(), Some("signing-secret"));
    assert_eq!(resolved.channel_id.as_deref(), Some("C123"));
    assert!(!prompter.output().contains("PROMPT:SLACK_BOT_TOKEN"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rcc resolve_setup_input_only_prompts_for_missing_fields -- --exact`
Expected: FAIL because `SetupInput` and `resolve_setup_input` do not exist yet.

- [ ] **Step 3: Add the shared `SetupInput` model**

Implement a single optional-input model in `crates/app/src/setup.rs`.

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SetupInput {
    pub slack_bot_token: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub slack_app_token: Option<String>,
    pub slack_allowed_user_id: Option<String>,
    pub channel_id: Option<String>,
    pub project_root: Option<String>,
    pub project_label: Option<String>,
}
```

- [ ] **Step 4: Add input completion helpers**

Implement helpers that fill only missing fields.

```rust
impl SetupInput {
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.slack_bot_token.as_deref().unwrap_or("").is_empty() {
            missing.push("slack_bot_token");
        }
        if self.slack_signing_secret.as_deref().unwrap_or("").is_empty() {
            missing.push("slack_signing_secret");
        }
        if self.slack_app_token.as_deref().unwrap_or("").is_empty() {
            missing.push("slack_app_token");
        }
        if self.slack_allowed_user_id.as_deref().unwrap_or("").is_empty() {
            missing.push("slack_allowed_user_id");
        }
        if self.channel_id.as_deref().unwrap_or("").is_empty() {
            missing.push("channel_id");
        }
        if self.project_root.as_deref().unwrap_or("").is_empty() {
            missing.push("project_root");
        }
        if self.project_label.as_deref().unwrap_or("").is_empty() {
            missing.push("project_label");
        }
        missing
    }
}

pub async fn resolve_setup_input(
    mut input: SetupInput,
    non_interactive: bool,
    prompter: &mut dyn SetupPrompter,
) -> Result<SetupInput> {
    if input.slack_bot_token.is_none() {
        if non_interactive { bail!("missing required field: slack_bot_token"); }
        input.slack_bot_token = Some(prompter.prompt_secret("SLACK_BOT_TOKEN")?);
    }
    if input.slack_signing_secret.is_none() {
        if non_interactive { bail!("missing required field: slack_signing_secret"); }
        input.slack_signing_secret = Some(prompter.prompt_secret("SLACK_SIGNING_SECRET")?);
    }
    if input.slack_app_token.is_none() {
        if non_interactive { bail!("missing required field: slack_app_token"); }
        input.slack_app_token = Some(prompter.prompt_secret("SLACK_APP_TOKEN")?);
    }
    if input.slack_allowed_user_id.is_none() {
        if non_interactive { bail!("missing required field: slack_allowed_user_id"); }
        input.slack_allowed_user_id = Some(prompter.prompt("SLACK_ALLOWED_USER_ID")?);
    }
    if input.channel_id.is_none() {
        if non_interactive { bail!("missing required field: channel_id"); }
        input.channel_id = Some(prompter.prompt("channelId")?);
    }
    if input.project_root.is_none() {
        if non_interactive { bail!("missing required field: project_root"); }
        input.project_root = Some(prompter.prompt("projectRoot")?);
    }
    if input.project_label.is_none() {
        if non_interactive { bail!("missing required field: project_label"); }
        input.project_label = Some(prompter.prompt("projectLabel")?);
    }
    Ok(input)
}
```

- [ ] **Step 5: Run the focused test to verify it passes**

Run: `cargo test -p rcc resolve_setup_input_only_prompts_for_missing_fields -- --exact`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/setup.rs crates/app/src/lib.rs
git commit -m "feat: add shared setup input model"
```

---

### Task 2: Add JSON file input for non-interactive setup

**Files:**
- Modify: `crates/app/src/setup.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs:340-540`
- Create: `docs/setup.example.json`

- [ ] **Step 1: Write the failing test for JSON file loading**

```rust
#[test]
fn load_setup_input_from_json_file() {
    let temp_dir = tempdir().expect("create temp dir");
    let path = temp_dir.path().join("setup.json");
    fs::write(
        &path,
        r#"{
  "slack_bot_token": "xoxb-json",
  "slack_signing_secret": "signing-json",
  "slack_app_token": "xapp-json",
  "slack_allowed_user_id": "UJSON",
  "channel_id": "CJSON",
  "project_root": "/tmp/project",
  "project_label": "json-project"
}"#,
    ).expect("write json file");

    let loaded = setup::load_setup_input_from_file(&path).expect("load setup input");
    assert_eq!(loaded.channel_id.as_deref(), Some("CJSON"));
    assert_eq!(loaded.project_label.as_deref(), Some("json-project"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rcc load_setup_input_from_json_file -- --exact`
Expected: FAIL because the file loader does not exist yet.

- [ ] **Step 3: Add the JSON loader and validation**

```rust
pub fn load_setup_input_from_file(path: &Path) -> Result<SetupInput> {
    let raw = fs::read_to_string(path).with_context(|| format!("read setup file: {}", path.display()))?;
    let input: SetupInput = serde_json::from_str(&raw)
        .with_context(|| format!("parse setup file: {}", path.display()))?;
    Ok(input)
}
```

- [ ] **Step 4: Extend CLI parsing to accept `--from-file`**

Add a setup options parser in `main.rs` or `setup.rs`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCliOptions {
    pub from_file: Option<PathBuf>,
    pub non_interactive: bool,
}

pub fn parse_setup_cli_options(args: &[String]) -> SetupCliOptions {
    let mut from_file = None;
    let mut non_interactive = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--from-file" => {
                if let Some(next) = args.get(index + 1) {
                    from_file = Some(PathBuf::from(next));
                    non_interactive = true;
                    index += 2;
                } else {
                    break;
                }
            }
            "--non-interactive" => {
                non_interactive = true;
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    SetupCliOptions { from_file, non_interactive }
}
```

- [ ] **Step 5: Add the example JSON file**

Create `docs/setup.example.json`:

```json
{
  "slack_bot_token": "xoxb-your-bot-token",
  "slack_signing_secret": "your-signing-secret",
  "slack_app_token": "xapp-your-app-token",
  "slack_allowed_user_id": "U12345678",
  "channel_id": "C12345678",
  "project_root": "/absolute/path/to/your/project",
  "project_label": "my-project"
}
```

- [ ] **Step 6: Run the focused test to verify it passes**

Run: `cargo test -p rcc load_setup_input_from_json_file -- --exact`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/app/src/setup.rs crates/app/src/main.rs crates/app/src/lib.rs docs/setup.example.json
git commit -m "feat: add setup file input"
```

---

### Task 3: Add environment override precedence

**Files:**
- Modify: `crates/app/src/setup.rs`
- Modify: `crates/app/src/lib.rs:340-540`

- [ ] **Step 1: Write the failing test for env override precedence**

```rust
#[test]
fn env_overrides_json_values_for_setup_input() {
    let previous = std::env::var_os("RCC_SETUP_CHANNEL_ID");
    unsafe { std::env::set_var("RCC_SETUP_CHANNEL_ID", "CENV") };

    let input = setup::apply_setup_env_overrides(setup::SetupInput {
        channel_id: Some("CJSON".into()),
        ..Default::default()
    });

    assert_eq!(input.channel_id.as_deref(), Some("CENV"));

    match previous {
        Some(value) => unsafe { std::env::set_var("RCC_SETUP_CHANNEL_ID", value) },
        None => unsafe { std::env::remove_var("RCC_SETUP_CHANNEL_ID") },
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rcc env_overrides_json_values_for_setup_input -- --exact`
Expected: FAIL because env override logic does not exist yet.

- [ ] **Step 3: Add the env override layer**

```rust
pub fn apply_setup_env_overrides(mut input: SetupInput) -> SetupInput {
    if let Ok(value) = std::env::var("RCC_SETUP_SLACK_BOT_TOKEN") {
        input.slack_bot_token = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_SLACK_SIGNING_SECRET") {
        input.slack_signing_secret = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_SLACK_APP_TOKEN") {
        input.slack_app_token = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_SLACK_ALLOWED_USER_ID") {
        input.slack_allowed_user_id = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_CHANNEL_ID") {
        input.channel_id = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_PROJECT_ROOT") {
        input.project_root = Some(value);
    }
    if let Ok(value) = std::env::var("RCC_SETUP_PROJECT_LABEL") {
        input.project_label = Some(value);
    }
    input
}
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -p rcc env_overrides_json_values_for_setup_input -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/setup.rs crates/app/src/lib.rs
git commit -m "feat: add setup env overrides"
```

---

### Task 4: Add fail-fast non-interactive setup mode

**Files:**
- Modify: `crates/app/src/setup.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs:340-540`

- [ ] **Step 1: Write the failing test for missing-field fail-fast**

```rust
#[tokio::test]
async fn non_interactive_setup_fails_fast_when_required_fields_are_missing() {
    let mut prompter = setup::FakePrompter::new(vec![]);
    let result = setup::resolve_setup_input(
        setup::SetupInput {
            slack_bot_token: Some("xoxb-ready".into()),
            ..Default::default()
        },
        true,
        &mut prompter,
    ).await;

    let error = format!("{result:?}");
    assert!(error.contains("missing required field"));
    assert!(error.contains("slack_signing_secret"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rcc non_interactive_setup_fails_fast_when_required_fields_are_missing -- --exact`
Expected: FAIL because missing-field reporting is too generic or absent.

- [ ] **Step 3: Improve non-interactive missing-field reporting**

Update `resolve_setup_input` so non-interactive paths return the full missing field list.

```rust
pub async fn resolve_setup_input(
    input: SetupInput,
    non_interactive: bool,
    prompter: &mut dyn SetupPrompter,
) -> Result<SetupInput> {
    if non_interactive {
        let missing = input.missing_fields();
        if !missing.is_empty() {
            bail!(format!(
                "missing required fields for non-interactive setup: {}. Fill them via --from-file or RCC_SETUP_*.",
                missing.join(", ")
            ));
        }
        return Ok(input);
    }
    // existing prompt fallback path...
}
```

- [ ] **Step 4: Route CLI setup through the layered input resolver**

In `run_setup`, resolve input like this:

```rust
let options = parse_setup_cli_options(args);
let mut input = SetupInput::default();
if let Some(path) = options.from_file.as_ref() {
    input = load_setup_input_from_file(path)?;
}
input = apply_setup_env_overrides(input);
let resolved = resolve_setup_input(input, options.non_interactive, &mut prompter).await?;
execute_setup(config, &workspace_root, resolved, &mut prompter).await
```

- [ ] **Step 5: Run the focused test to verify it passes**

Run: `cargo test -p rcc non_interactive_setup_fails_fast_when_required_fields_are_missing -- --exact`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/setup.rs crates/app/src/main.rs crates/app/src/lib.rs
git commit -m "feat: add fail-fast non-interactive setup"
```

---

### Task 5: Reuse one install execution engine for all setup frontends

**Files:**
- Modify: `crates/app/src/setup.rs`
- Modify: `crates/app/src/lib.rs:340-540`

- [ ] **Step 1: Write the failing test for shared execution engine**

```rust
#[tokio::test]
async fn execute_setup_accepts_pre_resolved_input_without_prompting() {
    let temp_dir = tempdir().expect("create temp dir");
    let workspace_root = temp_dir.path();
    fs::create_dir_all(workspace_root.join("slack")).expect("create slack dir");
    fs::write(workspace_root.join("slack/app-manifest.json"), "{}").expect("write manifest");

    let config = AppConfig {
        state_db_path: workspace_root.join(".local/state.db"),
        channel_project_store_path: workspace_root.join("data/channel-projects.json"),
        runtime_working_directory: workspace_root.display().to_string(),
        runtime_launch_command: "claude --settings .claude/claude-stop-hooks.json --dangerously-skip-permissions".to_string(),
        runtime_hook_events_directory: workspace_root.join(".local/hooks").display().to_string(),
        runtime_hook_settings_path: workspace_root.join(".claude/claude-stop-hooks.json"),
    };

    let input = setup::SetupInput {
        slack_bot_token: Some("xoxb-bot".into()),
        slack_signing_secret: Some("signing-secret".into()),
        slack_app_token: Some("xapp-app".into()),
        slack_allowed_user_id: Some("U123".into()),
        channel_id: Some("C123".into()),
        project_root: Some(workspace_root.display().to_string()),
        project_label: Some("demo-project".into()),
    };

    let mut prompter = setup::FakePrompter::new(vec![]);
    let result = setup::execute_setup(&config, workspace_root, input, &mut prompter).await;

    assert!(result.is_ok(), "{result:?}");
    assert!(fs::read_to_string(workspace_root.join(".env.local")).unwrap().contains("SLACK_BOT_TOKEN=xoxb-bot"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rcc execute_setup_accepts_pre_resolved_input_without_prompting -- --exact`
Expected: FAIL because execution is still entangled with prompting.

- [ ] **Step 3: Extract the install execution engine**

Create a shared executor.

```rust
pub async fn execute_setup(
    config: &AppConfig,
    workspace_root: &Path,
    input: SetupInput,
    prompter: &mut dyn SetupPrompter,
) -> Result<()> {
    let project_root = input.project_root.as_deref().context("missing project_root")?;
    validate_project_root(project_root)?;

    let env_path = workspace_root.join(".env.local");
    write_env_updates(
        &env_path,
        &[
            ("SLACK_BOT_TOKEN", input.slack_bot_token.as_deref().unwrap()),
            ("SLACK_SIGNING_SECRET", input.slack_signing_secret.as_deref().unwrap()),
            ("SLACK_APP_TOKEN", input.slack_app_token.as_deref().unwrap()),
            ("SLACK_ALLOWED_USER_ID", input.slack_allowed_user_id.as_deref().unwrap()),
        ],
    )?;
    let _ = from_path_override(&env_path);

    let store = JsonChannelProjectStore::new(config.channel_project_store_path.clone());
    let mut records = store.load()?;
    upsert_channel_project_record(
        &mut records,
        ChannelProjectRecord {
            channel_id: input.channel_id.unwrap(),
            project_root: project_root.to_string(),
            project_label: input.project_label.unwrap(),
        },
    );
    write_channel_project_records(&config.channel_project_store_path, &records)?;

    let checks = run_doctor(config, workspace_root);
    print_doctor_summary(prompter, &checks);
    if checks.iter().all(|check| check.ok) {
        prompter.println("Setup complete. You can now run: cargo run -p rcc");
        Ok(())
    } else {
        bail!(format_setup_doctor_failures(&checks))
    }
}
```

Have `run_setup_with_prompter` call `resolve_setup_input(...)` and then `execute_setup(...)`.

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -p rcc execute_setup_accepts_pre_resolved_input_without_prompting -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/setup.rs crates/app/src/lib.rs
git commit -m "refactor: split setup resolution from execution"
```

---

### Task 6: Update README and setup docs for hardened installer modes

**Files:**
- Modify: `README.md`
- Modify: `docs/slack-setup.md`
- Modify: `docs/manual-smoke-test.md`
- Modify: `docs/hero-export.md` (only if it now references setup flow)
- Create: `docs/setup.example.json`

- [ ] **Step 1: Write the failing doc checklist**

```md
- [ ] README mentions interactive and automation setup paths
- [ ] Slack setup doc includes `--from-file` and `RCC_SETUP_*`
- [ ] Manual smoke test uses deterministic non-interactive setup
- [ ] Example JSON file is documented and linked
```

- [ ] **Step 2: Verify the current docs fail the checklist**

Run: `python - <<'PY'
from pathlib import Path
checks = {
    'readme from-file': '--from-file' in Path('README.md').read_text(),
    'slack setup env overrides': 'RCC_SETUP_' in Path('docs/slack-setup.md').read_text(),
    'smoke deterministic setup': '--from-file' in Path('docs/manual-smoke-test.md').read_text(),
    'example json exists': Path('docs/setup.example.json').exists(),
}
for name, ok in checks.items():
    print(name, ok)
PY`
Expected: at least one `False` before editing.

- [ ] **Step 3: Update README Quickstart**

Add a short automation note below the install commands.

```md
Human:
```bash
cargo run -p rcc -- setup
```

Automation / Claude / smoke:
```bash
cargo run -p rcc -- setup --from-file docs/setup.example.json
```
```

- [ ] **Step 4: Update `docs/slack-setup.md`**

Add a dedicated section:

```md
## Automation-friendly setup

```bash
cargo run -p rcc -- setup --from-file docs/setup.example.json
```

Optional env overrides:
- `RCC_SETUP_SLACK_BOT_TOKEN`
- `RCC_SETUP_SLACK_SIGNING_SECRET`
- `RCC_SETUP_SLACK_APP_TOKEN`
- `RCC_SETUP_SLACK_ALLOWED_USER_ID`
- `RCC_SETUP_CHANNEL_ID`
- `RCC_SETUP_PROJECT_ROOT`
- `RCC_SETUP_PROJECT_LABEL`
```
```

- [ ] **Step 5: Update `docs/manual-smoke-test.md`**

Add a deterministic setup path before `doctor`.

```md
## Deterministic setup

```bash
cargo run -p rcc -- setup --from-file docs/setup.example.json
```

If secrets should not live in the file, inject them with `RCC_SETUP_*` environment variables.
```
```

- [ ] **Step 6: Run doc verification**

Run: `python - <<'PY'
from pathlib import Path
required = {
    'README.md': ['--from-file', 'cargo run -p rcc -- setup'],
    'docs/slack-setup.md': ['RCC_SETUP_', '--from-file'],
    'docs/manual-smoke-test.md': ['--from-file', 'RCC_SETUP_'],
    'docs/setup.example.json': ['slack_bot_token', 'channel_id', 'project_root'],
}
for path, needles in required.items():
    text = Path(path).read_text()
    missing = [needle for needle in needles if needle not in text]
    print(path, missing)
    if missing:
        raise SystemExit(1)
PY`
Expected: all files print empty missing lists.

- [ ] **Step 7: Commit**

```bash
git add README.md docs/slack-setup.md docs/manual-smoke-test.md docs/setup.example.json
git commit -m "docs: document hardened setup modes"
```

---

### Task 7: Verify the hardened installer end-to-end

**Files:**
- Verify: `crates/app/src/main.rs`
- Verify: `crates/app/src/lib.rs`
- Verify: `crates/app/src/setup.rs`
- Verify: `README.md`
- Verify: `docs/slack-setup.md`
- Verify: `docs/manual-smoke-test.md`
- Verify: `docs/setup.example.json`

- [ ] **Step 1: Run focused `rcc` tests**

Run: `cargo test -p rcc`
Expected: PASS, including new file/env/non-interactive tests.

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test`
Expected: PASS across the workspace.

- [ ] **Step 3: Run deterministic setup smoke test in a temporary workspace**

Create a temp setup file and execute:

```bash
cargo run -p rcc -- setup --from-file /tmp/setup.json
```

Expected: no prompt hang; setup completes or fails fast with explicit missing-field output.

- [ ] **Step 4: Run doctor after the smoke setup**

Run: `cargo run -p rcc -- doctor`
Expected: In a configured environment, all checks print `[OK]`. If the smoke workspace intentionally uses placeholders, report the exact fail-fast or doctor output instead of claiming success.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/main.rs crates/app/src/lib.rs crates/app/src/setup.rs README.md docs/slack-setup.md docs/manual-smoke-test.md docs/setup.example.json
git commit -m "feat: harden setup for automation"
```

---

## Self-review

### Spec coverage
- Shared setup input model is covered by Task 1.
- `--from-file` JSON input is covered by Task 2.
- `RCC_SETUP_*` env overrides are covered by Task 3.
- Non-interactive fail-fast behavior is covered by Task 4.
- Shared install execution engine is covered by Task 5.
- Documentation and deterministic smoke path are covered by Task 6.
- Final verification of no-hang automation is covered by Task 7.

### Placeholder scan
- No `TODO`, `TBD`, or “implement later” placeholders remain.
- Every task includes explicit files, code snippets, commands, and expected outcomes.

### Type consistency
- The core model is consistently `SetupInput`.
- Automation inputs are consistently `--from-file` and `RCC_SETUP_*`.
- The setup flow always resolves input first, then executes one install engine, then runs `doctor`.
