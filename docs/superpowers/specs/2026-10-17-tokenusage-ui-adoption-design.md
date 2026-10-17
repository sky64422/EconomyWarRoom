# TokenUsage UI 이식 검토 및 설계안

작성: 2026-10-17. 상태: 사용자 승인 후 구현 및 검증. 현재 결과는 `docs/testing.md` 참조.

> 최신 사용자 지시가 아래 초기 설계의 관련 조항보다 우선한다. 노치의 `+`/제거 `×`를 모두 없애고 최대 여섯 종목(초과 시 마지막은 목록)을 표시한다. 설정 `종목` 탭은 검색창을 항상 표시하며 종목 수/상단 닫기/상세 이동 버튼이 없다. 종목 행 클릭은 상세, 드래그는 순서 변경, 우측 휴지통은 제거다. 키보드 단축 조작과 전역 핫키는 제거하며 종목 코드 입력/일반 텍스트 편집은 유지한다. TokenUsage의 호환되는 설정·재질·피드백 요소는 [이식 목록](../../ui-references.md#tokenusage-transfer-inventory)을 따른다. 기존 JSON의 hotkey 필드는 호환성을 위해 남지만 실행되지 않는다.

## 1. 합의된 방향

TokenUsage의 노치, 접기/펼치기, 안쪽 상세 카드, 설정, 다크/글라스/리퀴드 재질과 조작 피드백을 EconomyWarRoom에 전반적으로 이식한다.

사용자가 지정한 정보 구분:

- 노치: 종목의 대표 아이콘과 그 아래 등락률.
- 등락률: **현재 구현의 윗줄**, 즉 최신 가격에 대응하는 `Quote.display.primary.change`를 그대로 사용한다. 정규장 전용 값이나 전일 대비 별도 계산으로 바꾸지 않는다.
- 해당 종목을 누르면 그 종목의 상세 카드가 열린다.
- 상세 카드: 지금처럼 스파크라인과 가격/등락률 두 줄을 함께 표시한다.

아래의 셀 수, 크기, 초과 종목 접근, 관리 탭 구성은 이 요구를 구현하기 위한 제안이다. 기존의 긴 목록을 상세 패널에 그대로 펼치는 안은 채택하지 않는다.

## 2. 현재 코드에서 확인한 것

| 항목 | EconomyWarRoom | TokenUsage | 이식 판단 |
|---|---|---|---|
| 기준 커밋 | `36210ba8d0809d6b6587e7e001178959b8b28d58` | `251df6bfdceda1bf29f1696e503b72e3cce907e1` | 실행 시 다시 확인 |
| 기반 | Tauri 2, vanilla TS, Vite, CSS, Pretendard | 동일 | React/컴포넌트 프레임워크 전환 불필요 |
| 기본 형태 | 자유 이동/리사이즈하는 세로 목록 | 화면 가장자리 노치 + 안쪽 카드 | 창 기하와 입력 처리는 Rust까지 이식 |
| 창 크기 | `app.ts`의 content-hug + native 최소 크기 clamp | 고정 투명 canvas 안에서 가시 영역 변경 | 두 크기 제어를 동시에 쓰면 안 됨 |
| 모서리 | DWM ROUND, CSS radius 0 | DWM DONOTROUND + SVG 오목한 어깨 | 노치에 기존 DWM rounding을 적용하지 않음 |
| 시각 재질 | 다크 글라스, 불투명도/가독성 floor | 다크 / 반사 글라스 / 실제 배경 굴절 리퀴드 | 세 재질 모두 목표 범위 |
| 데이터 | Yahoo 시세·스파크라인, scheduler, AppCore | 벤더 사용률·activity | 시세 파이프라인은 EconomyWarRoom 유지 |
| 설정 | 단일 overlay | 고정 header/tab + body scroll, 탭 공통 높이 | `모양 / 종목 / 일반`으로 맞춤 |

원본 주요 파일: `C:/dev/TokenUsage/src/ui/notch.ts`, `notch-state.ts`, `notch-drag.ts`, `notch-surface.ts`, `surface-finish.ts`, `liquid-glass.ts`, `liquid-loop.ts`, `settings-panel.ts`, `src/styles/notch.css`, `src-tauri/src/domain/notch.rs`, `domain/reveal.rs`, `infrastructure/notch_window.rs`, `infrastructure/liquid_glass.rs`.

주의: 원본 `DESIGN.md`/`AGENTS.md`의 깊이 설명은 64 DIP이나 현재 `domain/notch.rs::DEPTH`는 **56 DIP**다. 검토안은 코드의 56을 기준으로 한다. 과거 screenshot은 최신 테마/크기를 증명하는 자료로 취급하지 않는다.

## 3. 노치에 표시할 정보

각 일반 셀은 다음 두 요소만 가진다.

```text
   (종목 아이콘)
      +1.24%
```

- 원형 틀의 아이콘을 사용한다. 사용률 진행 호, 채워진 퍼센트 링, 활동 중 회전 효과는 이식하지 않는다. 주가 등락률에는 용량의 0~100% 의미가 없다.
- 주식은 기업 대표 아이콘, 코인은 자산 아이콘을 사용한다. 같은 발행사의 복수 종목도 데이터와 선택은 symbol 문자열이 아니라 watchlist item ID로 구분한다.
- 아이콘이 없는 종목은 동일 크기의 중립 원 안에 짧은 티커 문자를 표시한다. 회사 로고가 없다는 이유로 종목 추가를 막지 않는다.
- 초기 아이콘은 출처와 사용 조건을 확인한 번들 registry로 제공한다. 원본 서비스 로고를 기업 로고로 대체 사용하지 않는다. 시세 tick마다 외부 이미지 서버를 요청하지 않는다.
- 아이콘 자체는 식별을 위한 고유색을 유지한다. 등락률만 기존 `--up` / `--down` / 중립 색과 부호로 구별한다. 0은 `0.00%`, 값 없음은 `—`다.
- 화면에 티커·가격·차트·통화·세션 pill을 추가하지 않는다. 접근성 이름에는 전체 티커, 종목명, 현재 등락률을 제공한다. 클릭/키보드로 상세를 열어 식별할 수 있어야 한다.
- 갱신 때 셀 순서/크기는 변하지 않는다. 등락률 순으로 자동 정렬하거나 종목을 자동 교체하지 않는다.

### 표시 종목 수와 넘침

관심목록 순서를 그대로 따른다. 별도의 노치 고정 목록은 만들지 않는다.

- 셀 예산은 최대 6개이며 작업 영역과 어깨 여백을 고려해 더 줄어들 수 있다. 세로 셀 72 DIP, 가로 셀 96 DIP를 출발점으로 한다.
- 사용자 추가 지시에 따라 노치 마지막 칸은 종목 수와 관계없이 `+` 추가 버튼이다. 누르면 `종목` 탭의 검색 입력을 바로 연다. 물리 셀 예산 `C`는 최대 6이며 `N+1`과 작업 영역에 맞춰 줄인다.
- 예: `N=10, C=6`이면 종목 4개, `⋯ / +6` 목록, `+` 추가 순서다. 목록 셀은 모든 종목의 상세 열기를 제공한다. `C=1`이면 추가 셀에서 검색과 전체 종목 관리에 접근한다.
- 종목 0개일 때도 같은 `+` 버튼을 사용한다.
- 초과 종목도 같은 scheduler의 관심목록이다. 노치에 안 보인다는 이유로 시세 갱신 대상에서 제외하지 않는다.

## 4. 상세 카드에 표시할 정보

선택한 **한 종목**을 원본과 동일한 260 DIP 폭 안에 2단으로 표시한다. 상단은 아이콘·종목명과 가격/등락률 두 줄, 하단은 전체 폭 스파크라인이다.

사용자 후속 지시에 따라 별도 헤더·메타데이터·중첩 카드를 제거한다. 제거는 노치 종목 아이콘 우상단의 작은 `×`로 이동했다. hover/focus 때 표시하며 제거 요청은 상세 열기·드래그와 분리한다. 실패 시 종목을 유지하고 다시 시도할 수 있다. 노치의 추가 버튼은 글자 없이 `+`만 표시하며 다른 종목과 같은 링·세로 기준선을 사용한다. 링은 중립색이고 등락률 글자만 상승/하락색이다.

```text
  [아이콘] AAPL                  212.35 (+1.24%)
                                 209.75 (-0.36%)
  [              기존 스파크라인               ]
```

위 수치는 설명용이며 실제 시세가 아니다.

- 본문은 `priceRowsForQuote`, `metricsMarkup`/`patchMetricsRow`, `sparklineSvgMarkup`, 기존 포맷과 색 기준을 재사용한다. 기존 column ratios는 JSON에 보존하지만 새 2단 화면에는 조절선을 표시하지 않는다.
- 윗줄은 `Quote.display.primary`, 아랫줄은 `Quote.display.secondary`다. **노치 % = 상세 윗줄 %**. 프런트엔드에서 둘을 따로 계산하지 않는다.
- PRE/LIVE/POST/CLOSED 및 crypto의 기존 세션/가격 의미는 유지한다. 휴장이라는 이유로 아랫줄 등락률을 노치에 올리지 않는다. primary가 없으면 `—`이며 secondary로 대체하지 않는다.
- 차트는 기존 정규장 구간과 기존 regular-session 색 기준을 유지한다. 시간외 최신 등락률과 차트 색은 다를 수 있다. 후속 지시에 따라 구간 설명을 상세 화면에서는 표시하지 않는다.
- `Quote.as_of`는 공급자 시세 시각이다. 현재 수신한 최신 데이터라는 뜻이며, 별도 실시간 거래소/WebSocket 공급자를 새로 도입하지 않는다.
- 아직 없는 시세/차트는 각 영역의 `—`/placeholder로 구별한다. 마지막 시세는 요청 실패로 지우지 않는다. 시세 시각은 데이터에 유지하지만 후속 지시에 따라 상세 화면에 추가하지 않는다.
- 기존 card tint는 선택 종목 카드의 절제된 배경으로 보존한다. 아이콘·숫자 위에 foil이나 굴절 효과가 겹치지 않는다.
- 새 OHLC, 거래량, 뉴스, P&L 또는 이력 저장은 추가하지 않는다.

## 5. 클릭, 호버, 설정, 숨기기

기본 동작은 클릭 고정이다. 호버 상세는 설정 옵션이며 초기값은 off다.

| 상황 | 결과 |
|---|---|
| 종목 A 클릭 | A의 상세 카드를 안쪽에 열고 고정 |
| 열린 A 다시 클릭 | 상세 닫기 |
| A에서 B 클릭 | 같은 canvas 안에서 B 내용과 강조만 교체 |
| 호버 상세 on | 미고정 상태에서 미리보기; 카드로 이동하는 동안 180ms 이탈 여유 |
| 고정 상세 위에서 다른 셀 hover | 고정 유지; 클릭해야 종목 변경 |
| 우클릭 / Shift+F10 / Context Menu | 설정 열기; 노치에 별도 gear를 상시 배치하지 않음 |
| Esc | 설정 우선 닫기; 그다음 고정 상세 닫기; focus 복원으로 즉시 재개방 금지 |
| 노치 드래그 | 5 DIP 문턱 이후 이동; 취소 시 복원; 놓았을 때 한 번 저장 |
| 상세 내 차트/관리 목록 drag | 노치 이동으로 해석하지 않음 |
| 선택 종목 삭제 | 상세 닫고 남은 셀/목록 셀로 focus 복원 |

`표시 상태`는 세 가지로 구별한다.

- **접힘:** 노치 손잡이가 남아 있다. 시세 polling은 계속한다. Liquid 배경 capture는 정지한다.
- **펼침:** 노치·상세·설정 표시. 기존 시세 갱신 주기와 backoff를 따른다.
- **명시적 숨김:** tray 또는 hotkey로 HWND 숨김. 기존 `AppCore.set_visible_state(false)`를 통해 신규 provider 요청을 멈춘다. 진행 중 요청을 취소하는 새 의미는 부여하지 않는다. 프로세스 종료는 별도다.

`노치 항상 표시`가 off이고 포인터/포커스/드래그/상세/설정이 모두 비활성일 때 800ms 후 10×80 DIP 손잡이로 접는다. Hover는 다른 앱의 focus를 빼앗지 않는다. 기존 `Ctrl+Shift+Space`와 tray Show/Hide/Quit는 보존한다. 상세를 열었다고 Yahoo 요청을 별도로 반복하지 않는다.

## 6. 설정 구성

| 탭 | 내용 |
|---|---|
| 모양 | 다크/글라스/리퀴드, 불투명도, 노치 항상 표시, hover 상세, 모션 효과 |
| 종목 | 전체 관심종목, 검색/추가, 삭제/다중 선택, DnD 순서, tint, 각 종목 상세 열기 |
| 일반 | 1s/10s/60s 세 갱신 옵션, autostart, 버전/업데이트, 하단 로그 복사·종료. 사용자 후속 지시에 따라 숨기기는 tray에만 둔다. |

- TokenUsage의 `서비스` 토글/최소 한 서비스 제한과 고정 5초 갱신 정책은 복사하지 않는다. EconomyWarRoom은 빈 관심목록을 허용한다.
- header와 tab은 고정하고 body만 스크롤한다. 첫 열기부터 세 탭 중 가장 긴 내용으로 공통 높이를 측정한다. 같은 모니터에서 탭 전환으로 패널 크기나 위치가 변하지 않는다.
- 종목 증가/삭제 같은 실제 내용 변경, 모니터/DPI 변경 때 공통 높이를 다시 계산한다. 최대 높이 560 DIP 및 실제 작업 영역으로 제한한다.
- 좁은 작업 영역에서도 footer action과 검색 입력에 접근 가능해야 한다. 저장 중 잠금, 실패 시 이전 선택 복원/오류 표시/재시도, 고정 버튼 크기를 이식한다.
- 한글 chrome을 사용하고 ticker/통화는 원문을 유지한다. 설명 tooltip은 추가하지 않는다.

## 7. 창 기하와 재질

- 원본의 현재 silhouette: 깊이 56 DIP, 양끝 inset 44 DIP, tangent 어깨 곡선. 좌/우는 세로, 상/하는 가로이며 아이콘과 숫자는 회전하지 않는다. 원형 틀 40 DIP. 사용자 후속 지시에 따라 등락률 글자는 11px, 링 아래 간격은 기본 3 DIP로 조정했다.
- 사용자 후속 지시에 따라 상세/설정 폭은 원본과 동일한 **260 DIP**, 안쪽 여백 **16 DIP**로 한다. content scale과 물리 DPI도 원본의 계산을 유지한다. 큰 가격·긴 티커와 설정 탭의 넘침을 검증한다.
- Rust가 physical placement, DPI, negative monitor coordinates, drag, hit region을 소유한다. 프런트엔드는 받은 local rect 안에서 그린다.
- 최대 560 DIP 상세/설정 영역을 미리 수용하는 투명 canvas를 예약한다. 종목 간 상세 높이 전환이나 설정 tab 전환 때 HWND와 노치 원점이 움직이지 않는다. monitor/placement/셀 수 변경은 기하 재계산 사유다.
- 보이지 않는 영역은 타 앱으로 click-through, 노치↔카드 bridge는 포인터 이동 가능해야 한다. 작업표시줄과 겹치는 하단 docking 및 다른 가장자리 fallback은 원본처럼 실제 OS에서 검증한다.
- 기존 content-hug observer, `set_content_min_size` 호출, native Resized clamp, 자유 resize와 화살표 창 이동은 노치 shell에서 중단한다. 동시에 작동하는 두 기하 소유자를 만들지 않는다.
- monitor 논리 짧은 변에 따른 원본 content scale 0.92~1.0 정책을 함께 옮긴다. 실제 Windows DPI scale과 content scale을 별도 값으로 유지한다.

재질은 단계적으로 이식하되 최종 목표는 세 가지 모두다.

1. **다크:** 원본 검정 표면과 오목한 silhouette, 중립 control.
2. **글라스:** 은색 rim/낮은 채도의 반사, pointer에 따른 빛 이동. pointer는 기하/hit area를 바꾸지 않는다.
3. **리퀴드:** 실제 바탕화면의 보이는 표면 rect만 메모리로 capture하고 배경만 굴절시킨다. foreground는 선명하게 유지한다.

Liquid는 native GDI/capture affinity와 frame lifecycle까지 이식한다. 한 번에 하나의 capture batch, stale reply 폐기, 테마 전환/숨김/접힘/종료/reload/lease 만료 시 capture 중단 및 affinity 복원이 필요하다. 실패 시 tint fallback과 진단을 제공한다. Dark/Glass는 capture하지 않는다.

원본의 목표는 정지 30fps / drag 60fps / reduced-motion 4fps다. 이는 성능 보장값이 아니다. 260 DIP 카드와 두 위젯 동시 실행에서 CPU/GPU/IPC와 실제 frame cadence를 구별해 측정한다. 원본 방식대로 capture 제외 affinity를 사용하면 Liquid 상태 위젯은 OS screenshot/화면 공유에서 빠질 수 있다. 사용자 후속 지시에 따라 설정 하단의 해당 안내 문구는 표시하지 않는다.

## 8. 데이터·호환성과 범위

- `Quote.display`, regular-session sparkline, Yahoo parsing, queue/backoff, 시세 갱신 preset은 변경하지 않는다. 노치는 같은 snapshot을 보는 새 표시층이다.
- AppSettings에 `theme`, `notch` placement, `hover_detail`, `always_show_notch`, `show_animation`을 추가하고 serde default를 둔다. 기본값: dark / right-centre / false / false / true.
- 기존 watchlist ID/순서/tint, opacity, column ratios, refresh JSON 이름 `quote_refresh_secs`, autostart, hotkey를 보존한다. 기존 window geometry도 지우거나 다른 의미로 덮어쓰지 않는다.
- 새 placement가 없는 기존 데이터는 저장된 window가 속한 모니터를 우선 선택하고 오른쪽 중앙에 배치한다. 모니터를 찾지 못하면 현재 primary로 fallback한다.
- 새 setter는 clone → 검증 → 저장 성공 → 메모리 commit 순서로 원자적으로 반영한다. 실패한 drag/theme 변경은 UI와 native 모두 복원한다.
- `commands.rs`는 어댑터로 유지한다. 순수 placement/reveal 규칙은 domain, 설정 use case는 AppCore, OS API는 infrastructure에 둔다.
- 두 저장소를 직접 import하거나 공용 패키지로 재구성하지 않는다. 검토한 파일/알고리즘만 이식하고 원본의 attribution을 함께 유지한다.
- 새 패키지는 기본적으로 필요 없다. 기존 `windows` crate의 Win32 feature 확대만 계획한다. 사업 로직 coverage의 기존 ≥85% 범위는 유지하고 원본의 넓은 exclusion을 복사하지 않는다.
- 현재 문서의 `dark-only`, 자유 크기 조절/헤더 조작 계약은 구현 단계에서 이 설계로 명시적으로 갱신한다. 지금 검토 초안 작성만으로 현행 제품 규칙이 변경된 것으로 취급하지 않는다.
- 포트폴리오/P&L/브로커/SQLite 이력, 원본 quota/auth/activity 수집은 범위 밖이다.

## 9. 검증과 현재 검토의 한계

이번 검토에서는 실제 소스와 문서, 원본 보관 노치 이미지, 현재 UI 컴포넌트를 Edge에서 synthetic 시세와 native API mock으로 렌더링해 확인했다. 이 렌더링은 native focus/click-through/배경 굴절의 증명이 아니다.

- 기존 baseline: `npm test` **135 unit + 4 integration + 7 risk 통과**.
- 기존 baseline: `npm run build` 통과.
- 현재 컴포넌트 browser 확인: page error 0. 캡처와 검토 스크립트는 git 제외된 `tmp/ui-review-*`, `tmp/ui-adoption-review.py`에 있다.
- 현재 자동 검증: 188 tests, build, business coverage 96.31%. 브라우저 및 네이티브 검증의 버전별 범위와 실제 혼합 DPI·장시간 GPU/전력 미검증 조건은 `docs/testing.md`에 기록했다.

단계별 작업과 통과 조건: [구현 계획](../plans/2026-10-17-tokenusage-ui-adoption.md).
