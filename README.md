# oh-my-remote-ai

![GitHub stars](https://img.shields.io/github/stars/mskangg/remote-claude-code?style=flat&color=yellow)
![License](https://img.shields.io/badge/license-MIT-green)
![Slack First](https://img.shields.io/badge/transport-Slack--first-4A154B)
![Rust](https://img.shields.io/badge/built%20with-Rust-orange)

> 지금 작업 중인 AI 에이전트를 Slack에서 그대로 이어서 부릴 수 있습니다.

새 에이전트도, 새 원격 IDE도 필요 없습니다. 지금 일하던 로컬/클라우드 작업환경 그대로, 심지어 휴대폰에서도 같은 세션에 일을 시킬 수 있습니다.

[Quickstart](#quickstart) · [Doctor](#doctor) · [How it works](#how-it-works) · [Roadmap](#roadmap)

![oh-my-remote-ai hero demo](docs/images/hero-demo.gif)

- **Claude Code, Codex, Gemini 등 어떤 에이전트든 Slack에서 제어**
- **무료 · 무제한 세션 · 셀프호스팅**
- **새 에이전트나 새 원격 개발환경을 강요하지 않음**

![oh-my-remote-ai vibe shot](docs/images/hero-view.jpg)

## Why this is different

### 이건 이런 제품이 아닙니다

- 새로운 agent platform
- 별도의 remote IDE
- 지금 작업환경을 버리고 옮겨 타는 시스템
- 유료 구독이 필요한 서비스

### 이건 이런 제품입니다

- **Slack이 원격 UI가 됩니다**
- **에이전트는 원래 작업하던 환경에서 계속 실행됩니다**
- **당신은 같은 세션을 어디서든 이어서 부립니다**
- **Claude Code뿐 아니라 Codex, Gemini도 지원합니다**

## 슬래시 커맨드

| 커맨드 | 에이전트 |
|--------|---------|
| `/cc` | Claude Code |
| `/cx` | Codex |
| `/gm` | Gemini CLI |

## Quickstart

### 1. 레포 클론 및 Claude Code 실행

```bash
git clone https://github.com/mskangg/remote-claude-code.git
cd remote-claude-code
claude
```

### 2. 플러그인 설치

마켓플레이스 추가:

```bash
/plugin marketplace add mskangg/remote-claude-code
```

플러그인 설치:

```bash
/plugin install remote-claude-code-setup@remote-claude-code
```

### 3. Claude Code에서 셋업 시작

아래처럼 말하면 됩니다.

```text
remote-claude-code 셋업해줘
```

또는:

```text
슬랙 연동 설치해줘
```

### 4. 설치 마법사 진행

setup wizard는 다음 순서로 진행됩니다.
- 로컬 환경 확인
- Slack 콘솔 단계 안내
  - 링크: `https://api.slack.com/apps?new_app=1`
  - manifest 제공 (보기용 + raw, `/cc` `/cx` `/gm` 슬래시 커맨드 포함)
- 필요한 값을 한 단계씩 수집
  - 허용 사용자 ID 여러 명 가능 (쉼표 구분, 예: `U123,U456`)
- artifact 기반 resume
- `doctor`
- release binary 준비
- 감지된 에이전트(Codex, Gemini) 훅 자동 설치

### 5. 설치 완료 후 실행

```bash
rcc
```

백그라운드 상시 실행:

```bash
rcc service install    # launchd 서비스 등록 + 시작
rcc service start      # 서비스 시작
rcc service stop       # 서비스 중지
rcc service restart    # 서비스 재시작
rcc service status     # 서비스 상태 확인
rcc service uninstall  # 서비스 해제 + 바이너리 제거
```

### 허용 사용자 추가

나중에 봇을 사용할 수 있는 사람을 추가하려면:

```bash
# .env.local에서 SLACK_ALLOWED_USER_ID에 쉼표로 추가
SLACK_ALLOWED_USER_ID=U123,U456,U789

# 재시작
rcc service restart
```

### Direct CLI path

플러그인 없이 직접 진행하려면 아래 경로를 사용할 수 있습니다.

```bash
# 1. artifact 템플릿 생성
cargo run -p rcc -- setup --write-slack-artifact-template .local/slack-setup-artifact.json

# 2. 값 채운 뒤 merge
cargo run -p rcc -- setup --merge-slack-artifact <patch.json> --json

# 3. 설치 (release 빌드 + 바이너리 설치 + 설정 기록 자동)
cargo run -p rcc -- setup --from-slack-artifact .local/slack-setup-artifact.json --non-interactive --locale ko

# 4. 검증
rcc doctor

# 5. 서비스 등록
rcc service install && rcc service start
```

## Doctor

`doctor`는 "지금 바로 되는 상태인가?"를 빠르게 확인하기 위한 명령입니다.

```bash
rcc doctor
```

검증 항목:
- Slack 토큰 4종
- 허용 사용자 ID 설정 여부
- `.env.local` 존재 여부
- `tmux` 사용 가능 여부
- 상태 DB 경로
- hook events 디렉터리
- `slack/app-manifest.json`
- `data/channel-projects.json`
- Codex 설치 여부 (optional, `/cx` 사용 시 필요)
- Gemini 설치 여부 (optional, `/gm` 사용 시 필요)

## How it works

- Slack은 원격 UI입니다
- 에이전트(Claude Code / Codex / Gemini)는 기존 로컬 또는 클라우드 작업환경에서 실행됩니다
- tmux + hook relay를 통해 상태와 최종 응답이 Slack thread로 돌아옵니다
- 채널 하나가 프로젝트 하나를 대표합니다

## Use cases

### Away from desk
자리에서 벗어나도 휴대폰으로 같은 세션에 작업을 이어서 시킬 수 있습니다.

### In transit
이동 중에도 코드 리뷰, 파일 검토, 다음 액션 정리 같은 일을 Slack thread로 지시할 수 있습니다.

### Long-running sessions
긴 작업을 하나의 thread/session 흐름으로 유지하면서 상태와 최종 응답을 계속 추적할 수 있습니다.

### Multi-agent workflow
같은 프로젝트에서 `/cc`, `/cx`, `/gm`으로 각자 다른 에이전트 세션을 열어 병렬로 활용할 수 있습니다.

## Setup and docs

- Slack 설정: [`docs/slack-setup.md`](docs/slack-setup.md)
- Setup baseline example: [`docs/setup.example.json`](docs/setup.example.json)

## Contributing

버그 리포트, 기능 제안, PR 모두 환영합니다. 자세한 내용은 [CONTRIBUTING.md](CONTRIBUTING.md)를 참고하세요.

## License

MIT © 2026 [mskangg](https://github.com/mskangg)

See [LICENSE](LICENSE) for the full text.

## Roadmap

- oh-my-remote-ai 공개 런치
- 더 쉬운 설치 및 온보딩
- Discord transport
- Telegram transport
- OpenCode (`/oc`) 지원

## Current limitations

- `rcc service` 명령은 macOS launchd 기반으로, 현재 macOS 전용입니다.
- Codex/Gemini 세션은 앱 재시작 후 default 에이전트(Claude Code)로 복구될 수 있습니다.
- Codex 훅은 `features.codex_hooks = true` 플래그 지원 이후 활성화됩니다.
