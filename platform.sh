#!/usr/bin/env bash
# 🔧 Bad Apple Platform Detection Script
# 크로스 플랫폼 환경 감지 및 최적화 설정 스크립트
# Author: BadApple C Team
# Date: 2025-08-04

set -euo pipefail


# SCRIPT_DIR이 곧 프로젝트 루트임
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"

# 색상 정의 (이미 선언되어 있으면 skip)
if [[ -z "${RED+x}" ]]; then
    readonly RED='\033[0;31m'
    readonly GREEN='\033[0;32m'
    readonly YELLOW='\033[1;33m'
    readonly BLUE='\033[0;34m'
    readonly PURPLE='\033[0;35m'
    readonly CYAN='\033[0;36m'
    readonly NC='\033[0m'
fi

# 로깅 함수들
log_info() {
    echo -e "${CYAN}[PLATFORM]${NC} $1" >&2
}

log_success() {
    echo -e "${GREEN}[PLATFORM]${NC} $1" >&2
}

log_warning() {
    echo -e "${YELLOW}[PLATFORM]${NC} $1" >&2
}

log_error() {
    echo -e "${RED}[PLATFORM]${NC} $1" >&2
}

# GCC가 없을 때 사용할 기본 플랫폼 정보 설정
setup_basic_platform_info() {
    log_warning "GCC가 없어 기본 플랫폼 정보를 사용합니다."

    # 기본 플랫폼 정보 설정
    export BADAPPLE_OS_NAME="$(uname)"
    export BADAPPLE_OS_VERSION="Unknown"
    export BADAPPLE_ARCH="$(uname -m)"
    export BADAPPLE_TERMINAL="${TERM:-Unknown}"
    export BADAPPLE_SHELL="${SHELL:-bash}"

    # 터미널 크기 감지 (stty 사용)
    if command -v stty >/dev/null 2>&1; then
        local term_size
        term_size=$(stty size 2>/dev/null || echo "24 80")
        export BADAPPLE_TERMINAL_HEIGHT="${term_size%% *}"
        export BADAPPLE_TERMINAL_WIDTH="${term_size##* }"
    else
        export BADAPPLE_TERMINAL_HEIGHT="24"
        export BADAPPLE_TERMINAL_WIDTH="80"
    fi

    # 기본 기능 지원 설정
    export BADAPPLE_SUPPORTS_ANSI="true"
    export BADAPPLE_SUPPORTS_TRUECOLOR="false"
    export BADAPPLE_SUPPORTS_KITTY="false"
    export BADAPPLE_SUPPORTS_SIXEL="false"

    log_success "기본 플랫폼 정보 설정 완료"
}

# 플랫폼 감지 바이너리 경로
PLATFORM_BIN="$PROJECT_DIR/build/bin/platform_detector"
PLATFORM_SRC="$PROJECT_DIR/src/platform.c"
echo "[DEBUG] SCRIPT_DIR=$SCRIPT_DIR, PROJECT_DIR=$PROJECT_DIR" >&2
echo "[DEBUG] PLATFORM_BIN=$PLATFORM_BIN" >&2
echo "[DEBUG] PLATFORM_SRC=$PLATFORM_SRC" >&2
echo "[DEBUG] CACHE_PATH=$HOME/.badapple_platform_cache" >&2

# JSON 파싱을 위한 sed 기반 간단 함수 (에러시 빈 문자열 반환)
parse_json_value() {
    local json="$1" key="$2"
    echo "$json" | sed -nE 's/.*"'"$key"'"[ ]*:[ ]*"([^"]*)".*/\1/p'
}

parse_json_number() {
    local json="$1" key="$2"
    echo "$json" | sed -nE 's/.*"'"$key"'"[ ]*:[ ]*([0-9]+).*/\1/p'
}

parse_json_boolean() {
    local json="$1" key="$2" val
    val=$(echo "$json" | sed -nE 's/.*"'"$key"'"[ ]*:[ ]*(true|false).*/\1/p')
    [[ "$val" == "true" ]] && echo 1 || echo 0
}

# platform.c 컴파일
compile_platform_detector() {
    # GCC 없이 실행하는 옵션 체크
    if [[ "${BADAPPLE_NO_COMPILE:-false}" == "true" ]]; then
        log_warning "컴파일 생략 옵션이 설정되었습니다. 기본 설정을 사용합니다."
        setup_basic_platform_info
        return 0
    fi

    log_info "플랫폼 감지기 컴파일 중..."
    echo "[DEBUG] compile_platform_detector() called at $(date +%s)" >&2
    # 빌드 디렉토리 생성
    mkdir -p "$PROJECT_DIR/build/bin"
    # 컴파일 플래그 설정
    local cflags="-std=c99 -Wall -Wextra -O2"
    local ldflags=""
    # macOS 특별 설정
    if [[ "$(uname)" == "Darwin" ]]; then
        ldflags="$ldflags"
    fi
    # Linux 특별 설정
    if [[ "$(uname)" == "Linux" ]]; then
        ldflags="$ldflags"
    fi
    # 컴파일 실행
    echo "[DEBUG] gcc $cflags $PLATFORM_SRC $ldflags -o $PLATFORM_BIN" >&2
    if gcc $cflags "$PLATFORM_SRC" $ldflags -o "$PLATFORM_BIN"; then
        log_success "플랫폼 감지기 컴파일 완료"
        return 0
    else
        log_error "플랫폼 감지기 컴파일 실패"
        if ! command -v gcc >/dev/null 2>&1; then
            log_error "GCC 컴파일러가 없습니다. 기본 설정을 사용합니다."
            # GCC가 없을 때 기본 플랫폼 정보 설정
            setup_basic_platform_info
            return 0
        fi
        return 1
    fi
}
# 플랫폼 정보 감지 및 환경변수 설정
detect_and_export_platform() {
    local use_cache=true
    local force_detect=false
    local cache_args=""
    
    # 명령행 인수 처리
    while [[ $# -gt 0 ]]; do
        case $1 in
            --no-cache)
                use_cache=false
                shift
                ;;
            --no-compile)
                export BADAPPLE_NO_COMPILE=true
                shift
                ;;
            --force)
                force_detect=true
                shift
                ;;
            --clear-cache)
                if [[ -f "$PLATFORM_BIN" ]]; then
                    "$PLATFORM_BIN" --clear-cache >/dev/null 2>&1
                    log_success "플랫폼 캐시 삭제됨"
                fi
                return 0
                ;;
            *)
                shift
                ;;
        esac
    done
    
    # 플랫폼 감지기 컴파일 (필요시)
    echo "[DEBUG] detect_and_export_platform() - check binary at $(date +%s)" >&2
    if [[ ! -f "$PLATFORM_BIN" ]] || [[ "$PLATFORM_SRC" -nt "$PLATFORM_BIN" ]] || $force_detect; then
        echo "[DEBUG] 컴파일 필요: $PLATFORM_BIN 없음 또는 $PLATFORM_SRC 최신 또는 force_detect=$force_detect" >&2
        if ! compile_platform_detector; then
            log_warning "컴파일 실패, 기본 설정 사용"
            return 1
        fi
    fi
    # 플랫폼 정보 감지 실행
    log_info "플랫폼 환경 감지 중..."
    echo "[DEBUG] 감지 바이너리 실행: $PLATFORM_BIN $cache_args at $(date +%s)" >&2
    local platform_json
    if ! $use_cache; then
        cache_args="--no-cache"
    fi
    if platform_json=$("$PLATFORM_BIN" $cache_args 2>&1); then
        log_success "플랫폼 정보 감지 완료"
        echo "[DEBUG] platform_json: $platform_json" >&2
    else
        log_error "플랫폼 감지 실패, 기본 설정 사용"
        echo "[DEBUG] 감지 바이너리 실행 실패: $PLATFORM_BIN $cache_args" >&2
        return 1
    fi
    
    # JSON에서 값 추출 및 환경변수 설정
    export BADAPPLE_OS_NAME=$(parse_json_value "$platform_json" "os_name")
    export BADAPPLE_OS_VERSION=$(parse_json_value "$platform_json" "os_version")
    export BADAPPLE_ARCH=$(parse_json_value "$platform_json" "arch")
    export BADAPPLE_TERMINAL=$(parse_json_value "$platform_json" "terminal")
    export BADAPPLE_SHELL=$(parse_json_value "$platform_json" "shell")
    export BADAPPLE_TERMINAL_WIDTH=$(parse_json_number "$platform_json" "terminal_width")
    export BADAPPLE_TERMINAL_HEIGHT=$(parse_json_number "$platform_json" "terminal_height")
    export BADAPPLE_SUPPORTS_ANSI=$(parse_json_boolean "$platform_json" "supports_ansi")
    export BADAPPLE_SUPPORTS_TRUECOLOR=$(parse_json_boolean "$platform_json" "supports_truecolor")
    export BADAPPLE_SUPPORTS_KITTY=$(parse_json_boolean "$platform_json" "supports_kitty")
    export BADAPPLE_SUPPORTS_SIXEL=$(parse_json_boolean "$platform_json" "supports_sixel")
    export BADAPPLE_CPU_CORES=$(parse_json_number "$platform_json" "cpu_cores")
    export BADAPPLE_MEMORY_MB=$(parse_json_number "$platform_json" "memory_mb")
    
    # 동적 터미널 크기 override (실제 터미널 크기 반영)
    local term_rows term_cols
    echo "[DEBUG] override terminal via stty/env/tput" >&2
    if stty size >/dev/null 2>&1; then
        read -r term_rows term_cols <<< "$(stty size)"
        echo "[DEBUG] stty size -> rows=$term_rows cols=$term_cols" >&2
    elif [[ -n "$LINES" && -n "$COLUMNS" ]]; then
        term_rows=$LINES; term_cols=$COLUMNS
        echo "[DEBUG] env LINES/COLUMNS -> rows=$term_rows cols=$term_cols" >&2
    elif command -v tput >/dev/null 2>&1; then
        term_cols=$(tput cols); term_rows=$(tput lines)
        echo "[DEBUG] tput cols/lines -> rows=$term_rows cols=$term_cols" >&2
    fi
    if [[ -n "$term_cols" && -n "$term_rows" && "$term_cols" -gt 0 && "$term_rows" -gt 0 ]]; then
        export BADAPPLE_TERMINAL_WIDTH="$term_cols"
        export BADAPPLE_TERMINAL_HEIGHT="$term_rows"
        echo "[DEBUG] overridden BADAPPLE_TERMINAL_WIDTH=$term_cols, HEIGHT=$term_rows" >&2
    fi
    
    # 플랫폼별 최적화 설정
    setup_platform_optimizations
    
    # 환경 정보 출력 (선택적)
    if [[ "${BADAPPLE_VERBOSE:-0}" == "1" ]]; then
        print_platform_summary
    fi
    
    return 0
}

# 플랫폼별 최적화 설정
setup_platform_optimizations() {
    # 기본 설정
    export BADAPPLE_RECOMMENDED_MODE="ASCII"
    export BADAPPLE_RECOMMENDED_FPS="120"  # 120FPS 고정
    export BADAPPLE_RECOMMENDED_WIDTH=""
    export BADAPPLE_RECOMMENDED_HEIGHT=""
    export BADAPPLE_BUILD_PARALLEL=""
    
    # CPU 코어 기반 병렬 빌드 설정
    if [[ "$BADAPPLE_CPU_CORES" -gt 1 ]]; then
        export BADAPPLE_BUILD_PARALLEL="-j$BADAPPLE_CPU_CORES"
    fi
    
    # 터미널 크기 기반 해상도 최적화 (전체에서 5 빼기)
    local term_w=${BADAPPLE_TERMINAL_WIDTH:-240}
    local term_h=${BADAPPLE_TERMINAL_HEIGHT:-85}

    # 안전한 크기로 조정 (여백 5픽셀 확보)
    local opt_w=$((term_w - 5))
    local opt_h=$((term_h - 5))

    # 최소 크기
    (( opt_w < 40 )) && opt_w=40
    (( opt_h < 20 )) && opt_h=20
    # 최대 크기
    (( opt_w > 500 )) && opt_w=500
    (( opt_h > 150 )) && opt_h=150
    
    export BADAPPLE_RECOMMENDED_WIDTH="$opt_w"
    export BADAPPLE_RECOMMENDED_HEIGHT="$opt_h"
    
    # 그래픽 지원 기반 모드 추천
    if [[ "$BADAPPLE_SUPPORTS_KITTY" == "1" ]]; then
        export BADAPPLE_RECOMMENDED_MODE="GRAPHICS"
        log_info "Kitty 그래픽 프로토콜 지원 감지 - GRAPHICS 모드 추천"
    elif [[ "$BADAPPLE_SUPPORTS_TRUECOLOR" == "1" ]]; then
        export BADAPPLE_RECOMMENDED_MODE="RGB"
        log_info "24비트 트루컬러 지원 감지 - RGB 모드 추천"
    elif [[ "$BADAPPLE_SUPPORTS_ANSI" == "1" ]]; then
        export BADAPPLE_RECOMMENDED_MODE="ASCII"
        log_info "ANSI 색상 지원 감지 - ASCII 모드 추천"
    fi
    
    # 성능 기반 FPS 조정 (120FPS 고정)
    export BADAPPLE_RECOMMENDED_FPS="120"
    log_info "고성능 설정 - 120FPS 고정"
    
    # Windows PowerShell 특별 처리
    if [[ "$BADAPPLE_TERMINAL" == "PowerShell" ]]; then
        export BADAPPLE_RECOMMENDED_MODE="ASCII"
        export BADAPPLE_RECOMMENDED_FPS="120"
        log_info "PowerShell 환경 감지 - ASCII 모드 120FPS 설정"
    fi
    
    # macOS Terminal.app 최적화
    if [[ "$BADAPPLE_TERMINAL" == "Terminal.app" ]]; then
        export BADAPPLE_RECOMMENDED_FPS="120"
        log_info "macOS Terminal.app 감지 - 120FPS 고성능 설정"
    fi
    
    # SSH 세션 특별 처리
    if [[ "$BADAPPLE_TERMINAL" == "SSH Session" ]]; then
        export BADAPPLE_RECOMMENDED_MODE="ASCII"
        export BADAPPLE_RECOMMENDED_FPS="60"  # SSH만 60FPS
        # SSH에서는 작은 크기로
        export BADAPPLE_RECOMMENDED_WIDTH=$((opt_w / 2))
        export BADAPPLE_RECOMMENDED_HEIGHT=$((opt_h / 2))
        log_info "SSH 세션 감지 - 네트워크 최적화 60FPS 설정"
    fi
}

# 플랫폼 정보 요약 출력
print_platform_summary() {
    echo -e "\n${PURPLE}🔧 Bad Apple Platform Information${NC}" >&2
    echo -e "${BLUE}════════════════════════════════════${NC}" >&2
    echo -e "${CYAN}OS:${NC} $BADAPPLE_OS_NAME $BADAPPLE_OS_VERSION ($BADAPPLE_ARCH)" >&2
    echo -e "${CYAN}Terminal:${NC} $BADAPPLE_TERMINAL" >&2
    echo -e "${CYAN}Shell:${NC} $BADAPPLE_SHELL" >&2
    echo -e "${CYAN}Terminal Size:${NC} ${BADAPPLE_TERMINAL_WIDTH}x${BADAPPLE_TERMINAL_HEIGHT}" >&2
    echo -e "${CYAN}Graphics Support:${NC} ANSI($BADAPPLE_SUPPORTS_ANSI) TrueColor($BADAPPLE_SUPPORTS_TRUECOLOR) Kitty($BADAPPLE_SUPPORTS_KITTY) Sixel($BADAPPLE_SUPPORTS_SIXEL)" >&2
    echo -e "${CYAN}Hardware:${NC} ${BADAPPLE_CPU_CORES} cores, ${BADAPPLE_MEMORY_MB}MB RAM" >&2
    echo -e "${GREEN}Recommended:${NC} ${BADAPPLE_RECOMMENDED_MODE} mode, ${BADAPPLE_RECOMMENDED_FPS}FPS, ${BADAPPLE_RECOMMENDED_WIDTH}x${BADAPPLE_RECOMMENDED_HEIGHT}" >&2
    echo -e "${BLUE}════════════════════════════════════${NC}" >&2
}

# 폴백 기본 설정 (컴파일 실패시)
setup_fallback_defaults() {
    log_warning "기본 설정 사용"
    
    export BADAPPLE_OS_NAME="$(uname -s)"
    export BADAPPLE_OS_VERSION="$(uname -r)"
    export BADAPPLE_ARCH="$(uname -m)"
    export BADAPPLE_TERMINAL="${TERM_PROGRAM:-${TERM:-unknown}}"
    export BADAPPLE_SHELL="$(basename "${SHELL:-bash}")"
    
    # stty로 터미널 크기 감지
    if command -v stty >/dev/null 2>&1; then
        local size=$(stty size 2>/dev/null || echo "24 80")
        export BADAPPLE_TERMINAL_HEIGHT=$(echo $size | cut -d' ' -f1)
        export BADAPPLE_TERMINAL_WIDTH=$(echo $size | cut -d' ' -f2)
    else
        export BADAPPLE_TERMINAL_WIDTH=80
        export BADAPPLE_TERMINAL_HEIGHT=24
    fi
    
    # 기본값들
    export BADAPPLE_SUPPORTS_ANSI=1
    export BADAPPLE_SUPPORTS_TRUECOLOR=0
    export BADAPPLE_SUPPORTS_KITTY=0
    export BADAPPLE_SUPPORTS_SIXEL=0
    export BADAPPLE_CPU_CORES=1
    export BADAPPLE_MEMORY_MB=0
    
    # 기본 추천 설정
    export BADAPPLE_RECOMMENDED_MODE="ASCII"
    export BADAPPLE_RECOMMENDED_FPS="30"
    export BADAPPLE_RECOMMENDED_WIDTH=$((BADAPPLE_TERMINAL_WIDTH - 4))
    export BADAPPLE_RECOMMENDED_HEIGHT=$((BADAPPLE_TERMINAL_HEIGHT - 4))
    export BADAPPLE_BUILD_PARALLEL=""
}


# 메인 실행 함수
main() {
    # 플랫폼 감지 시도
    if ! detect_and_export_platform "$@"; then
        setup_fallback_defaults
    fi
    # 성공 표시
    log_success "플랫폼 감지 완료"
    # 디버그 출력 (요청시)
    if [[ "${1:-}" == "--verbose" ]] || [[ "${BADAPPLE_VERBOSE:-0}" == "1" ]]; then
        print_platform_summary
    fi
}

# cross-shell source 감지 (bash/zsh/dash 등 호환)
_is_sourced() {
    # bash
    if [ -n "$BASH_VERSION" ]; then
        [[ "${BASH_SOURCE[0]}" != "${0}" ]]
        return
    fi
    # zsh
    if [ -n "$ZSH_VERSION" ]; then
        case $ZSH_EVAL_CONTEXT in *:file) return 0;; esac
        return 1
    fi
    # ksh
    if [ -n "$KSH_VERSION" ] || [ -n "$FCEDIT" ]; then
        [[ "${.sh.file}" != "${0}" ]]
        return
    fi
    # POSIX/dash: $0 == sh when sourced, but not always reliable
    case $0 in
        -sh|sh|dash) return 0;;
    esac
    return 1
}

if _is_sourced; then
    # source로 실행된 경우: 환경변수 export
    main "$@"
else
    # 직접 실행된 경우: 안내만 출력
    echo "[INFO] 이 스크립트는 반드시 'source platform.sh' 또는 '. ./platform.sh'로 실행해야 환경변수가 적용됩니다." >&2
    echo "[INFO] 예시: source ./platform.sh --no-cache" >&2
    exit 0
fi
