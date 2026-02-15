# 🏢 Gascii 모듈별 주요 업무 및 담당 역할

## 📑 목차
1. [최상위 계층 (CLI & UI)](#1-최상위-계층-cli--ui)
2. [비즈니스 로직 계층](#2-비즈니스-로직-계층)
3. [핵심 처리 계층](#3-핵심-처리-계층)
4. [시스템 인터페이스 계층](#4-시스템-인터페이스-계층)
5. [유틸리티 & 지원 계층](#5-유틸리티--지원-계층)
6. [역할별 책임 정리](#6-역할별-책임-정리)

---

## 1. 최상위 계층 (CLI & UI)

### 1.1 **src/main.rs**
**파일 크기**: ~300 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 프로그램 진입점, CLI 파싱, 명령 분배 |
| **책임** | 전체 프로그램 생명주기 관리 |
| **주요 기능** | ✓ 로거 초기화<br>✓ 터미널 상태 복구<br>✓ CLI 명령 파싱 (clap)<br>✓ 명령별 핸들러 호출 |
| **의존성** | logger, ui, core |
| **다운스트림** | 모든 모듈에 의존 |

**주요 코드 흐름**:
```rust
fn main() {
    // 1. 로거 초기화
    crate::utils::logger::init("error.log");
    
    // 2. CLI 파싱
    let cli = Cli::parse();
    
    // 3. 명령 분배
    match &cli.command {
        Commands::PlayLive { ... } => run_game(...),
        Commands::Menu { ... } => run_menu(...),
        Commands::Extract { ... } => extract(...),
    }
}
```

---

### 1.2 **src/ui/interactive.rs**
**파일 크기**: ~250 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 상호작용형 비디오 재생 조정 |
| **책임** | 재생 흐름 조율 |
| **주요 기능** | ✓ 터미널 크기 감지<br>✓ 해상도 자동 계산<br>✓ ANSI 모드 선택<br>✓ 메인 렌더 루프 |
| **핵심 함수** | `run_game()` - 게임 모드<br>`run_ansi_mode()` - ANSI 렌더링 |
| **의존성** | DisplayManager, VideoDecoder, FrameProcessor |
| **담당** | 사용자-시스템 인터페이스 |

**책임**:
- 터미널 크기를 감지하고 비디오 해상도에 맞춤
- 16:9 aspect ratio 계산
- 디코더, 렌더러, 오디오 관리자 생성 및 조율
- 메인 이벤트 루프 구현
- 키보드 입력 처리 (Ctrl+C, q)
- 프레임 타이밍 결정

---

### 1.3 **src/ui/menu.rs**
**파일 크기**: ~200 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 대화형 메뉴 시스템 |
| **책임** | 사용자 선택 입력 |
| **주요 기능** | ✓ 파일 선택 (Select)<br>✓ 설정 입력 (Input)<br>✓ 옵션 선택 (Multiple)<br>✓ 설정 파일 업데이트 |
| **사용 라이브러리** | dialoguer (TUI) |
| **담당** | 대화형 구성 |

**선택 항목**:
1. 폰트 크기 입력 → Gascii.config 저장
2. 비디오 파일 선택 (assets/video/)
3. 오디오 파일 선택 (assets/audio/, 선택사항)
4. 렌더링 모드 (RGB vs ASCII)
5. 화면 모드 (Full vs 16:9)

**출력**:
```
__BAD_APPLE_CONFIG__VIDEO_PATH=/path/to/video.mp4
__BAD_APPLE_CONFIG__AUDIO_PATH=/path/to/audio.wav
__BAD_APPLE_CONFIG__RENDER_MODE=rgb
__BAD_APPLE_CONFIG__FILL_SCREEN=false
__BAD_APPLE_CONFIG__GHOSTTY_ARGS=--window-width=240 --window-height=68
```

---

## 2. 비즈니스 로직 계층

### 2.1 **src/core/player.rs**
**파일 크기**: ~260 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 실시간 재생 엔진 |
| **책임** | 재생 로직 조율 |
| **주요 기능** | ✓ Producer-Consumer 패턴<br>✓ 프레임 버퍼링<br>✓ 타이밍 제어<br>✓ 프레임 드롭 |
| **핵심 함수** | `play_realtime()` - 메인 재생 함수 |
| **의존성** | VideoDecoder, DisplayManager, FrameProcessor, AudioManager |

**핵심 책임**:
```
┌─────────────────────────────────────┐
│ 1. 디코더 스레드 생성               │
│ 2. 버퍼 초기화 (120프레임)          │
│ 3. 메인 루프:                       │
│    ├─ 프레임 수신 (try_recv)       │
│    ├─ 타이밍 비교                   │
│    ├─ 프레임 처리 (Rayon)          │
│    ├─ 렌더링 (Diff)                │
│    └─ 동기화 (Sleep/VSync)         │
│ 4. 종료 처리 (프레임 드롭 통계)     │
└─────────────────────────────────────┘
```

---

### 2.2 **src/core/audio_manager.rs**
**파일 크기**: ~40 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 오디오 재생 관리 |
| **책임** | 오디오 스트림 제어 |
| **주요 기능** | ✓ OutputStream 초기화<br>✓ Sink 생성<br>✓ 오디오 디코딩 (Decoder)<br>✓ 재생 제어 |
| **라이브러리** | rodio (오디오 렌더러) |
| **담당** | 오디오 출력 |

**책임**:
- 시스템 기본 오디오 장치 확인
- 오디오 파일 디코딩 (WAV, MP3, FLAC 등)
- 오디오 싱크 제어 (play, stop)
- 오류 처리 (장치 없음, 형식 미지원 등)

---

### 2.3 **src/core/frame_manager.rs**
**파일 크기**: ~220 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 프레임 메모리 관리 |
| **책임** | 프레임 캐싱 및 LRU 정책 |
| **주요 기능** | ✓ 프레임 로딩 (LZ4 압축)<br>✓ 메모리 캐싱<br>✓ LRU 제거<br>✓ 메모리 최적화 |
| **데이터 구조** | Arc<Vec<u8>> (패킹)<br>LRU 추적 |
| **담당** | 메모리 효율성 |

**주요 함수**:
```rust
pub fn load_frames(&mut self, dir: &str) -> Result<usize>
  // .bin 파일 읽기 → LZ4 압축 해제 → 메모리 로드

pub fn get_frame(&self, index: usize) -> Option<Arc<Vec<u8>>>
  // 캐시 조회 → LRU 업데이트 → 반환
```

---

### 2.4 **src/core/frame_buffer.rs**
**파일 크기**: ~50 LOC

| 항목 | 설명 |
|------|------|
| **역할** | Lock-free 링 버퍼 |
| **책임** | 프레임 큐 관리 |
| **주요 기능** | ✓ ArrayQueue (crossbeam)<br>✓ Non-blocking push/pop<br>✓ 버퍼 상태 추적 |
| **용량** | 120 프레임 (약 2초 @ 60fps) |
| **담당** | 스레드 간 프레임 전달 |

**API**:
```rust
pub fn push(&self, frame: Vec<u8>) -> bool       // 논블로킹
pub fn pop(&self) -> Option<Vec<u8>>            // 논블로킹
pub fn fill_level(&self) -> f32                 // 0.0..1.0
```

---

## 3. 핵심 처리 계층

### 3.1 **src/decoder/video.rs**
**파일 크기**: ~400 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 비디오 디코딩 엔진 |
| **책임** | 프레임 추출 및 처리 |
| **주요 기능** | ✓ OpenCV 래퍼<br>✓ GPU/CPU 디코딩<br>✓ SIMD 리사이즈<br>✓ Letterbox/Crop |
| **라이브러리** | OpenCV (videoio, imgproc)<br>fast_image_resize (SIMD) |
| **담당** | 비디오 I/O |

**주요 함수**:
```rust
pub fn new(path, width, height, fill_mode) -> Result<Self>
  // OpenCV VideoCapture 초기화

pub fn spawn_decoding_thread(self, sender) -> JoinHandle
  // 디코더 스레드 생성 (무한 루프)

pub fn read_frame_into(&mut self, buffer) -> Result<bool>
  // 단일 프레임 처리 (8단계):
  // 1. OpenCV read()
  // 2. BGR → RGB
  // 3. SIMD resize
  // 4. Letterbox/Crop
  // 5. 버퍼 반환
```

**성능**:
- OpenCV decode: 3.2ms
- SIMD resize: 2.1ms (6배 빠름)
- Letterbox: 1.0ms
- **총**: 6.3ms per frame

---

### 3.2 **src/decoder/frame_data.rs**
**파일 크기**: ~20 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 프레임 데이터 구조 |
| **책임** | 타입 정의 |
| **구조체** | `FrameData` |
| **필드** | buffer (Vec<u8>)<br>width (u32)<br>height (u32)<br>timestamp (Duration) |

```rust
pub struct FrameData {
    pub buffer: Vec<u8>,        // RGB 픽셀 데이터
    pub width: u32,             // 픽셀 너비
    pub height: u32,            // 픽셀 높이
    pub timestamp: Duration,    // 프레임 타이밍
}
```

---

### 3.3 **src/renderer/processor.rs**
**파일 크기**: ~100 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 프레임 처리 (Half-block) |
| **책임** | 픽셀 → 셀 변환 |
| **주요 기능** | ✓ Half-block 렌더링<br>✓ Rayon 병렬화<br>✓ 색상 추출 |
| **라이브러리** | Rayon (데이터 병렬) |
| **담당** | 렌더링 준비 |

**핵심 로직**:
```rust
pub fn process_frame_into(&self, pixel_data, cells) {
    cells.par_chunks_mut(2000)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            // 각 셀 = Half-block (2 수직 픽셀)
            // FG = 상단 픽셀 색상
            // BG = 하단 픽셀 색상
        });
}
```

**성능**:
- 순차: 1,500us
- 병렬 (4코어): 400us
- **스피드업**: 3.75배

---

### 3.4 **src/renderer/cell.rs**
**파일 크기**: ~30 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 셀 데이터 구조 |
| **책임** | 타입 정의 |
| **구조체** | `CellData` |
| **필드** | char (문자)<br>fg (RGB)<br>bg (RGB) |

```rust
pub struct CellData {
    pub char: char,           // '▀' (Half-block)
    pub fg: (u8, u8, u8),     // Foreground RGB
    pub bg: (u8, u8, u8),     // Background RGB
}
```

---

### 3.5 **src/renderer/display.rs**
**파일 크기**: ~350 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 터미널 출력 관리 |
| **책임** | ANSI 렌더링 |
| **주요 기능** | ✓ Diff-based 렌더링<br>✓ VSync 관리<br>✓ 색상 캐싱<br>✓ Cursor 최적화 |
| **라이브러리** | crossterm (터미널 제어) |
| **담당** | 터미널 I/O |

**핵심 함수**:
```rust
pub fn render_diff(&mut self, cells, width) -> Result<()> {
    // 1. VSync 시작
    // 2. Diff 렌더링 (변경된 셀만)
    // 3. Cursor 이동 (필요시)
    // 4. 색상 업데이트 (변경시)
    // 5. 문자 출력
    // 6. VSync 종료
    // 7. 버퍼 플러시
}
```

**최적화**:
- Diff-based: 모든 셀 vs 변경된 셀 (5배 감소)
- Cursor 배치: 불필요한 이동 제거 (172배 감소)
- BufWriter: 시스템 호출 감소 (170배)
- 색상 캐싱: 중복 색상 설정 제거

---

### 3.6 **src/renderer/kitty.rs**
**파일 크기**: ~100 LOC

| 항목 | 설명 |
|------|------|
| **역할** | Kitty Graphics Protocol |
| **책임** | 픽셀 정확 렌더링 (미완성) |
| **주요 기능** | ✓ PNG 인코딩<br>✓ Base64 변환<br>✓ Kitty 시퀀스<br>✓ 그래픽 전송 |
| **상태** | ⚠️ 구현 중 |
| **담당** | 대체 렌더러 |

**향후 개선**:
- Sixel 프로토콜도 지원
- 픽셀 정확도 (Half-block 대신)

---

## 4. 시스템 인터페이스 계층

### 4.1 **src/sync/clock.rs**
**파일 크기**: ~70 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 마스터 클록 |
| **책임** | 시간 관리 |
| **주요 기능** | ✓ 경과 시간 추적<br>✓ 일시정지/재개<br>✓ 리셋 |
| **사용** | (현재 미사용, 향후 확장) |

```rust
pub fn elapsed(&self) -> Duration
pub fn pause(&mut self)
pub fn resume(&mut self)
pub fn reset(&mut self)
```

---

### 4.2 **src/sync/vsync.rs**
**파일 크기**: ~110 LOC

| 항목 | 설명 |
|------|------|
| **역할** | VSync 관리 |
| **책임** | 프레임 페이싱 |
| **주요 기능** | ✓ 다음 프레임 대기<br>✓ 프레임 드롭 판정<br>✓ 통계 추적 |
| **사용** | (현재 미사용, 향후 확장) |

```rust
pub fn wait_for_next_frame(&mut self)
pub fn should_drop_frame(&self, clock) -> bool
pub fn stats(&self) -> VSyncStats
```

---

### 4.3 **src/analyzer/mod.rs**
**파일 크기**: ~100 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 콘텐츠 분석 |
| **책임** | 렌더러 선택 |
| **주요 기능** | ✓ 프레임 차이 계산<br>✓ 2D vs 3D 분류<br>✓ 최적 렌더러 추천 |
| **상태** | ⚠️ 구현 중 |

```rust
pub fn analyze_video(&self, path) -> Result<ContentType>
  // 첫 100프레임 분석
  // 픽셀 변화율 계산
  // 2D (<30%) vs 3D (>30%)
```

---

## 5. 유틸리티 & 지원 계층

### 5.1 **src/utils/logger.rs**
**파일 크기**: ~100 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 로깅 시스템 |
| **책임** | 에러 추적 |
| **주요 기능** | ✓ 파일 기반 로깅<br>✓ Panic hook 등록<br>✓ 타임스탬프<br>✓ 레벨별 로그 |
| **출력** | error.log, debug.log |

```rust
pub fn init(log_file: &str)        // 초기화 + Panic hook
pub fn info(msg: &str)
pub fn error(msg: &str)
pub fn debug(msg: &str)
```

**Panic hook**:
- Backtrace 기록
- Location 정보
- error.log에 저장
- 터미널 정상화 시도

---

### 5.2 **src/utils/platform.rs**
**파일 크기**: ~220 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 플랫폼 정보 감지 |
| **책임** | 환경 분석 |
| **주요 기능** | ✓ 터미널 크기 감지<br>✓ 화면 해상도 감지<br>✓ 터미널 기능 확인<br>✓ CPU/메모리 정보 |
| **라이브러리** | crossterm, num_cpus, sysctl |

```rust
pub fn detect() -> Result<Self>
  // PlatformInfo {
  //   os_name, os_version, arch,
  //   terminal, shell,
  //   terminal_width, terminal_height,
  //   supports_ansi, supports_truecolor,
  //   supports_kitty, supports_sixel,
  //   cpu_cores, memory_mb
  // }
```

---

### 5.3 **src/utils/file_utils.rs**
**파일 크기**: ~40 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 파일 I/O 유틸 |
| **책임** | 파일 작업 |
| **주요 기능** | ✓ 파일 나열 (필터)<br>✓ 파일 읽기 |

```rust
pub fn list_files(dir, ext) -> Result<Vec<PathBuf>>
pub fn read_file(path) -> Result<Vec<u8>>
```

---

### 5.4 **src/utils/time_utils.rs**
**파일 크기**: ~40 LOC

| 항목 | 설명 |
|------|------|
| **역할** | 타이밍 유틸 |
| **책임** | 시간 관련 작업 |
| **주요 기능** | ✓ Timer 구조<br>✓ 경과 시간 측정<br>✓ Sleep |

```rust
pub struct Timer { ... }
pub fn elapsed_ms(&self) -> u64
pub fn sleep_ms(ms: u64)
```

---

## 6. 역할별 책임 정리

### 6.1 계층별 책임

```
┌─────────────────────────────────────────┐
│ CLI & UI 계층 (사용자 인터페이스)        │
├─────────────────────────────────────────┤
│ ├─ main.rs        : 프로그램 생명주기    │
│ ├─ interactive.rs : 재생 흐름 조율      │
│ └─ menu.rs        : 대화형 설정         │
└─────────────────────────────────────────┘
           ↓ 구성
┌─────────────────────────────────────────┐
│ 비즈니스 로직 계층 (재생 엔진)           │
├─────────────────────────────────────────┤
│ ├─ player.rs      : 재생 조율           │
│ ├─ audio_manager  : 오디오 제어         │
│ └─ frame_buffer   : 프레임 큐           │
└─────────────────────────────────────────┘
           ↓ 활용
┌─────────────────────────────────────────┐
│ 핵심 처리 계층 (데이터 변환)             │
├─────────────────────────────────────────┤
│ ├─ decoder/video : 디코딩              │
│ ├─ processor     : 렌더링 준비          │
│ └─ display      : 터미널 출력          │
└─────────────────────────────────────────┘
           ↓ 지원
┌─────────────────────────────────────────┐
│ 시스템 & 유틸 계층 (지원 서비스)        │
├─────────────────────────────────────────┤
│ ├─ sync/         : 동기화               │
│ ├─ logger        : 로깅                 │
│ ├─ platform      : 환경 감지            │
│ └─ timer         : 타이밍               │
└─────────────────────────────────────────┘
```

### 6.2 기능별 책임

| 기능 | 담당 모듈 | 책임 |
|------|---------|------|
| **비디오 입력** | decoder/video.rs | 파일 읽기, 디코딩, 리사이징 |
| **프레임 처리** | processor.rs | Half-block 변환, 색상 추출 |
| **터미널 출력** | display.rs | ANSI 시퀀스 생성, 플러시 |
| **오디오 출력** | audio_manager.rs | 오디오 스트림 제어 |
| **타이밍 제어** | player.rs, clock.rs | 프레임 동기화, 드롭 판정 |
| **메모리 관리** | frame_buffer.rs, frame_manager.rs | 버퍼 관리, 캐싱 |
| **UI 제어** | interactive.rs, menu.rs | 사용자 입력, 설정 |
| **에러 처리** | logger.rs | 로깅, Panic 처리 |
| **환경 감지** | platform.rs | 터미널, 성능 정보 |

### 6.3 스레드별 책임

```
┌──────────────────────────────────────────┐
│ 메인 스레드 (Main Thread)                │
├──────────────────────────────────────────┤
│ ├─ 1. 이벤트 루프 실행                   │
│ ├─ 2. 키입력 처리                        │
│ ├─ 3. 프레임 수신                        │
│ ├─ 4. Rayon 병렬화 분배                 │
│ └─ 5. 렌더링 (Diff)                     │
└──────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│ 디코더 스레드 (Decoder Thread)            │
├──────────────────────────────────────────┤
│ ├─ 1. OpenCV read()                     │
│ ├─ 2. 색상 변환                         │
│ ├─ 3. SIMD 리사이징                     │
│ ├─ 4. Letterbox 처리                    │
│ └─ 5. Channel 송신                      │
└──────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│ Rayon 스레드 풀 (Thread Pool)            │
├──────────────────────────────────────────┤
│ ├─ 코어 0: 청크 0 처리                   │
│ ├─ 코어 1: 청크 1 처리                   │
│ ├─ 코어 2: 청크 2 처리                   │
│ ├─ 코어 3: 청크 3 처리                   │
│ └─ ... (work-stealing)                  │
└──────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│ 오디오 프로세스 (Audio Process)           │
├──────────────────────────────────────────┤
│ ├─ ffplay -nodisp                       │
│ ├─ 독립적 오디오 재생                    │
│ └─ 종료 신호 대기                        │
└──────────────────────────────────────────┘
```

### 6.4 데이터 흐름 책임

```
파일 시스템
    │
    ├─ video.mp4
    │  ↓ [decoder/video.rs]
    │  (OpenCV read + SIMD resize)
    │  ↓
    │  FrameData { buffer: Vec<u8>, timestamp }
    │  ↓ [Channel: Ring Buffer]
    │  (Lock-free 전달)
    │  ↓
    │  [processor.rs] (Rayon 병렬)
    │  (Half-block 변환)
    │  ↓
    │  CellData { char, fg, bg }
    │  ↓
    │  [display.rs] (Diff 렌더링)
    │  (ANSI 시퀀스 생성)
    │  ↓
    │  BufWriter (4MB)
    │  ↓
    │  Terminal (터미널 출력)
    │
    └─ audio.wav
       ↓ [audio_manager.rs]
       (rodio Decoder)
       ↓
       [ffplay 프로세스]
       (독립 재생)
```

---

## 7. 모듈 상호작용 매트릭스

| From \ To | decoder | processor | display | player | audio_mgr | utils |
|-----------|---------|-----------|---------|--------|-----------|-------|
| **main** | - | - | - | ✓ | - | ✓ |
| **interactive** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **menu** | - | - | - | - | - | ✓ |
| **decoder** | - | - | - | - | - | ✓ |
| **processor** | ← | - | - | - | - | - |
| **display** | ← | ← | - | - | - | ✓ |
| **player** | ✓ | ✓ | ✓ | - | ✓ | ✓ |
| **audio_mgr** | - | - | - | ← | - | ✓ |

**설명**:
- ✓: 호출 관계
- ←: 데이터 흐름
- -: 의존성 없음

---

## 8. 성능 책임

| 모듈 | 책임 | 성능 목표 | 현재 성능 |
|------|------|---------|---------|
| **decoder** | 디코딩 | 8.3ms (@ 120fps) | 6.3ms ✓ |
| **processor** | 처리 | 10ms | 1.5ms ✓ |
| **display** | 렌더링 | 8ms | 1.5ms ✓ |
| **player** | 동기화 | ±100ms | ±100ms ✓ |
| **total** | 전체 | 16.67ms (@ 60fps) | 9.3ms ✓ |

---

## 9. 보안 책임

| 모듈 | 책임 | 구현 |
|------|------|------|
| **logger** | Panic 처리 | Backtrace 기록, 파일 안전 |
| **decoder** | 파일 검증 | OpenCV 에러 처리 |
| **display** | 터미널 복구 | Drop impl로 정상화 |
| **player** | 리소스 정리 | join(), drop() |
| **main** | 전체 생명주기 | RAII 패턴 |

---

## 10. 확장성 책임

| 모듈 | 확장 포인트 |
|------|-----------|
| **decoder** | Sixel, Kitty Graphics 추가 가능 |
| **processor** | 다른 렌더링 문자 지원 |
| **display** | 색상 양자화, 터미널 특화 최적화 |
| **analyzer** | 다양한 분류 기준 추가 |
| **audio_mgr** | 실시간 오디오 처리, 필터링 |

---

## 요약

Gascii의 각 모듈은 **단일 책임 원칙(SRP)**을 따릅니다:

```
┌────────────────┐
│   CLI & UI     │ ← 사용자 상호작용
├────────────────┤
│  비즈니스 로직  │ ← 재생 흐름 조율
├────────────────┤
│  핵심 처리      │ ← 데이터 변환
├────────────────┤
│  시스템 & 유틸  │ ← 지원 서비스
└────────────────┘
```

**책임 분리**로 인한 이점:
- ✓ 테스트 용이
- ✓ 재사용성 높음
- ✓ 유지보수 쉬움
- ✓ 확장성 우수
- ✓ 에러 격리
