# Remote Claude Code

> 언제 어디서든, 휴대폰으로도 내가 이미 작업 중인 Claude Code 환경에 그대로 일을 시킬 수 있습니다.

새 에이전트도, 새 원격 IDE도 필요 없습니다. 지금 일하던 로컬/클라우드 작업환경을 그대로 Slack에서 이어가세요.

![Remote Claude Code hero demo](docs/images/hero-demo.gif)

- 기존 Claude Code 작업환경을 어디서든 이어서 사용
- 새 에이전트/새 워크플로우/새 원격 IDE 강요 없음
- 설치 후 `doctor`로 바로 검증 가능

## 왜 흥미로운가

Remote Claude Code는 Slack을 첫 번째 원격 UI로 사용합니다. Claude Code는 계속 내 로컬 또는 클라우드 작업환경에서 실행되고, 나는 Slack thread에서 같은 세션에 일을 이어서 시킬 수 있습니다.

핵심은 새로운 시스템으로 옮겨 타는 것이 아니라, **원래 쓰던 Claude Code 세션을 그대로 이어서 부리는 것**입니다.

## Quickstart

1. `data/channel-projects.example.json`을 복사해 `data/channel-projects.json`을 만듭니다.
2. 워크스페이스 루트에 `.env.local`을 준비합니다.
3. 설치와 환경이 맞는지 먼저 확인합니다.

```bash
cargo run -p rcc -- doctor
```

4. 앱을 실행합니다.

```bash
cargo run -p rcc
```

5. Slack에서 `/cc`를 실행해 세션을 시작합니다.

## Doctor

`doctor`는 현재 다음 항목을 확인합니다.

- `SLACK_BOT_TOKEN`
- `SLACK_APP_TOKEN`
- `SLACK_SIGNING_SECRET`
- `SLACK_ALLOWED_USER_ID`
- `.env.local` 존재 여부
- `tmux` 사용 가능 여부
- 상태 DB 경로 생성 가능 여부
- hook events 디렉터리 생성 가능 여부
- `slack/app-manifest.json` 존재 여부
- `data/channel-projects.json` 존재 여부

설치가 제대로 되었는지 빠르게 확인하고 싶다면, 앱 실행 전에 항상 `doctor`부터 돌리면 됩니다.

## How it works

- Slack은 원격 UI입니다.
- Claude Code는 기존 로컬 또는 클라우드 작업환경에서 계속 실행됩니다.
- tmux, session, hook relay를 통해 상태와 최종 응답이 Slack thread로 돌아옵니다.
- 첫 공개는 Slack-first지만, 장기적으로는 다른 메시징 인터페이스도 같은 모델로 확장할 수 있습니다.

## Use cases

- 외출 중 휴대폰으로 현재 Claude Code 세션에 작업 이어서 시키기
- 이동 중 코드 리뷰, 파일 검토, 다음 액션 정리 지시하기
- 긴 작업을 하나의 thread/session 흐름으로 계속 유지하기

## Setup

- Slack 설정: [`docs/slack-setup.md`](docs/slack-setup.md)
- 수동 점검: [`docs/manual-smoke-test.md`](docs/manual-smoke-test.md)
- 런치 카피 팩: [`docs/launch-copy.ko.md`](docs/launch-copy.ko.md)

## Roadmap

- Slack-first public release
- Better launch assets and lighter install flow
- Discord transport
- Telegram transport

## Current limitations

- 현재 공개 대상은 Slack 기준으로 설계되어 있습니다.
- `rcc setup slack`은 아직 구현되지 않았습니다.
- 설치 경험은 계속 단순화 중이지만, 지금은 `.env.local`과 Slack 앱 생성이 필요합니다.
- 런타임/운영 안정성은 계속 강화 중입니다.
