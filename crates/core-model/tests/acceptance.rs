//! Feature: AI 에이전트 타입 식별
//!
//! Slack 슬래시 커맨드로 AI 에이전트를 식별하고 사용자에게 이름을 표시한다.
//!
//! Feature: 세션 상태 표시 레이블 (Tell Don't Ask 개선)
//!
//! 세션 상태가 표시 레이블을 스스로 알고 있어야 한다.
//! 외부에서 상태를 꺼내 분기하는 대신, 상태 객체에게 직접 레이블을 요청한다.

use core_model::{AgentType, SessionState, TurnId};

// ── 픽스처 ───────────────────────────────────────────────────────────────────

fn 슬래시_커맨드로_에이전트_타입_파싱(cmd: &str) -> AgentType {
    AgentType::from_slash_command(cmd)
}

fn 세션_상태_표시_레이블(state: &SessionState) -> &'static str {
    state.display_label()
}

// ── Feature: AI 에이전트 타입 식별 ───────────────────────────────────────────

/// Scenario: /cc 커맨드로 Claude Code 에이전트를 시작한다
///   Given  Slack에서 "/cc" 슬래시 커맨드가 수신된다
///   When   에이전트 타입을 파싱한다
///   Then   Claude Code 에이전트로 식별된다
///   And    사용자에게 "Claude Code"로 표시된다
#[test]
fn cc_커맨드는_claude_code_에이전트다() {
    let 에이전트_타입 = 슬래시_커맨드로_에이전트_타입_파싱("/cc");

    assert_eq!(에이전트_타입, AgentType::ClaudeCode);
    assert_eq!(에이전트_타입.display_name(), "Claude Code");
}

/// Scenario: /cx 커맨드로 Codex 에이전트를 시작한다
///   Given  Slack에서 "/cx" 슬래시 커맨드가 수신된다
///   When   에이전트 타입을 파싱한다
///   Then   Codex 에이전트로 식별된다
///   And    사용자에게 "Codex"로 표시된다
#[test]
fn cx_커맨드는_codex_에이전트다() {
    let 에이전트_타입 = 슬래시_커맨드로_에이전트_타입_파싱("/cx");

    assert_eq!(에이전트_타입, AgentType::Codex);
    assert_eq!(에이전트_타입.display_name(), "Codex");
}

/// Scenario: /gm 커맨드로 Gemini 에이전트를 시작한다
///   Given  Slack에서 "/gm" 슬래시 커맨드가 수신된다
///   When   에이전트 타입을 파싱한다
///   Then   Gemini 에이전트로 식별된다
///   And    사용자에게 "Gemini"로 표시된다
#[test]
fn gm_커맨드는_gemini_에이전트다() {
    let 에이전트_타입 = 슬래시_커맨드로_에이전트_타입_파싱("/gm");

    assert_eq!(에이전트_타입, AgentType::Gemini);
    assert_eq!(에이전트_타입.display_name(), "Gemini");
}

/// Scenario: 알 수 없는 커맨드는 Claude Code로 기본 처리된다
///   Given  Slack에서 알 수 없는 슬래시 커맨드가 수신된다
///   When   에이전트 타입을 파싱한다
///   Then   Claude Code 에이전트로 기본 처리된다
#[test]
fn 알_수_없는_커맨드는_claude_code로_기본_처리된다() {
    let 에이전트_타입 = 슬래시_커맨드로_에이전트_타입_파싱("/unknown");

    assert_eq!(에이전트_타입, AgentType::ClaudeCode);
}

// ── Feature: 세션 상태 표시 레이블 ───────────────────────────────────────────
// RED: SessionState::display_label() 메서드가 아직 없음.
// GREEN: core-model에 display_label() 추가 후 통과.

/// Scenario: Idle 상태는 "대기 중" 레이블을 반환한다
///   Given  세션이 Idle 상태이다
///   When   표시 레이블을 요청한다
///   Then   "Ready for next prompt." 레이블이 반환된다
#[test]
fn idle_상태는_대기_중_레이블을_반환한다() {
    assert_eq!(세션_상태_표시_레이블(&SessionState::Idle), "Ready for next prompt.");
}

/// Scenario: Starting 상태는 "작업 중" 레이블을 반환한다
#[test]
fn starting_상태는_작업_중_레이블을_반환한다() {
    assert_eq!(세션_상태_표시_레이블(&SessionState::Starting), "⏳ Working...");
}

/// Scenario: Running 상태는 "작업 중" 레이블을 반환한다
#[test]
fn running_상태는_작업_중_레이블을_반환한다() {
    let 상태 = SessionState::Running { active_turn: TurnId::new() };
    assert_eq!(세션_상태_표시_레이블(&상태), "⏳ Working...");
}

/// Scenario: Cancelling 상태는 "작업 중" 레이블을 반환한다
#[test]
fn cancelling_상태는_작업_중_레이블을_반환한다() {
    let 상태 = SessionState::Cancelling { active_turn: TurnId::new() };
    assert_eq!(세션_상태_표시_레이블(&상태), "⏳ Working...");
}

/// Scenario: Completed 상태는 "완료" 레이블을 반환한다
///   Given  세션이 Completed 상태이다
///   When   표시 레이블을 요청한다
///   Then   "Completed." 레이블이 반환된다
#[test]
fn completed_상태는_완료_레이블을_반환한다() {
    assert_eq!(세션_상태_표시_레이블(&SessionState::Completed), "Completed.");
}

/// Scenario: Failed 상태는 "실패" 레이블을 반환한다
///   Given  세션이 Failed 상태이다
///   When   표시 레이블을 요청한다
///   Then   "Failed." 레이블이 반환된다
#[test]
fn failed_상태는_실패_레이블을_반환한다() {
    let 상태 = SessionState::Failed { reason: "런타임 오류".to_string() };
    assert_eq!(세션_상태_표시_레이블(&상태), "Failed.");
}

/// Scenario: WaitingForApproval 상태는 "승인 대기" 레이블을 반환한다
///   Given  세션이 WaitingForApproval 상태이다
///   When   표시 레이블을 요청한다
///   Then   "Waiting for approval." 레이블이 반환된다
#[test]
fn waiting_for_approval_상태는_승인_대기_레이블을_반환한다() {
    assert_eq!(
        세션_상태_표시_레이블(&SessionState::WaitingForApproval),
        "Waiting for approval."
    );
}
