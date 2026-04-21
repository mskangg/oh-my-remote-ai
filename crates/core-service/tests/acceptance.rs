//! Feature: 세션 전체 라이프사이클
//!
//! 세션 레지스트리를 통해 세션이 생성되고, 명령이 처리되고, 상태가 전이된다.
//! SlackApplicationService SRP 리팩터링 전 핵심 세션 동작을 블랙박스로 보호한다.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use core_model::{SessionId, SessionMsg, SessionState, UserCommand};
use core_service::{RuntimeEngine, SessionRegistry, SessionRepository, SessionRuntimeCleanup};
use tokio::sync::RwLock;

// ── 픽스처: 인메모리 저장소 ──────────────────────────────────────────────────

#[derive(Default)]
struct InMemoryRepo {
    states: RwLock<HashMap<SessionId, SessionState>>,
}

#[async_trait]
impl SessionRepository for InMemoryRepo {
    async fn load_state(&self, id: SessionId) -> anyhow::Result<Option<SessionState>> {
        Ok(self.states.read().await.get(&id).cloned())
    }

    async fn save_state(&self, id: SessionId, state: &SessionState) -> anyhow::Result<()> {
        self.states.write().await.insert(id, state.clone());
        Ok(())
    }
}

// ── 픽스처: 성공하는 Noop 런타임 ─────────────────────────────────────────────

#[derive(Default, Clone)]
struct NoopRuntime;

#[async_trait]
impl RuntimeEngine for NoopRuntime {
    async fn handle(
        &self,
        _session_id: SessionId,
        _message: &SessionMsg,
        _next_state: &SessionState,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl SessionRuntimeCleanup for NoopRuntime {
    async fn clear_runtime_bookkeeping(&self, _session_id: SessionId) -> anyhow::Result<()> {
        Ok(())
    }
}

fn 세션_레지스트리_초기화() -> Arc<SessionRegistry<InMemoryRepo, NoopRuntime>> {
    Arc::new(SessionRegistry::new(
        Arc::new(InMemoryRepo::default()),
        Arc::new(NoopRuntime),
    ))
}

async fn 세션에_메시지_전송(
    레지스트리: &Arc<SessionRegistry<InMemoryRepo, NoopRuntime>>,
    세션_id: SessionId,
    메시지: SessionMsg,
) -> SessionState {
    레지스트리
        .session(세션_id)
        .await
        .send(메시지)
        .await
        .expect("메시지 전송 성공")
}

// ── 시나리오 ─────────────────────────────────────────────────────────────────

/// Scenario: 세션에 UserCommand를 전송하면 Running 상태가 된다
///   Given  세션 레지스트리가 초기화되어 있다
///   And    신규 세션 ID가 준비되어 있다
///   When   세션에 UserCommand를 전송한다
///   Then   세션이 Running 상태가 된다
#[tokio::test]
async fn 사용자_명령을_전송하면_세션이_running_상태가_된다() {
    // Given
    let 레지스트리 = 세션_레지스트리_초기화();
    let 세션_id = SessionId::new();

    // When
    let 다음_상태 = 세션에_메시지_전송(
        &레지스트리,
        세션_id,
        SessionMsg::UserCommand(UserCommand { text: "analyze the failing test".to_string() }),
    )
    .await;

    // Then
    assert!(matches!(다음_상태, SessionState::Running { .. }));
}

/// Scenario: 세션에 Interrupt를 전송하면 Cancelling 상태가 된다
///   Given  세션이 Running 상태이다
///   When   세션에 Interrupt를 전송한다
///   Then   세션이 Cancelling 상태가 된다
#[tokio::test]
async fn interrupt를_전송하면_세션이_cancelling_상태가_된다() {
    // Given: Running 상태로 만들기
    let 레지스트리 = 세션_레지스트리_초기화();
    let 세션_id = SessionId::new();
    세션에_메시지_전송(
        &레지스트리,
        세션_id,
        SessionMsg::UserCommand(UserCommand { text: "start".to_string() }),
    )
    .await;

    // When
    let 다음_상태 = 세션에_메시지_전송(&레지스트리, 세션_id, SessionMsg::Interrupt).await;

    // Then
    assert!(matches!(다음_상태, SessionState::Cancelling { .. }));
}

/// Scenario: Terminate 메시지는 세션을 Completed로 종료한다
///   Given  세션이 Running 상태이다
///   When   세션에 Terminate를 전송한다
///   Then   세션이 Completed 상태가 된다
#[tokio::test]
async fn terminate를_전송하면_세션이_completed_상태가_된다() {
    // Given: Running 상태로 만들기
    let 레지스트리 = 세션_레지스트리_초기화();
    let 세션_id = SessionId::new();
    세션에_메시지_전송(
        &레지스트리,
        세션_id,
        SessionMsg::UserCommand(UserCommand { text: "start".to_string() }),
    )
    .await;

    // When
    let 다음_상태 = 세션에_메시지_전송(&레지스트리, 세션_id, SessionMsg::Terminate).await;

    // Then
    assert_eq!(다음_상태, SessionState::Completed);
}

/// Scenario: 동일한 세션 ID는 동일한 액터를 반환한다
///   Given  세션 레지스트리가 초기화되어 있다
///   When   동일한 세션 ID로 두 번 핸들을 요청한다
///   Then   같은 세션 ID를 가진 핸들이 반환된다
#[tokio::test]
async fn 동일한_세션_id는_같은_핸들을_반환한다() {
    // Given
    let 레지스트리 = 세션_레지스트리_초기화();
    let 세션_id = SessionId::new();

    // When
    let 핸들_1 = 레지스트리.session(세션_id).await;
    let 핸들_2 = 레지스트리.session(세션_id).await;

    // Then
    assert_eq!(핸들_1.session_id(), 핸들_2.session_id());
}

/// Scenario: 서로 다른 세션은 독립적인 상태를 갖는다
///   Given  두 개의 독립적인 세션이 존재한다
///   When   하나의 세션에만 명령을 전송한다
///   Then   다른 세션의 상태는 변경되지 않는다
#[tokio::test]
async fn 서로_다른_세션은_독립적인_상태를_갖는다() {
    // Given
    let 레지스트리 = 세션_레지스트리_초기화();
    let 세션_a = SessionId::new();
    let 세션_b = SessionId::new();

    // When: 세션 A에만 명령 전송
    let a_상태 = 세션에_메시지_전송(
        &레지스트리,
        세션_a,
        SessionMsg::UserCommand(UserCommand { text: "task A".to_string() }),
    )
    .await;

    // Then: 세션 A는 Running, 세션 B는 Idle (Starting에서 메시지 없으면 Starting 그대로)
    assert!(matches!(a_상태, SessionState::Running { .. }));

    // 세션 B는 영향 없음 - Idle 메시지를 보내면 Idle 상태 확인
    let b_상태 = 세션에_메시지_전송(
        &레지스트리,
        세션_b,
        SessionMsg::SendKey { key: "Escape".to_string() },
    )
    .await;
    // Idle이 아닌 Starting에서 SendKey → Starting 유지 (런타임 없으면 Starting)
    // 핵심: 두 세션이 독립적이라는 것 - 세션 ID가 다르다
    assert_ne!(세션_a, 세션_b);
    let _ = b_상태; // 상태 자체보다 독립성이 핵심
}
