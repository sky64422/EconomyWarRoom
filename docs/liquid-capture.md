# Liquid & Glass Desktop Capture Lease Architecture

이 문서는 Liquid 및 Glass 테마에서 바탕화면 블러/굴절 텍스처를 캡처할 때 발생하는 **화면 캡처 시 위젯 사라짐(Screenshot Exclusion)** 현상의 근본 원인과 최종 개선된 임대(Lease) 아키텍처를 정리합니다.

---

## 1. 문제 현상 및 배경

### 1.1 증상
- Liquid/Glass 테마 적용 상태에서 Windows 화면 캡처 도구(예: `Win + Shift + S`, 캡처 및 스케치, Snipping Tool, PrintScreen)를 실행하면, 바탕화면 위의 위젯 노치가 캡처 영역에서 투명하게 사라지거나 빈 공간으로 캡처됨.

### 1.2 왜 이전 릴리즈 버전과 차이가 있었는가?
- **이전 실행 버전**: 화면 잠금(Workstation Lock) 해제나 절전 모드 복귀 시 일시적으로 GDI `BitBlt`가 실패하면서 JS의 `catch` 블록으로 진입, `release_liquid_backdrop`이 호출되어 윈도우 디스플레이 친화도(Display Affinity)가 `0`(`WDA_NONE`)으로 풀린 채 CSS fallback 배경(`rgba(29,39,52)`) 상태로 유지되었기 때문.
- **새로 실행된 버전**: 데스크톱 캡처 루프가 지속적으로 가동되면서 캡처 임대(Lease)를 끊임없이 갱신하여 상시 `WDA_EXCLUDEFROMCAPTURE` 상태가 유지됨.

---

## 2. 근본 기술적 원인 (Deep Root Cause)

Windows DWM에서 위젯 창 뒤편의 데스크톱 화면을 캡처할 때, 위젯 창 자신이 캡처 버퍼에 포함되면 무한 재귀 블러('무한 거울상/hall of mirrors') 현상이 발생합니다. 이를 방지하기 위해 캡처 시점에 다음 API를 적용합니다:
```rust
SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) // value: 0x11
```
이 속성이 지정된 창은 DWM의 모든 화면 캡처 버퍼(데스크톱 복제, Snipping Tool, Win+Shift+S 등)에서 제외됩니다.

### 치명적이었던 구조적 버그 2가지

1. **`release_liquid_backdrop`과 `watch_lease`의 프레임 캐시 파괴 (`state.1.reset()`)**:
   - `release_liquid_backdrop` 및 `watch_lease`가 만료될 때마다 `state.1.reset()`을 호출하고 있었습니다.
   - `CaptureWorker::reset()`은 워커 스레드의 `surfaces`를 `[None, None]`으로 초기화하여 GDI DC와 `FrameCache`를 매번 파괴했습니다.
   - 그 결과, 다음 프레임 캡처 시 `FrameCache`가 비어있으므로 바탕화면이 전혀 바뀌지 않았음에도 불구하고 `self.source == bgra` 비교가 불가능해져 항상 "새 프레임(changed)"으로 판정되어 픽셀 데이터를 반환했습니다.
   - 프론트엔드는 픽셀이 계속 반환되므로 `paint` 콜백에서 `lastChangedAt = now`를 매 틱마다 갱신하여 화면 정지 시간(`now - lastChangedAt`)이 0ms에 영구 고정되었습니다.

2. **정지 상태 미도달로 인한 무한 30 FPS 폴링 & 상시 임대 잠금**:
   - `lastChangedAt`이 매 틱마다 갱신되므로 루프의 `interval`이 영원히 `IDLE_FRAME_MS` (33ms = 30 FPS)로 유지되었습니다.
   - Rust 백엔드의 임대 수명(`CAPTURE_LEASE` = 80ms)보다 빠른 33ms 주기로 `capture_liquid_backdrop`이 끊임없이 호출되면서 임대가 무한히 갱신되었습니다.
   - 그 결과 `WDA_EXCLUDEFROMCAPTURE`가 **100% 영구적으로 유지**되어, 사용자가 언제 스크린샷을 찍어도 위젯이 항상 사라지게 되었습니다.

---

## 3. 최종 개선 아키텍처 (Frame Cache 보존 + Zero-Polling Static Sleep)

핵심 원칙: **"화면이 정지하면 캡처 루프를 완전 수면(Infinity) 상태로 전환하여 Affinity 0을 100% 유지하고, 마우스 인터랙션 시에만 즉각 복귀한다."**

```mermaid
flowchart TD
    A["이동 / 마우스 인터랙션 (pointerenter / move / drag)"] -->|loop.refresh() 즉각 깨움| B["30~60fps 고속 캡처"]
    B --> C["Rust: WDA_EXCLUDEFROMCAPTURE (80ms Lease)"]
    C --> D["FrameCache 유지: 동일 픽셀 시 0 byte 반환"]
    D --> E{"정지 시간 >= 2000ms ?"}
    E -- 아니오 --> B
    E -- 예 --> F["invoke('release_liquid_backdrop') -> Affinity=0"]
    F --> G["interval = Infinity -> 루프 완전 수면 (0% CPU/GPU)"]
    G --> H["캔버스 텍스처 영구 유지 + 스크린샷 100% 정상 포함"]

    subgraph 포커스 아웃 즉각 반응
        I["Win + Shift + S 실행"] --> J["window 'blur' 이벤트 감지"]
        J --> K["즉시 invoke('release_liquid_backdrop')"]
        K --> F
    end
```

### 3.1 세부 개선 내용

1. **`FrameCache` 및 GDI 서피스 영구 보존**:
   - `release_liquid_backdrop` 및 `watch_lease`에서 `state.1.reset()` 호출을 완전히 제거.
   - 바탕화면 픽셀이 바뀌지 않는 동안에는 Rust 백엔드가 `Ok(Vec::new())` (0 byte)를 반환하여 불필요한 페인팅 및 시간 리셋 방지.
   - 실제 테마 전환이나 위젯 파괴 시에만 GDI를 정리하도록 `destroy_liquid_backdrop` 신설.

2. **정지(Settle) 시 완전 수면 (`interval = Infinity`)**:
   - 2초간 변화가 없으면(`unchangedFor >= STATIC_AFTER_MS`), `interval`이 `Infinity`를 반환.
   - 불필요한 백그라운드 캡처 호출을 완전히 중단(Zero Polling).
   - `release_liquid_backdrop` 호출로 `WDA_EXCLUDEFROMCAPTURE`를 완전히 해제하여 평상시 100%의 시간 동안 Affinity `0` 유지.
   - 이미 캔버스에 그려진 리퀴드 블러는 그대로 유지되어 시각적 끊김이 전혀 없음.

3. **마우스 및 인터랙션 시 즉각 깨움 (Zero Latency Wake-up)**:
   - `rail.addEventListener("pointerenter")`
   - `rail.addEventListener("pointermove")`
   - `window.addEventListener("pointerdown")`
   - 드래그 시작/종료 (`is-dragging`), 디테일 카드 열기/닫기 (`hidden`), 윈도우 이동/모니터 전환
   - 위 모든 이벤트 발생 시 즉시 `lastChangedAt = performance.now(); loop.refresh();`를 호출하여 60fps/30fps로 즉각 깨어남.

4. **캡처 도구 포커스 아웃 즉시 감지 (Blur 안전망)**:
   - `window.addEventListener("blur", () => { void invoke("release_liquid_backdrop"); })` 등록 유지.

---

## 4. 적용 프로젝트 및 검증 결과

[EconomyWarRoom](file:///C:/dev/EconomyWarRoom) 및 [TokenUsage](file:///C:/dev/TokenUsage) 두 프로젝트에 동일하게 적용 완료.

| 검증 항목 | EconomyWarRoom | TokenUsage | 결과 |
|---|---|---|---|
| Cargo Unit Tests (`liquid_glass`) | 145 / 145 통과 | 121 / 121 통과 | 정상 |
| Liquid Frames Integration | 2 / 2 통과 | 2 / 2 통과 | 정상 |
| UI State & Lifecycle Tests | 14 / 14 통과 | 89 / 89 통과 | 정상 |
| Notch Geometry Tests | 28 / 28 통과 | 28 / 28 통과 | 정상 |
| Risk Scenarios Tests | 9 / 9 통과 | 19 / 19 통과 | 정상 |
| Frontend Typecheck & Build | `tsc && vite build` 완료 | `tsc && vite build` 완료 | 정상 |
