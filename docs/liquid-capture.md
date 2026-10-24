# Windows 플로팅 블러/위젯 화면 캡처 충돌 해결 & TC 구축 엔지니어링 가이드

> **대상**: Windows 환경에서 투명/블러(Liquid, Glass, Acrylic) 효과를 위해 데스크톱 배경을 실시간 캡처하는 플로팅 앱(Tauri, Electron, WPF, Win32)을 개발하는 엔지니어링 팀.

---

## 1. 근본 원인: DWM 캡처 제외(`0x11`)의 모순

- **앱 내부 캡처 요구**: 데스크톱 배경을 GDI `BitBlt` 등으로 캡처할 때, 자기 자신 윈도우가 찍히면 잔상이 겹치므로 `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` (`0x11`)를 적용해야 함.
- **사용자 캡처 도구 요구**: 사용자가 캡처 도구(Snipping Tool, ShareX, 픽픽 등)로 화면을 찍을 때는 우리 위젯이 선명히 보여야 하므로 `0x00`이어야 함.
- **핵심 원칙**: **"내 앱이 캡처하는 1ms 동안만 `0x11`을 켜고 즉시 `0x00`으로 복구한다(Zero-Exclusion). 사용자가 화면을 캡처하려는 모든 순간(단축키 입력, 포커스 상실 등)에는 무조건 `0x00`이어야 한다."**

---

## 2. 현장 발생 6대 결함 및 원인 요약

| 결함 | 증상 | 근본 원인 |
|:---|:---|:---|
| **1. 상시 증발** | 화면 캡처 시 위젯이 사라짐 | 캡처 루프 종료 후에도 친화도를 `0x00`으로 복원하지 않고 `0x11`로 유지 |
| **2. 핫키 레이스** | 가만히 둘 땐 찍히나 마우스 호버 직후 누르면 증발 | 호버 시 400ms 동안 30 FPS로 캡처를 돌려 0x11 상태가 지속되는 틈에 캡처 도구가 동작 |
| **3. 드롭 직후 증발** | 노치를 드래그 후 놓자마자 캡처하면 증발 | 정지된 배경에 대해 0바이트(동일 프레임)가 오면서 프론트엔드의 친화도 해제 호출이 누락됨 |
| **4. Blur 촉발 증발** | 캡처 단축키를 누르는 순간 증발 | 캡처 도구 활성화로 인한 `window blur` 이벤트 핸들러가 무심코 캡처 루프를 재실행(`loop.refresh()`)하여 0x11 진입 |
| **5. ESC 취소 먹통** | 캡처 화면에서 ESC를 눌러도 취소가 안 됨 | 항상 위(`TOPMOST`) 위젯이 포인터 캡처 및 OS 포커스를 독점하여 타 창으로 키 입력이 전달되지 못함 |
| **6. 포커스 핑퐁** | ESC 입력 시 동료 플로팅 위젯으로 포커스가 튐 | 하위 창 탐색 시 바로 아래 위치한 다른 플로팅 위젯을 픽업하여 포커스가 상호 순환 |

---

## 3. 핵심 코드 수정 가이드 (Actionable Code)

### 3.1 프론트엔드 (JavaScript / TypeScript)

#### ① `blur` 이벤트 시 캡처 루프 절대 재실행 금지 (결함 4 방어)
```typescript
// ❌ 잘못된 코드: blur 핸들러에서 캡처 루프를 리프레시함
window.addEventListener("blur", () => {
  loop.refresh(); // 캡처 도구가 뜨는 찰나에 0x11로 전환되어 스크린샷에서 증발!
});

// ✅ 올바른 코드: 백드롭 리스만 즉시 반납하고 루프는 건드리지 않음
window.addEventListener("blur", () => {
  lastChangedAt = 0;
  void invoke("release_liquid_backdrop"); // 친화도 0x00 강제 복구
});
```

#### ② 0바이트 캐시 히트 프레임에서도 무조건 릴리즈 (결함 3 방어)
```typescript
paint: (frames, now) => {
  let painted = false;
  frames.forEach(({ entry, pixels, width, height }) => {
    // 렌더링 로직...
    painted = true;
  });

  // 드래그 중이 아니라면 painted 여부(0바이트 동일 프레임)와 상관없이 무조건 릴리즈 호출!
  if (!rail.classList.contains("is-dragging")) {
    void invoke("release_liquid_backdrop");
    if (!painted && now - lastChangedAt >= SETTLING_AFTER_MS) {
      lastChangedAt = 0; // 유휴 상태로 즉시 전환
    }
  }
}
```

#### ③ ESC 및 PrintScreen 시 포인터 캡처 해제 & 포커스 양보 (결함 5 방어)
```typescript
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    drag.cancel(); // releasePointerCapture() 호출 (반드시 try-catch 감쌀 것)
    void invoke("yield_notch_focus", { forwardEscape: true });
  } else if (e.key === "PrintScreen") {
    drag.cancel();
    void invoke("yield_notch_focus", { forwardEscape: false });
  }
}, { capture: true });
```

---

### 3.2 백엔드 (Rust / Win32 / C++)

#### ① 단일 프레임 캡처 직후 즉각 0x00 반납 (결함 1, 2 방어)
```rust
// 정적 프레임(호버, 정착) 캡처 시 비동기 대기 없이 백엔드 내부에서 즉시 0x00 복원
if !is_dragging {
    let bytes = capture_pixels(hwnd)?;
    set_window_display_affinity(hwnd, 0x00)?; // 15ms 이내 복구 완료
    return Ok(bytes);
}
```

#### ② 하위 창 포커스 양보 시 동료 위젯 필터링 (결함 6 방어)
```rust
pub fn is_companion_widget_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("token usage")
        || lower.contains("tokenusage")
        || lower.contains("economy war room")
        || lower.contains("economywarroom")
}

// EnumWindows 콜백 내부
if w > 50 && h > 50 {
    let title = get_window_text(h);
    // 동료 위젯은 건너뛰고 실제 사용자 작업 창/캡처 도구로만 포커스 인계
    if is_companion_widget_title(&title) {
        return TRUE;
    }
    target = Some(h);
    return FALSE;
}
```

---

## 4. 왜 기존 테스트(TC)는 이 버그를 놓쳤는가?

1. **접착 레이어(Glue Layer) 테스트 부재**:
   - 개별 모듈(루프 계산기, 색상 대비 수학식)은 100% 모킹되어 통과했으나, DOM 이벤트(`blur`, `focus`)와 Tauri IPC를 바인딩하는 통합 진입점의 테스트가 누락됨.
2. **긍정 검증(Positive Assertion) 편향**:
   - "호버했을 때 캡처가 실행되는가(`expect(captured).toBe(true)`)"만 검증함.
   - 핵심 불변식인 **"비활성화(`blur`) 시 캡처가 절대로 스케줄링되지 않아야 한다(`expect(captured).toBe(false)`)"**는 부정 검증(Negative Invariant)이 없었음.
3. **다중 윈도우(Z-Order) 시뮬레이션 한계**:
   - Cargo/Node 가상 환경에서는 단일 윈도우만 떠 있으므로, 두 개 이상의 플로팅 앱이 Z-order 상 겹칠 때 발생하는 포커스 핑퐁 현상이 재현되지 못함.

---

## 5. 필수 자동화 테스트 케이스 (TC Matrix)

유사 앱에서 동일한 회귀 버그를 방지하기 위해 반드시 작성해야 하는 핵심 테스트입니다:

| TC ID | 테스트 시나리오 | 검증 단언 (Assertion) | 핵심 목적 |
|:---|:---|:---|:---|
| **TC-CAP-01** | 정착 루프 0바이트 캐시 히트 | `!isDragging` 시 `painted: false`여도 `release_liquid_backdrop` 호출 | 드래그 종료 직후 0x00 복원 보장 |
| **TC-CAP-02** | 백엔드 동기 반납 | 정적 프레임 캡처 후 즉시 `SetWindowDisplayAffinity(0x00)` 실행 | 0x11 체류 시간을 15ms 미만으로 제한 |
| **TC-CAP-03** | 드래그 종료 네이티브 릴리즈 | 마우스 버튼 up 시 네이티브 핸들러에서 리스 파기 | 드롭 애니메이션 중 캡처 허용 |
| **TC-CAP-04** | ESC 키 포커스 양보 | Escape 입력 시 `releasePointerCapture` 및 `yield_notch_focus` 호출 | 캡처 오버레이 닫힘 보장 |
| **TC-CAP-05** | PointerCapture 예외 방어 | `releasePointerCapture`가 예외를 던져도 정상 릴리즈 지속 | 크래시 방지 안전망 |
| **TC-CAP-06** | 동료 위젯 타이틀 필터링 | `is_companion_widget_title("App")` 단위 테스트 검증 | 다중 위젯 포커스 핑퐁 차단 |
| **TC-CAP-07** | **Window Blur 시 캡처 불발** | `window.dispatchEvent("blur")` 시 `capture_liquid_backdrop` 미호출 | **캡처 단축키 입력 시 증발 완벽 차단** |

### TC-CAP-07 구현 예시 (Node / Vitest / Jest)
```javascript
test("window blur releases backdrop and strictly DOES NOT schedule capture (TC-CAP-07)", async () => {
  const { renderer } = fixture();
  renderer.setEnabled(true);
  await settle();

  invocations.length = 0; // 초기화

  // 사용자가 캡처 단축키(Alt + ` 등)를 눌러 창 포커스가 빠져나가는 상황
  window.dispatchEvent(new Event("blur"));
  await settle();

  // 1. 친화도 반납은 즉각 호출되어야 함
  assert.equal(invocations.some(i => i.cmd === "release_liquid_backdrop"), true);

  // 2. 캡처 루프가 절대 재스케줄링되어서는 안 됨 (부정 불변식 단언)
  assert.equal(invocations.some(i => i.cmd === "capture_liquid_backdrop"), false);
});
```
