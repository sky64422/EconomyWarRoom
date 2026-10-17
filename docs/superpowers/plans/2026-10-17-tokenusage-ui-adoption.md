# TokenUsage UI Adoption Implementation Plan

> **For agentic workers:** 사용자 승인 후 `superpowers:executing-plans`로 구현했다. 아래 체크리스트는 원래의 작업·검증 범위이며, 현재 실행 결과와 하드웨어 미검증 범위는 `docs/testing.md`를 기준으로 한다.

**Goal:** 종목 아이콘+최신 등락률 노치와 종목별 기존 형태 상세 카드를 제공하고, TokenUsage의 창 동작·설정·세 재질을 이식한다.

**Architecture:** 기존 AppCore/scheduler/Yahoo/Quote.display를 데이터 원본으로 유지한다. 원본의 Rust placement/reveal/native input 모델과 TS shell을 EconomyWarRoom의 동적 watchlist ID에 맞춰 옮긴다. 고정 provider 목록과 quota 로직은 가져오지 않는다.

**Tech Stack:** Tauri 2, Rust, TypeScript, Vite, CSS/SVG/canvas, 기존 windows 0.61. 프레임워크/추가 이미지 라이브러리 없음.

**Spec:** [이식 검토 및 설계안](../specs/2026-10-17-tokenusage-ui-adoption-design.md).

상태: 사용자 승인 후 구현. 커밋/버전 변경/릴리스는 수행하지 않았다. 검증 결과와 하드웨어 한계는 `docs/testing.md`, 단계 기록은 로컬 실행 ledger에 있다.

## 실행 결과 및 적용 판단

7단계 구현 및 후속 조정을 마쳤다. 최신 사용자 지시에 따라 노치의 추가/제거 버튼과 키보드 단축 조작·전역 핫키를 제거했다. 종목 탭은 항상 표시하는 검색창과 간결한 목록이며 종목 수·상단 취소·상세 이동 버튼이 없다. 행 클릭은 상세, 드래그는 정렬, 우측 휴지통은 제거다. TokenUsage의 호환되는 설정·재질·피드백 이식 내역은 [이식 목록](../../ui-references.md#tokenusage-transfer-inventory)에 있다. 상세/설정은 260 DIP 폭과 16 DIP 여백, 상세는 작은 가격 두 줄과 전체 폭 차트의 2단 구성이다. 아래 체크리스트의 이전 Add/Escape/hotkey 요건은 이 후속 지시로 대체됐다. 현재 검증 결과와 네이티브 검증의 버전별 범위는 [검증 기록](../../testing.md)에 있다.

실행 중 정한 사항과 영향:

- Node 내장 runner로 순수 TS 테스트를 실행한다. 런타임 의존성은 늘지 않으며 개발 환경에는 Node 24 이상이 필요하다.
- 커밋·버전·게시를 요청받지 않아 소스 변경과 검증 자료만 남긴다. 설치된 버전에는 아직 반영되지 않는다.
- 슬롯 계산은 실제 `round()`에 맞춰 반 물리 픽셀의 허용치를 사용한다. 경계 화면 크기에서 셀 수가 달라질 수 있어 순수 geometry 회귀로 보호한다.
- 브라우저 mock도 실제 native의 drag-finished 통지를 내보내도록 고쳤다. mock의 입력 누락 때문에 제품 동작을 바꾸지 않았다. native 별도 검증을 유지한다.
- WebView2 native hide는 `document.hidden`을 바꾸지 않아 명시적 visibility 이벤트와 native capture guard를 사용한다. 두 경로의 연결은 숨김/복원 회귀로 보호한다.
- 모니터 migration 테스트의 겹치는 가상 좌표를 수정했다. 실제 데이터 정책은 명시 placement 우선, legacy monitor 복원이다.
- 요청한 원래 작업 폴더로 검증한 파일만 복사하고 격리 작업 사본도 유지한다. 충돌·기준 HEAD·복사 후 해시를 확인하며, 두 위치의 미커밋 변경은 별도로 남는다.

독립 검토자는 중요 문제 2개를 재현했고 모두 실패→성공 회귀로 수정했다. 검토 에이전트의 사용량 제한으로 전체 외부 검토 보고서는 끝나지 않아 남은 최종 검토는 작성자가 수행했다. 보고된 경미 항목은 없다. 혼합 DPI 하드웨어·화면 분리/재연결·지속 GPU/전력 측정·서명 설치 검증은 수행하지 않았다.

## Global Constraints

- 노치 %는 `Quote.display.primary.change`. 상세 윗줄과 동일하며 secondary/regular 전용 %로 대체하지 않는다.
- 상세 본문은 기존 symbol · sparkline · metrics 및 primary/secondary 두 줄을 보존한다.
- 기본값은 dark / right-centre / hover false / always-show false / animation true.
- 기본 깊이 56 DIP, 일반 셀 세로 72 / 가로 96 DIP, ends 44 DIP, 링 40 DIP. 사용자 후속 지시에 따라 퍼센트 글자는 11px, 링 아래 간격은 기본 3 DIP다. 상세/설정 폭 260 DIP, 최대 높이 560 DIP.
- HWND/노치 원점은 상세 전환으로 이동하지 않는다. content-hug와 새 placement가 동시에 작동하면 안 된다.
- 명시적 Hide는 신규 quote polling 중단, Fold는 quote polling 유지. Hide와 Quit는 별도다.
- 기존 관심종목/순서/tint/column ratios/opacity/refresh/autostart/hotkey 및 ≥85% 사업 로직 coverage를 유지한다.
- 외부 앱 쓰기/실사용 설정 덮어쓰기 없이 isolated preview로 먼저 검증한다.

## Review Focus

1. PRE/POST에서 primary와 regular/secondary가 다른 경우: 노치가 primary와 정확히 같아야 한다. Task 1/3.
2. 관심종목 0개/6개 초과/선택 종목 삭제: 입력 focus와 모든 종목 접근이 남아야 한다. Task 1/3/4.
3. 짧은 상세 직후 첫 설정 열기와 종목 탭 증가: 첫 프레임부터 공통 높이, body-only scroll. Task 4.
4. 음수 모니터 좌표/혼합 DPI/화면 가장자리: 노치 고정, 투명 영역 click-through, 실패한 drag 복원. Task 2/7.
5. Liquid 응답 도중 테마 변경/Hide/Fold/reload: 오래된 이미지가 재적용되지 않고 capture/affinity가 정리돼야 한다. Task 6/7.

## 파일과 책임

아래 `Create` 경로는 새 책임 때문에 필요한 후보다. 구현 직전 동등한 심볼/파일이 이미 생겼으면 기존 것을 사용한다.

| 파일 | 작업/책임 |
|---|---|
| `src-tauri/src/domain/types.rs`, `constants.rs` | Modify: theme/placement 설정 및 정책 상수 |
| `src-tauri/src/domain/notch.rs`, `domain/reveal.rs` | Create: 동적 셀 geometry, hit/reveal 순수 규칙 |
| `src-tauri/src/application/service.rs` | Modify: 새 설정 원자 저장, 기존 시세 계약 유지 |
| `src-tauri/src/infrastructure/notch_window.rs` | Create: 네이티브 위치/입력/drag adapter |
| `src-tauri/src/infrastructure/window_ctl.rs`, `lib.rs`, `state.rs`, `commands.rs` | Modify: 기존 hug clamp 해제와 새 shell wiring |
| `src/ui/app.ts`, `types.ts` | Modify: shell 조립, 기존 quote 이벤트 구독 |
| `src/ui/notch.ts`, `notch-state.ts`, `notch-drag.ts`, `notch-surface.ts` | Create: 종목 셀, 상태 전이, drag와 native 응답 조정 |
| `src/ui/stock-detail.ts` | Create: 단일 종목의 기존 시세 행/차트 조립 |
| `src/ui/watchlist/index.ts`, `metrics.ts`, `format.ts`, `dnd.ts`, `search.ts`, `columns.ts`, `tint.ts` | Reuse/필요한 부분만 Modify: 기존 관리 동작 및 시세 표현 |
| `src/ui/settings-panel.ts` | Modify: 모양/종목/일반, 탭 높이, 기존 action |
| `src/ui/surface-finish.ts`, `liquid-glass.ts`, `liquid-loop.ts` | Create: 재질 및 capture lifecycle |
| `src-tauri/src/infrastructure/liquid_glass.rs` | Create: 제한된 Windows 배경 capture 및 lease |
| `src/styles/tokens.css`, `base.css`, `layout.css`, `header.css`, `watchlist.css`, `settings.css`, `app.css`; `src/styles/notch.css` | Modify/Create: native shape와 별개인 표면/제어 스타일 |
| `src/assets/marks/` 및 notices | Create: 종목 아이콘과 출처 |
| `src-tauri/tests/notch.rs`, 기존 risk/integration 테스트 | Create/Extend: 정책, migration, polling 회귀 |
| `tests/ui/*.test.mjs` | Create: Node 내장 runner로 순수 TS 상태 전이/capture loop 검증 |
| `scripts/notch-smoke.py`, `scripts/settings-smoke.py` | Create: 구현 경로를 검증하는 browser/native smoke |

새 Rust module은 소속 `mod.rs`에서 등록한다. 새 native adapter는 필요한 경우 기존 capability 설정과 Tauri invoke handler에 연결한다. 원본의 `AppCore`/DTO/commands 전체를 복사하지 않는다.

## Task 1: 표시 계약과 기존 설정 호환

**Files:** `domain/types.rs`, `constants.rs`, `domain/notch.rs`, `application/service.rs`, `infrastructure/store.rs`, `tests/risk_scenarios.rs`.

**Interfaces:** 기존 `get_quotes`/`quotes-updated`의 `Quote.display`를 유지. 새 `AppTheme { Dark, Glass, Liquid }`, `NotchPlacement { edge, monitor_hint, offset }`, `AppCore::set_theme(theme)`, `AppCore::set_notch_placement(placement)`는 기존 setter와 같은 `Result<_, String>` 패턴 사용.

- [ ] 기존 JSON에 새 필드가 없어도 watchlist/column ratios/opacity/refresh가 그대로인 테스트부터 작성하고 실패를 확인한다.
- [ ] `notch_quote_uses_primary_not_secondary`: fixture의 primary.change=1.24, secondary.change=-0.36이면 노치 DTO/선택 결과는 1.24. primary 없음/NaN은 None, 명시적 0은 0. PRE/POST에서 regular_change와 다른 값도 primary 유지.
- [ ] 새로운 파생 시세 계산은 만들지 않는다. 순수 표시 projection이 필요할 때만 기존 display 코드 옆에 추가하고 frontend에서 동일 snapshot을 사용하게 한다.
- [x] 셀 선택 함수는 마지막 칸을 항상 추가 버튼으로 예약한다. `N=0`은 add target, `N=10,C=6`은 앞 4 ID+overflow 6+add, `N=1,C=6`은 한 ID+add, `N=10,C=1`은 add를 통한 전체 관리 접근으로 고정한다. 사용자 후속 지시로 기존 계획을 갱신했다.
- [ ] 새 setter의 저장 실패 테스트: 실패 전후 메모리와 디스크 설정 일치, retry 성공. 원본이 쓰는 clone/save/commit 방법을 기존 서비스 구조에 맞춘다.
- [ ] 새 설정 타입/default와 setter를 구현하고 위 테스트를 통과시킨다.
- [ ] 검증: `npm test`. **성공:** 기존 146개 회귀 + 새 migration/primary/overflow/rollback 테스트 통과. 단계 완료 diff에 관련 파일만 남긴다.

## Task 2: native 노치 배치와 입력 경계

**Files:** 새 `domain/notch.rs`, `reveal.rs`, `infrastructure/notch_window.rs`, 기존 `window_ctl.rs`, `lib.rs`, `state.rs`, `commands.rs`, `src-tauri/Cargo.toml`, `tests/notch.rs`.

**Interfaces:** 원본 `NotchLayout`/monitor/rect 구조 및 native surface 요청 구조를 기반으로 하되, 고정 provider 수를 Task 1의 동적 slot 수로 바꾼다. DTO의 physical DPI `scale`과 논리 `content_scale`은 별도 유지한다.

- [x] geometry 테스트: 4 edges × DPI 1/1.25/1.5/2, 음수 좌표 모니터, 1~6 slots, 56 깊이/260 상세 폭/560 높이 제한. 서로 다른 상세 높이 요청의 window rect와 notch origin은 동일하다.
- [ ] hit 테스트: 노치 실루엣/상세/bridge는 true, 예약한 투명 바깥 영역은 false. 축소된 모니터와 작업표시줄 제약에서 visible fallback을 검증한다.
- [ ] reveal/drag 테스트: 800ms 이전 유지/이후 접힘을 가짜 시각으로 검증, focus/detail/settings/drag 중 접힘 금지, 5 DIP 전 클릭 유지, Escape 취소, release 시 저장 한 번, 저장 실패 원래 placement 복원.
- [ ] 원본 순수 geometry/reveal부터 이식하고 native adapter를 연결한다. `windows`의 기존 dependency에서 필요한 Win32 feature만 추가한다.
- [ ] 기존 content-hug observer 및 새 shell에서의 `set_content_min_size` 호출, native Resized 최소크기 clamp, DWM ROUND를 중단한다. 기존 commands는 호환성을 해치지 않는 범위에서 유지하고 두 제어 경로가 동시에 호출되지 않음을 확인한다.
- [ ] `cargo test --test notch` 후 **Windows `npm run tauri dev`**에서 4 edges, hover focus, 투명영역 click-through를 확인한다. **성공:** 네이티브 원점 이동/흰 fringe/투명 hit 사각형 없음. browser 결과로 이 단계를 대체하지 않는다.

## Task 3: 종목 아이콘 노치와 기존 형태 상세

**Files:** `ui/app.ts`, `types.ts`, 새 `notch*.ts`, `stock-detail.ts`, `assets/marks/`, 기존 watchlist metrics/format/sparkline/columns, `styles/notch.css`, `tests/ui/notch-state.test.mjs`.

**Interfaces:** `StockDetailController`는 기존 WatchlistController 패턴처럼 선택 item과 같은 snapshot의 Quote/Sparkline을 받아 patch한다. `NotchState`는 `{ itemId, pinned, settings }`, 순수 reducer는 hover/pin/leave/escape/settings/items 변경을 처리한다. 저장 DTO에는 임시 선택 상태를 넣지 않는다.

- [ ] Node 24 내장 test runner와 `--experimental-strip-types`로 DOM 없는 reducer 테스트를 작성한다. 테스트는 `tests/ui/`의 `.mjs`에서 `.ts`를 명시적으로 import한다. test 전용 DOM 모듈은 불러오지 않는다. 런타임 버전을 문서화하고 Node 24가 없는 환경은 같은 순수 계약을 Rust 또는 기존 browser smoke로 확인한다. 추가 test dependency는 도입하지 않는다.
- [ ] 테스트: A pin→B hover는 A 유지, B click은 B pin, B 재클릭은 닫힘, 설정 Escape는 설정만 닫음, 선택 ID 삭제는 해제, 없는 ID focus는 열지 않음.
- [ ] 아이콘 출처/사용 조건을 확인하고 registry+fallback을 구현한다. SVG/이미지는 텍스트와 native hit boundary를 바꾸지 않으며 이미지 실패 때도 같은 크기다.
- [ ] 아이콘/링+아래 primary %만 렌더한다. 목록/추가 셀의 예외 표시는 설계대로 구현한다. up/down/zero/missing를 부호와 색으로 확인한다.
- [x] 선택한 한 종목을 후속 지시에 따라 260 DIP의 2단 상세로 조립했다. 중첩 카드·별도 헤더·시간 footer 없이 아이콘/종목명과 작은 가격 두 줄, 전체 폭 차트만 제공한다. 제거는 노치 `×`로 옮겼다. 기존 formatter/sparkline/primary-secondary를 재사용하며 새 시세 재계산은 하지 않는다.
- [ ] browser mock에서 PRE/POST primary와 sparkline 색이 다른 fixture, 빈 차트, 누락 아이콘, 긴 티커, 큰 가격, 작은 폭/낮은 opacity, reduced-motion을 실제로 렌더해 본다.
- [ ] 검증: `node --experimental-strip-types --test tests/ui/*.test.mjs`, `npm run build`, `npm test`, native 종목별 click/변경 smoke. **성공:** 모든 시점에서 노치와 상세 윗줄 % 일치, 아래줄/차트 의미 유지, 카드 교체 시 노치 불변.

## Task 4: 세 탭 설정과 종목 관리

**Files:** `settings-panel.ts`, 기존 `watchlist/index.ts` 및 DnD/search/tint, `styles/settings.css`, `scripts/settings-smoke.py`, 관련 AppCore 설정 setter.

**Interfaces:** 기존 watchlist의 추가/삭제/정렬/다중선택/tint 명령을 그대로 사용한다. 설정 모양에 새 theme/hover/always-show/animation setter를 연결한다. `종목` 행의 상세 열기는 Task 3의 선택 동작을 호출한다.

- [ ] 실패 테스트/재현부터 준비: 짧은 상세 78/108 DIP 직후 첫 settings open과 첫 탭 변경의 높이 비교. 비활성 종목 탭이 가장 길 때도 첫 측정이 그 높이를 포함해야 한다.
- [ ] `모양 / 종목 / 일반`을 구현한다. 기존 symbol 관리 동작을 필요한 범위만 분리/재사용하고 종목 탭에는 가격 카드 대신 관리 행을 사용한다. 유효한 빈 목록도 허용한다.
- [ ] 추가/삭제로 종목 탭 내용이 바뀌면 공통 높이를 재측정한다. 560 DIP/작업 영역 cap, fixed header/tab, body-only scroll로 처리한다.
- [ ] theme는 Task 5/6의 구현이 연결된 시점에만 각 선택지를 활성화한다. 최종 상태는 세 테마 모두 제공한다. 저장 실패 rollback/retry와 연속 클릭 잠금을 이식한다.
- [ ] 기존 refresh preset, autostart, update progress/restart, copy diagnostics, hide/quit를 새 탭에 연결한다. TokenUsage의 fixed 5s 정책이나 provider enable 제약을 가져오지 않는다.
- [ ] 검증: mocked browser 설정 smoke + native first-open/tab/종목CRUD/키보드 focus. **성공:** tab 변경으로 HWND/패널 높이 불변, 수십 종목도 검색/삭제/상세 열기 가능, 실패한 설정이 성공처럼 남지 않음.

## Task 5: 다크와 글라스 재질/피드백

**Files:** 기존 tokens/opacity/styles, 새 `surface-finish.ts`, `styles/notch.css`, 관련 style imports.

**Interfaces:** 원본 `mountSurfaceFinish(surfaces)`의 `setEnabled`/`destroy`를 재사용한다. 주가 색은 기존 EconomyWarRoom 토큰을 쓴다.

- [ ] dark와 glass를 노치/상세/설정에 함께 적용한다. 은색 rim과 pearl reflection은 바깥 재질에만 두고 종목 icon/가격/차트와 구분한다.
- [ ] 버튼/tabs/switch의 hover/press/saving/success/error 피드백을 옮긴다. 상시 회전/활동 강조를 주가에 부여하지 않는다.
- [ ] pointer 반사 이동이 layout rect나 hit 영역을 바꾸지 않는지 확인한다. drag/touch는 반사를 조종하지 않고 reduced-motion은 정지한다.
- [ ] 기존 opacity 변환/floor를 유지한다. 밝은/어두운 desktop 위, 낮은/높은 opacity에서 primary/secondary 수치와 control을 확인한다.
- [ ] 검증: `npm run build`, native dark/glass 비교 및 종목 전환. **성공:** 같은 geometry와 입력 경계, fringe 없음, 가독성 유지, close/destroy 시 animation frame/listener 정리.

## Task 6: 실제 Liquid 배경 굴절

**Files:** 새 `ui/liquid-glass.ts`, `liquid-loop.ts`, `infrastructure/liquid_glass.rs`, 기존 commands/state/lib/Cargo features, `tests/ui/liquid-loop.test.mjs`, native capture validation tests.

**Interfaces:** 원본 `createLiquidLoop`의 serialize/cancel/dispose 계약과 `capture_liquid_backdrop`/`release_liquid_backdrop`를 이식한다. capture 권한은 현재 theme가 Liquid일 때만 유효하다.

- [ ] 먼저 테스트: capture pending 중 dark로 전환 후 liquid로 복귀하면 옛 frame 폐기, Hidden/Folded 시 새 capture 0, batch 최대 1 in-flight, stop에서 release, 종료/에러 뒤 retry 가능.
- [ ] native rect 검증 테스트: 0 크기/정수 overflow/최대 pixel 제한/창 바깥 rect, capture lease 복원, 음수 모니터 위치. 검증 가능한 순수 로직은 native OS exclusion 안에 숨기지 않는다.
- [ ] 원본 Windows capture/affinity lease를 연결하고 보이는 표면만 메모리로 처리한다. quota/auth 참조와 원본 진단 메서드는 EWR AppCore 인터페이스로 바꾼다. 이미지 패키지는 추가하지 않는다.
- [ ] foreground는 굴절하지 않고 기존 tint/가독성 floor를 유지한다. capture 실패 시 fallback/로그를 확인한다. 설정에 screen capture 제외 동작을 안내한다.
- [ ] 외부 패턴 창을 바탕에 두고 실제 굴절/자기 화면 재귀 없음/색 유지/테마 전환 복원/reload를 확인한다. DOM 배경만 바꾼 screenshot을 native 성공 증거로 쓰지 않는다.
- [ ] 260 DIP 카드와 TokenUsage 동시 실행에서 지속 capture interval/CPU/GPU를 측정한다. 설정상의 30/60/4fps와 실제 수치를 구별한다. 기존 짧은 초기 표본을 지속 성능 증거로 사용하지 않는다.
- [ ] 검증: Node loop tests, native rect tests, `npm test`, `npm run build`, native Liquid smoke. **성공:** 상기 수명주기 검증 통과, hidden/folded에서 capture 중단, dark/glass에서 capture 거절, foreground 선명.

## Task 7: 통합 회귀와 문서

**Files:** integration/risk/native smoke, `AGENTS.md`, `README.md`, `docs/ARCHITECTURE.md`, `docs/ui-references.md`, `docs/testing.md`, `docs/windows-dev.md`, coverage script 필요 부분.

- [ ] 접힘은 polling 유지, explicit hide는 신규 provider 호출 0, show는 기존 scheduler cadence/backoff 복귀를 mock provider 요청 수로 확인한다. 이미 진행 중인 요청과 신규 요청을 구별한다.
- [ ] native matrix: 4 edges × DPI 100/125/150/200%, mixed-DPI/음수 좌표, 0/1/6/다수 종목, 첫/끝 셀, pointer bridge, 20회 상세/설정 전환, 키보드, taskbar edge, 두 위젯 동시 실행. 없는 하드웨어 조건은 미검증으로 기록한다.
- [ ] 저장 재시작 후 placement/theme/watchlist/tint/column ratios/refresh/hotkey 유지. 디스크 실패 시 설정/위치 rollback. debug preview가 실사용 autostart/설정을 덮어쓰지 않음 확인.
- [ ] baseline 명령 `npm test`, `npm run build`, 새 Node tests 및 native smoke를 실행한다. coverage는 기존 ≥85% 사업 로직 범위를 지키고 새 domain/application을 포함한다. 원본 coverage exclusion을 복사하지 않는다.
- [ ] 문서의 dark-only/free-resize/header/설정 설명을 최종 동작으로 맞춘다. test count와 native 실측/미검증 조건을 근거와 함께 갱신한다.
- [ ] 최종 diff에서 quota/auth/activity/SQLite와 불필요한 dependency가 들어오지 않았는지 검토한다.
- [ ] **성공:** 요구별 검증 근거 존재, baseline 회귀 없음, native 제한 명시. 커밋/버전/배포는 요청받은 경우에만 `C:/dev/Rules/RELEASE.md`를 파일로 연 뒤 별도 수행한다.

## 구현 순서의 이유

먼저 시세의 의미와 설정 호환을 고정하고, 가장 위험한 native geometry를 검증한다. 다음으로 아이콘/등락률과 단일 종목 카드를 연결한 뒤 관리/설정을 완성한다. 그 위에 글라스와 Liquid를 올려서 재질 실패가 시세나 창 배치 문제로 섞이지 않게 한다. 각 단계는 자체 통과 조건이 있으며, 마지막에 전체 native 경로를 확인한다.
