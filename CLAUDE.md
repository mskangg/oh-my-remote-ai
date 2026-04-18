# CLAUDE.md

## 목적

이 프로젝트의 목표는 Slack 안에서 Claude Code 세션을 안정적으로 원격 제어하는 제품을 출시하는 것이다.

핵심 기준:

- 타입스크립트 버전과 동일하거나 더 나은 사용자 경험을 제공해야 한다.
- 구조 개선은 허용된다. 다만 구조 개선이 실사용 회귀를 만들면 실패다.
- “동작하는 코드”보다 “운영 가능한 코드”를 우선한다.

## 현재 아키텍처 원칙

Rust 포트는 다음 계층 구조를 기준으로 유지한다.

- `crates/application`
  - 유스케이스, 오케스트레이션, 상태 전이 후속 처리
  - Slack 제품 동작 규칙을 담는다
- `crates/transport-slack`
  - Slack Socket Mode ingress
  - Slack API adapter
  - Slack payload 파싱/응답 생성
- `crates/runtime-local`
  - tmux 실행
  - Claude 프로세스 실행
  - hook file polling
- `crates/session-store`
  - sqlite 영속화
- `crates/core-model`
  - 도메인 식별자, 상태, 메시지 모델
- `crates/core-service`
  - session actor / reducer / runtime forwarding 정책
- `crates/app`
  - bootstrap, wiring, env/config

의존성 규칙:

- `application`은 `transport-slack`의 adapter 인터페이스를 사용해도 된다.
- `transport-slack`는 유스케이스를 직접 소유하지 않는다.
- `runtime-local`과 `session-store`는 인프라다. 제품 정책을 넣지 않는다.
- `app`은 조립만 한다.

## 환경 변수 / 실행 규칙

- Rust는 워크트리 루트의 `.env.local`만 본다.
- Slack 앱 토큰/봇 토큰은 워크트리 기준으로 독립 관리한다.
- 기본 상태 DB는 `.local/state.db`
- 기본 hook event 디렉터리는 `.local/hooks`

## 빌드 / 진단

- 바이너리 크레이트 패키지 이름은 `rcc`: `cargo test -p rcc`, `cargo build -p rcc` (`app`으로 하면 오류)
- 전체 테스트: `cargo test --workspace` / 린트: `cargo clippy --all-targets --all-features`
- 로그: `RUST_LOG=debug rcc` (기본 INFO; tracing_subscriber 설치됨)
- 새 크레이트 추가 시 `Cargo.toml`에 `[lints]\nworkspace = true` 필수

## Slack UX 규칙

### 1. `/cc`

- `/cc`는 바로 세션을 만들지 않는다.
- 먼저 메인 메뉴를 보여준다.
- 메뉴에는 최소 다음 액션이 있어야 한다.
  - `새 세션 열기`
  - `기존 세션 보기`

### 2. 기존 세션 보기

- 텍스트만 보여주면 안 된다.
- 세션 목록 block UI를 사용한다.
- 각 항목에는 최소 다음 정보가 있어야 한다.
  - 프로젝트명
  - tmux session name
  - thread ts
  - `스레드 열기` 버튼

### 3. 세션 thread

- thread 안에서는 slash command에 의존하지 않는다.
- 세션 제어 진입점은 thread 안의 `명령어` 버튼이다.
- command palette에는 최소 다음 액션이 있어야 한다.
  - `Interrupt`
  - `Esc`
  - `Clear`
  - `CLAUDE.md update`
  - `세션 종료`

### 4. 세션 종료

- `세션 종료`는 tmux session 종료를 의미한다.
- 종료 후 stale action이 눌려도 프로세스는 죽으면 안 된다.
- 사용자에게는 graceful 하게 무시되거나 종료 상태로 처리되어야 한다.

## 상태 메시지 규칙

이 프로젝트에서 가장 중요한 UX 규칙 중 하나다.

- `Working...` 상태 메시지는 thread root를 편집하는 방식이 아니다.
- 각 turn마다 별도의 status message를 thread에 새로 만든다.
- turn 진행 중에는 그 status message만 갱신한다.
- turn 완료/실패 시에는 그 status message를 삭제한다.
- 최종 답변은 새 thread message로 올린다.

즉 금지 사항:

- root message를 상태 표시용으로 edit
- 완료 답변을 status message edit로 대체
- 삭제된 status message를 다음 turn에서 재사용

상태 메시지 내용 규칙:

- 기본 진행 상태는 `작업 중...` 계열
- hook progress event가 있으면 더 구체적인 상태로 바꾼다
  - 예: 검색 중, 파일 읽는 중, 수정 중, 응답 정리 중

## Hook / runtime 규칙

- Claude 종료/응답 relay는 hook file 기준으로 처리한다.
- tmux pane 상태만 보고 “아마 끝났음” 식으로 처리하지 않는다.
- hook `Stop` / `StopFailure`가 최종 전달 기준이다.
- hook progress event(`PreToolUse`, `PostToolUse`)는 status message 갱신에 사용한다.

turn 처리 규칙:

- turn은 단일 값이 아니라 순차적으로 관리 가능한 구조여야 한다.
- 이전 turn 완료 전에 다음 입력이 들어와도, 완료 이벤트 매핑이 꼬이지 않도록 해야 한다.
- terminal event가 와도 pending turn이 없으면 프로세스가 죽으면 안 된다.

초기화 순서:

- `configure_slack_lifecycle_observer`는 반드시 `recover_active_sessions` **이전**에 호출해야 한다.
  recovery 중 발생하는 runtime event가 observer를 통해 Slack에 전달된다. 순서가 바뀌면 reply가 유실된다.

운영 상수:

- hook poller: 연속 5회 실패 → `RuntimeFailed` emit 후 종료 (`MAX_CONSECUTIVE_FAILURES = 5`)
- WebSocket open 실패: 최대 10회 재시도 후 fatal (`MAX_CONSECUTIVE_OPEN_FAILURES = 10`)

## 에러 처리 규칙

이 프로젝트는 “요청 실패”와 “프로세스 실패”를 엄격히 분리한다.

non-fatal:

- 종료된 세션에 대한 stale action
- 없는 status message update 실패
- 특정 thread에 대한 session binding 없음
- permalink 조회 실패
- 개별 Slack action 처리 실패

fatal:

- 프로세스 부팅 실패
- Slack Socket Mode 연결 자체 실패
- 필수 env/config 누락

원칙:

- 개별 Slack 요청 실패로 `rcc` 전체가 종료되면 안 된다.
- listener loop는 최대한 살아 있어야 한다.
- action handler 내부 오류는 로그 + graceful continue가 기본이다.
- transcript recovery(`recover_terminal_event`)에서 `HOME` 미설정은 non-fatal skip이다.
  에러로 처리하면 hook poller 실패 카운터에 쌓여 멀쩡한 세션이 강제 실패된다.

## tmux 규칙

- 앱 시작 시 orphan UUID tmux session 정리를 수행한다.
- DB에 존재하지 않는 UUID 세션만 정리한다.
- 사용자가 직접 쓰는 `slack-*` 등 일반 세션은 건드리지 않는다.

## 테스트 / 변경 규칙

- 새 기능이나 회귀 수정은 반드시 테스트를 먼저 추가하거나 함께 추가한다.
- 최소 기준:
  - targeted test
  - 관련 crate test
  - 최종적으로 `cargo test`

필수 회귀 테스트 대상:

- `/cc` 메뉴
- 새 세션 생성
- 기존 세션 보기
- thread reply relay
- status message 생성/업데이트/삭제
- 최종 답변 relay
- command palette
- `세션 종료` 후 stale action
- orphan tmux cleanup

## 작업 우선순위 규칙

작업 우선순위는 항상 아래 순서를 따른다.

1. 사용자 체감 회귀 수정
2. 데이터/세션 일관성
3. 프로세스 생존성
4. TS parity
5. 구조 개선

단, 구조 개선이 위 1~4를 더 안정적으로 만드는 경우에는 바로 진행해도 된다.

## 구현 태도

- 구조 욕심을 내는 것은 허용된다.
- 하지만 출시가 목표이므로, 구조는 제품 안정성을 높이는 방향으로만 바꾼다.
- “예쁜 구조”보다 “운영 중 회귀를 줄이는 구조”를 택한다.
- TS보다 후진 UX가 나오면 그 변경은 미완성이다.

## 하네스: Remote Claude Harness

**목표:** 이 저장소의 세션 lifecycle, Slack UX, hook relay, stale action 작업을 낮은 토큰 비용의 orchestrator + specialist 방식으로 처리한다.

**트리거:** 이 저장소에서 하네스, orchestrator, specialist, low-token 에이전트 팀, session lifecycle, status message UX, hook relay, stale action, startup recovery 관련 작업이면 `remote-claude-harness` 스킬을 사용하라. 단순 질문은 직접 응답 가능.

**변경 이력:**
| 날짜 | 변경 내용 | 대상 | 사유 |
|------|----------|------|------|
| 2026-04-16 | 초기 하네스 구성 | `.claude/agents/`, `.claude/skills/remote-claude-harness`, `CLAUDE.md` | 새 세션에서도 짧은 호출문으로 하네스를 트리거하기 위해 |
