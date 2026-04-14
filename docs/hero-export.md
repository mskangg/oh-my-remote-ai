# Hero Export

## 목표

`docs/hero-mock-v18.html`을 README에 바로 넣을 수 있는 hero asset으로 반복 가능하게 export합니다.

## 기본 흐름

1. 브라우저에서 `docs/hero-mock-v18.html`을 엽니다.
2. 10~15초 정도의 짧은 루프를 녹화합니다.
3. 제품 프레임만 남기도록 crop 합니다.
4. `docs/images/hero-demo.gif`로 export 합니다.
5. GIF가 너무 크면 `.mp4` 원본을 남기고 다시 인코딩합니다.

## 권장 캡처 기준

- 2x retina 해상도
- 프레임 전체가 잘 보이도록 여백 최소화
- `/cc` → 세션 시작 → thread 명령 → status 갱신 → 최종 reply까지 한 루프로 보이기
- CLI / terminal 감성이 살아 있도록 dark terminal 패널이 잘 보이게 유지

## ffmpeg 예시

```bash
ffmpeg -i hero-demo.mov -vf "fps=12,scale=1400:-1:flags=lanczos" docs/images/hero-demo.gif
```

## 최종 체크

- README에서 바로 참조 가능한 경로인가?
- 용량이 너무 크지 않은가?
- 첫 3초 안에 제품 가치가 보이는가?
- Slack-first 메시지와 기존 작업환경 continuity가 보이는가?
