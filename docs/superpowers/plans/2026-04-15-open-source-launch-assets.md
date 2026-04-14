# Open-Source Launch Assets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repackage the current Rust prototype into a star-worthy public open-source presentation with a Korean-first README, strong hero/demo assets, AI-friendly installation flow, and a roadmap that starts with Slack and expands to Discord/Telegram.

**Architecture:** Keep the product surface Slack-first while reorganizing the repository’s public-facing materials around three layers: Hero (desire), Proof (doctor + quickstart), and Quickstart (first success). Do not add product behavior beyond what already exists unless it directly reduces installation friction or makes existing behavior easier to validate.

**Tech Stack:** Rust workspace (`cargo`, `rcc`), Markdown docs, static HTML demo asset, optional browser capture tooling for GIF export

---

## File structure

### Existing files to modify
- `README.md` — replace prototype-focused copy with Korean-first public launch README using Hero → Proof → Quickstart structure
- `docs/manual-smoke-test.md` — tighten wording so it supports README quickstart and doctor-first trust flow
- `docs/slack-setup.md` — rewrite around “딸깍 설치 or AI-assisted install” and keep Slack-first setup concise
- `docs/hero-mock-v18.html` — adjust text/art direction so the exported demo matches the launch promise and CLI/pixel aesthetic direction

### New files to create
- `docs/launch-copy.ko.md` — Korean launch copy pack: GitHub description, README opener, social/community launch text, demo captions
- `docs/hero-export.md` — exact repeatable instructions for exporting the hero HTML into README-safe GIF/video assets
- `docs/images/` (asset directory) — place exported hero asset(s) here, e.g. `docs/images/hero-demo.gif`

### Existing files to check while implementing
- `docs/superpowers/specs/2026-04-15-open-source-launch-design.md` — source-of-truth spec
- `docs/architecture.md` — keep architectural claims honest and aligned with current implementation
- `crates/app/src/main.rs` — verify actual CLI entry points (`rcc`, `doctor`)
- `crates/app/src/lib.rs:240-321` — verify current `doctor` behavior and checks before documenting them
- `data/channel-projects.example.json` — match quickstart instructions to real mapping format

---

### Task 1: Rewrite the README around Hero → Proof → Quickstart

**Files:**
- Modify: `README.md`
- Check: `docs/superpowers/specs/2026-04-15-open-source-launch-design.md`
- Check: `docs/architecture.md`
- Check: `crates/app/src/main.rs:1-77`
- Check: `crates/app/src/lib.rs:240-321`

- [ ] **Step 1: Write the failing content checklist in the README draft**

Add a temporary checklist at the top of your working draft or notes so the rewrite is testable against the spec.

```md
- [ ] Hero one-liner emphasizes existing Claude Code workspace continuity
- [ ] README starts in Korean
- [ ] Hero asset appears before architecture details
- [ ] Core bullets mention no new agent workflow, no new environment, doctor verification
- [ ] Quickstart includes install/config/doctor/run/`/cc`
- [ ] Roadmap names Discord and Telegram after Slack
```

- [ ] **Step 2: Verify the current README fails the checklist**

Run: `python - <<'PY'
from pathlib import Path
text = Path('README.md').read_text()
checks = {
    'workspace continuity': '기존 Claude Code' in text or 'existing Claude Code' in text,
    'korean opening': any('\uac00' <= ch <= '\ud7a3' for ch in text[:200]),
    'doctor in quickstart': 'doctor' in text and 'Quickstart' in text,
    'discord roadmap': 'Discord' in text,
}
for name, ok in checks.items():
    print(f'{name}: {ok}')
PY`
Expected: multiple `False` results because the current README is still prototype-oriented.

- [ ] **Step 3: Replace the README with the launch-oriented structure**

Write the new README around the exact order below.

```md
# Remote Claude Code

> 언제 어디서든, 휴대폰으로도 내가 이미 작업 중인 Claude Code 환경에 그대로 일을 시킬 수 있습니다.

![Remote Claude Code hero demo](docs/images/hero-demo.gif)

- 기존 Claude Code 작업환경을 어디서든 이어서 사용
- 새 에이전트/새 워크플로우/새 원격 IDE 강요 없음
- 설치 후 `doctor`로 바로 검증 가능

## 왜 흥미로운가
Slack은 첫 번째 원격 UI입니다. Claude Code는 계속 내 로컬 또는 클라우드 작업환경에서 실행되고, 나는 Slack thread에서 같은 세션에 일을 이어서 시킵니다.

## Quickstart
1. 저장소 준비
2. `.env.local` 설정
3. `cargo run -p rcc -- doctor`
4. `cargo run -p rcc`
5. Slack에서 `/cc`

## Doctor
`doctor`는 Slack 토큰, `tmux`, manifest, channel mapping, 상태 저장 경로를 확인합니다.

## How it works
- Slack은 원격 UI
- Claude Code는 기존 환경에서 실행
- tmux/session/hook으로 상태와 응답 relay

## Use cases
- 외출 중 휴대폰으로 작업 이어가기
- 이동 중 코드 리뷰/파일 검토
- 장기 세션 continuity 유지

## Roadmap
- Slack-first public release
- Discord transport
- Telegram transport
```

- [ ] **Step 4: Run a focused README sanity check**

Run: `python - <<'PY'
from pathlib import Path
text = Path('README.md').read_text()
required = [
    '휴대폰',
    '기존 Claude Code',
    'doctor',
    '/cc',
    'Discord',
    'Telegram',
]
missing = [item for item in required if item not in text]
print('missing:', missing)
raise SystemExit(1 if missing else 0)
PY`
Expected: exits successfully with `missing: []`.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README for public launch"
```

---

### Task 2: Make install, setup, and doctor feel one-click or AI-assisted

**Files:**
- Modify: `docs/slack-setup.md`
- Modify: `docs/manual-smoke-test.md`
- Check: `crates/app/src/lib.rs:240-321`
- Check: `data/channel-projects.example.json`

- [ ] **Step 1: Write the failing setup expectations as a doc contract**

Create a concrete setup contract before editing.

```md
- [ ] Setup starts with the product promise, not internal architecture
- [ ] Setup can be followed by a human or delegated to Claude Code
- [ ] `doctor` appears before “run the app”
- [ ] Channel mapping instructions match `data/channel-projects.example.json`
- [ ] Manual smoke test starts from a successful `doctor`
```

- [ ] **Step 2: Verify the current setup docs fail the new contract**

Run: `python - <<'PY'
from pathlib import Path
setup = Path('docs/slack-setup.md').read_text()
smoke = Path('docs/manual-smoke-test.md').read_text()
checks = {
    'ai assisted wording': 'Claude Code' in setup,
    'doctor before run app': setup.find('doctor') < setup.find('run') if 'doctor' in setup and 'run' in setup else False,
    'smoke starts from doctor': '## Doctor' in smoke,
}
for name, ok in checks.items():
    print(f'{name}: {ok}')
PY`
Expected: at least one weak or failing signal that justifies the rewrite.

- [ ] **Step 3: Rewrite `docs/slack-setup.md` for AI-assisted installation**

Use short screenshot-friendly copy and include a Claude Code delegation path.

```md
# Slack Setup

## 목표
설치는 딸깍이거나, 최소한 Claude Code에게 맡길 수 있을 정도로 단순해야 합니다.

## 가장 짧은 흐름
1. `slack/app-manifest.json`으로 Slack 앱 생성
2. `.env.local`에 Slack 값 입력
3. `cargo run -p rcc -- doctor`
4. `cargo run -p rcc`
5. Slack에서 `/cc`

## Claude Code에게 맡길 때
다음처럼 요청할 수 있습니다.

```text
이 저장소의 Slack 설정을 진행해줘. manifest 경로를 쓰고, `.env.local`에 필요한 항목을 채우고, 마지막에 doctor까지 실행해줘.
```
```

- [ ] **Step 4: Tighten `docs/manual-smoke-test.md` around doctor-first proof**

Rewrite the opening so the test sequence is: prerequisites → doctor → run → `/cc` → thread reply.

```md
## Doctor
먼저 아래 명령이 모두 `[OK]`를 출력해야 합니다.

```bash
cargo run -p rcc -- doctor
```

## Slack Run
`doctor`가 통과한 뒤에만 앱을 실행합니다.
```

- [ ] **Step 5: Run doc verification**

Run: `python - <<'PY'
from pathlib import Path
setup = Path('docs/slack-setup.md').read_text()
smoke = Path('docs/manual-smoke-test.md').read_text()
required_setup = ['Claude Code', 'doctor', '/cc', 'slack/app-manifest.json']
required_smoke = ['## Doctor', '## Slack Run', '/cc']
missing = {
    'setup': [x for x in required_setup if x not in setup],
    'smoke': [x for x in required_smoke if x not in smoke],
}
print(missing)
raise SystemExit(1 if any(missing.values()) else 0)
PY`
Expected: exits successfully with empty missing lists.

- [ ] **Step 6: Commit**

```bash
git add docs/slack-setup.md docs/manual-smoke-test.md
git commit -m "docs: simplify setup and doctor flow"
```

---

### Task 3: Turn the HTML mock into a real hero asset pipeline

**Files:**
- Modify: `docs/hero-mock-v18.html`
- Create: `docs/hero-export.md`
- Create: `docs/images/hero-demo.gif` (or a temporary captured asset that will later be optimized)

- [ ] **Step 1: Define the failing hero asset requirements**

Before editing the HTML, write the acceptance criteria.

```md
- [ ] Demo shows `/cc` → session start → continued thread work → final reply
- [ ] Copy emphasizes “existing workspace” instead of generic Slack integration
- [ ] Visual style supports CLI / terminal / pixel aesthetic
- [ ] Export steps are repeatable by another engineer
- [ ] README can reference a stable asset path under `docs/images/`
```

- [ ] **Step 2: Verify the current mock misses at least one launch requirement**

Run: `python - <<'PY'
from pathlib import Path
text = Path('docs/hero-mock-v18.html').read_text()
checks = {
    'existing workspace wording': 'existing Claude Code workspace' in text or '작업환경' in text,
    'pixel mention optional': 'pixel' in text or 'retro' in text,
    'stable asset path exists': Path('docs/images/hero-demo.gif').exists(),
}
for name, ok in checks.items():
    print(f'{name}: {ok}')
PY`
Expected: at least the asset-path check fails before implementation.

- [ ] **Step 3: Update the HTML copy and art direction for launch messaging**

Adjust the visible strings to match the README promise.

```html
<p class="subtitle">Keep your existing Claude Code workspace moving from Slack.</p>
<div class="desc">Your workspace stays where it is. Slack becomes the remote UI.</div>
<div class="terminal-foot">CLI-native remote control for the Claude Code session you already use.</div>
```

If you add any decorative treatment, keep it subtle and terminal-native rather than generic SaaS.

- [ ] **Step 4: Write repeatable export instructions**

Create `docs/hero-export.md` with exact commands for a local screen capture/export flow.

```md
# Hero Export

1. Open `docs/hero-mock-v18.html` in a browser.
2. Record a 10-15 second loop at 2x retina resolution.
3. Crop to the product frame only.
4. Export a lightweight GIF to `docs/images/hero-demo.gif`.
5. If GIF size is too large, keep a `.mp4` source and re-encode.

Example ffmpeg flow:
```bash
ffmpeg -i hero-demo.mov -vf "fps=12,scale=1400:-1:flags=lanczos" docs/images/hero-demo.gif
```
```

- [ ] **Step 5: Produce the first real asset and verify the path**

Run: `test -f docs/images/hero-demo.gif && file docs/images/hero-demo.gif`
Expected: command succeeds and prints GIF file metadata.

- [ ] **Step 6: Commit**

```bash
git add docs/hero-mock-v18.html docs/hero-export.md docs/images/hero-demo.gif
git commit -m "docs: add launch hero asset pipeline"
```

---

### Task 4: Add the Korean launch copy pack and public roadmap messaging

**Files:**
- Create: `docs/launch-copy.ko.md`
- Modify: `README.md`
- Check: `docs/superpowers/specs/2026-04-15-open-source-launch-design.md`

- [ ] **Step 1: Write the failing copy matrix**

Make the required copy surfaces explicit.

```md
- [ ] GitHub repository description
- [ ] README one-line hook
- [ ] README subheading / support copy
- [ ] Short community launch post
- [ ] Demo caption
- [ ] Roadmap wording that says Slack first, Discord/Telegram next
```

- [ ] **Step 2: Verify these surfaces are not yet centralized**

Run: `test -f docs/launch-copy.ko.md; echo $?`
Expected: `1` because the copy pack does not exist yet.

- [ ] **Step 3: Create the Korean launch copy pack**

Populate `docs/launch-copy.ko.md` with short reusable copy.

```md
# Launch Copy (KO)

## GitHub description
언제 어디서든 Slack으로 내 Claude Code 작업환경을 이어서 쓰는 오픈소스 도구.

## README one-liner
휴대폰으로도 내가 이미 작업 중인 Claude Code 환경에 그대로 일을 시킬 수 있습니다.

## README support copy
새 에이전트도, 새 원격 IDE도 필요 없습니다. 지금 일하던 로컬/클라우드 작업환경을 그대로 Slack에서 이어가세요.

## Launch post
방금 `Remote Claude Code`를 공개했습니다.
Slack에서 `/cc`를 열고, 제가 원래 작업하던 Claude Code 세션을 그대로 이어서 부릴 수 있습니다.
설치는 가볍게, 검증은 `doctor`로 바로.

## Roadmap line
Slack-first로 공개하고, 다음 transport는 Discord와 Telegram입니다.
```

- [ ] **Step 4: Link the README wording back to the copy pack**

Update the README opening lines so they exactly match the chosen Korean copy pack.

```md
> 휴대폰으로도 내가 이미 작업 중인 Claude Code 환경에 그대로 일을 시킬 수 있습니다.

새 에이전트도, 새 원격 IDE도 필요 없습니다. 지금 일하던 로컬/클라우드 작업환경을 그대로 Slack에서 이어가세요.
```

- [ ] **Step 5: Run copy consistency verification**

Run: `python - <<'PY'
from pathlib import Path
readme = Path('README.md').read_text()
copy = Path('docs/launch-copy.ko.md').read_text()
needles = [
    '휴대폰으로도 내가 이미 작업 중인 Claude Code 환경에 그대로 일을 시킬 수 있습니다.',
    'Slack-first',
    'Discord',
    'Telegram',
]
for needle in needles:
    print(needle, needle in readme or needle in copy)
PY`
Expected: every line prints `True`.

- [ ] **Step 6: Commit**

```bash
git add docs/launch-copy.ko.md README.md
git commit -m "docs: add Korean launch copy pack"
```

---

### Task 5: Final verification for the public-launch docs set

**Files:**
- Verify: `README.md`
- Verify: `docs/slack-setup.md`
- Verify: `docs/manual-smoke-test.md`
- Verify: `docs/hero-mock-v18.html`
- Verify: `docs/hero-export.md`
- Verify: `docs/launch-copy.ko.md`
- Verify: `docs/images/hero-demo.gif`

- [ ] **Step 1: Run focused tests for the current product claims**

Run: `cargo test -p app`
Expected: PASS, proving the documented `doctor` and CLI entry points still match the code.

- [ ] **Step 2: Run full workspace tests before claiming the launch docs are ready**

Run: `cargo test`
Expected: PASS across the workspace.

- [ ] **Step 3: Run the documented health check**

Run: `cargo run -p rcc -- doctor`
Expected: Either all `[OK]` in a configured environment, or specific `[FAIL]` lines that match the documented doctor behavior. If local secrets are unavailable, capture that limitation explicitly rather than hiding it.

- [ ] **Step 4: Verify the docs set contains every required launch surface**

Run: `python - <<'PY'
from pathlib import Path
required_files = [
    'README.md',
    'docs/slack-setup.md',
    'docs/manual-smoke-test.md',
    'docs/hero-mock-v18.html',
    'docs/hero-export.md',
    'docs/launch-copy.ko.md',
    'docs/images/hero-demo.gif',
]
missing = [path for path in required_files if not Path(path).exists()]
print('missing:', missing)
raise SystemExit(1 if missing else 0)
PY`
Expected: exits successfully with `missing: []`.

- [ ] **Step 5: Commit the verified launch package**

```bash
git add README.md docs/slack-setup.md docs/manual-smoke-test.md docs/hero-mock-v18.html docs/hero-export.md docs/launch-copy.ko.md docs/images/hero-demo.gif
git commit -m "docs: finalize public launch package"
```

---

## Self-review

### Spec coverage
- Hero positioning, continuity language, and Korean-first README are covered in Task 1.
- One-click / AI-assisted install and doctor-first trust flow are covered in Task 2.
- Visual-heavy CLI/pixel-friendly hero asset work is covered in Task 3.
- Slack-first with Discord/Telegram next, plus launch copy surfaces, are covered in Task 4.
- Final verification and evidence-before-claims are covered in Task 5.

### Placeholder scan
- No `TODO`, `TBD`, or “write tests later” placeholders remain.
- Every task includes exact files, commands, and concrete content snippets.

### Type and interface consistency
- All CLI references use the current binary/command style: `cargo run -p rcc -- doctor`, `cargo run -p rcc`, and `/cc`.
- Roadmap wording is consistently Slack-first with Discord/Telegram next.
- The hero asset path is consistently `docs/images/hero-demo.gif`.
