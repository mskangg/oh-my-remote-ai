//! Feature: 명령 위험도 분류
//!
//! 사용자 명령의 위험도를 분류하여 승인 필요 여부를 결정한다.
//! OCP 원칙 준수를 위해 키워드 목록을 데이터로 분리하기 전,
//! 현재 분류 동작을 블랙박스 테스트로 보호한다.

use core_model::UserCommand;
use policy::{classify, CommandRisk};

// ── 픽스처 ───────────────────────────────────────────────────────────────────

fn 명령_위험도_분류(text: &str) -> CommandRisk {
    classify(&UserCommand { text: text.to_string() })
}

// ── 승인 필요 시나리오 ────────────────────────────────────────────────────────

/// Scenario: commit 명령은 승인이 필요하다
///   Given  사용자가 "commit changes" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   승인 필요(ApprovalRequired)로 분류된다
#[test]
fn commit_명령은_승인이_필요하다() {
    assert_eq!(명령_위험도_분류("commit changes"), CommandRisk::ApprovalRequired);
}

/// Scenario: delete 명령은 승인이 필요하다
///   Given  사용자가 "delete the file" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   승인 필요(ApprovalRequired)로 분류된다
#[test]
fn delete_명령은_승인이_필요하다() {
    assert_eq!(명령_위험도_분류("delete the file"), CommandRisk::ApprovalRequired);
}

/// Scenario: remove 명령은 승인이 필요하다
///   Given  사용자가 "remove unused imports" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   승인 필요(ApprovalRequired)로 분류된다
#[test]
fn remove_명령은_승인이_필요하다() {
    assert_eq!(명령_위험도_분류("remove unused imports"), CommandRisk::ApprovalRequired);
}

/// Scenario: edit 명령은 승인이 필요하다
///   Given  사용자가 "edit config.toml" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   승인 필요(ApprovalRequired)로 분류된다
#[test]
fn edit_명령은_승인이_필요하다() {
    assert_eq!(명령_위험도_분류("edit config.toml"), CommandRisk::ApprovalRequired);
}

/// Scenario: 대소문자를 구분하지 않고 위험도를 분류한다
///   Given  사용자가 "COMMIT changes" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   승인 필요(ApprovalRequired)로 분류된다
#[test]
fn 대소문자_구분_없이_위험도를_분류한다() {
    assert_eq!(명령_위험도_분류("COMMIT changes"), CommandRisk::ApprovalRequired);
    assert_eq!(명령_위험도_분류("DELETE all"), CommandRisk::ApprovalRequired);
    assert_eq!(명령_위험도_분류("REMOVE file"), CommandRisk::ApprovalRequired);
    assert_eq!(명령_위험도_분류("EDIT settings"), CommandRisk::ApprovalRequired);
}

// ── 안전 시나리오 ─────────────────────────────────────────────────────────────

/// Scenario: 일반 분석 명령은 안전하다
///   Given  사용자가 "analyze the failing test" 명령을 입력한다
///   When   위험도를 분류한다
///   Then   안전(Safe)으로 분류된다
#[test]
fn 일반_분석_명령은_안전하다() {
    assert_eq!(명령_위험도_분류("analyze the failing test"), CommandRisk::Safe);
}

/// Scenario: 코드 조회 명령은 안전하다
#[test]
fn 코드_조회_명령은_안전하다() {
    assert_eq!(명령_위험도_분류("explain this function"), CommandRisk::Safe);
    assert_eq!(명령_위험도_분류("show me the logs"), CommandRisk::Safe);
    assert_eq!(명령_위험도_분류("run the tests"), CommandRisk::Safe);
}

/// Scenario: 빈 명령은 안전하다
#[test]
fn 빈_명령은_안전하다() {
    assert_eq!(명령_위험도_분류(""), CommandRisk::Safe);
}
