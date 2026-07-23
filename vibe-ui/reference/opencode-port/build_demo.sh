#!/bin/bash
# =============================================================================
# build_demo.sh — Build and run the opencode-port Qt6 demo
#
# Prerequisites:
#   MSYS2 UCRT64: pacman -S mingw-w64-ucrt-x86_64-qt6-base mingw-w64-ucrt-x86_64-cmake mingw-w64-ucrt-x86_64-ninja
#   OR
#   Qt6 Official: Install from qt.io, add to CMAKE_PREFIX_PATH
#
# Usage:
#   ./build_demo.sh          # Build and run
#   ./build_demo.sh build    # Build only
#   ./build_demo.sh clean    # Clean build directory
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
SOURCE_DIR="${SCRIPT_DIR}"

ACTION="${1:-all}"

clean() {
    echo "==> Cleaning build directory..."
    rm -rf "${BUILD_DIR}"
    echo "==> Done."
}

configure() {
    echo "==> Configuring CMake..."
    mkdir -p "${BUILD_DIR}"
    cd "${BUILD_DIR}"

    # Try to detect Qt6.
    # On MSYS2 UCRT64, Qt6 is in /mingw64.
    # On official Qt, set CMAKE_PREFIX_PATH to the Qt6 installation directory.
    CMAKE_ARGS="-G Ninja -DCMAKE_BUILD_TYPE=Release"

    if [ -d "/mingw64/lib/cmake/Qt6" ]; then
        echo "    Using MSYS2 Qt6 from /mingw64"
        CMAKE_ARGS="${CMAKE_ARGS} -DCMAKE_PREFIX_PATH=/mingw64"
    elif [ -n "${CMAKE_PREFIX_PATH:-}" ]; then
        echo "    Using CMAKE_PREFIX_PATH=${CMAKE_PREFIX_PATH}"
    else
        echo "    WARNING: Qt6 not auto-detected. Set CMAKE_PREFIX_PATH."
        echo "    Example: export CMAKE_PREFIX_PATH=/c/Qt/6.8.0/mingw_64"
    fi

    cmake "${SOURCE_DIR}" ${CMAKE_ARGS}
    echo "==> Configuration complete."
}

build() {
    configure
    echo "==> Building..."
    cmake --build "${BUILD_DIR}" --config Release
    echo "==> Build complete."
}

run() {
    echo "==> Running OpenCodePortDemo..."
    cd "${BUILD_DIR}"
    ./OpenCodePortDemo.exe
}

case "${ACTION}" in
    clean)
        clean
        ;;
    build)
        build
        ;;
    run)
        run
        ;;
    all)
        build
        run
        ;;
    *)
        echo "Usage: $0 {clean|build|run|all}"
        exit 1
        ;;
esac
