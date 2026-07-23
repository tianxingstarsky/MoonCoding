# Cross-compiling MoonCoding for Luckfox Lyra (RK3506B armhf) against Buildroot Qt6.
#
# Usage (from WSL, after Qt6 buildroot finishes):
#   cmake -S . -B build-board -DCMAKE_TOOLCHAIN_FILE=cmake/lyra-rk3506-toolchain.cmake
#   cmake --build build-board -j$(nproc)
#
# Override roots:
#   -DLYRA_SDK_ROOT=$HOME/Lyra-sdk
#   -DLYRA_BR_OUTPUT=$HOME/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox

set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR arm)

if(NOT LYRA_SDK_ROOT)
  if(DEFINED ENV{LYRA_SDK_ROOT})
    set(LYRA_SDK_ROOT "$ENV{LYRA_SDK_ROOT}")
  else()
    set(LYRA_SDK_ROOT "$ENV{HOME}/Lyra-sdk")
  endif()
endif()

if(NOT LYRA_BR_OUTPUT)
  if(DEFINED ENV{LYRA_BR_OUTPUT})
    set(LYRA_BR_OUTPUT "$ENV{LYRA_BR_OUTPUT}")
  else()
    set(LYRA_BR_OUTPUT "${LYRA_SDK_ROOT}/buildroot/output/rockchip_rk3506_luckfox")
  endif()
endif()

set(LYRA_HOST "${LYRA_BR_OUTPUT}/host")
set(LYRA_SYSROOT "${LYRA_BR_OUTPUT}/host/arm-buildroot-linux-gnueabihf/sysroot")
set(LYRA_TARGET_DIR "${LYRA_BR_OUTPUT}/target")

set(CMAKE_SYSROOT "${LYRA_SYSROOT}")
set(CMAKE_FIND_ROOT_PATH "${LYRA_SYSROOT}" "${LYRA_HOST}")

set(CMAKE_C_COMPILER "${LYRA_HOST}/bin/arm-buildroot-linux-gnueabihf-gcc")
set(CMAKE_CXX_COMPILER "${LYRA_HOST}/bin/arm-buildroot-linux-gnueabihf-g++")
set(CMAKE_C_COMPILER_TARGET arm-buildroot-linux-gnueabihf)
set(CMAKE_CXX_COMPILER_TARGET arm-buildroot-linux-gnueabihf)

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# Prefer Buildroot-provided Qt6 CMake packages inside the sysroot / host staging.
list(PREPEND CMAKE_PREFIX_PATH
  "${LYRA_SYSROOT}/usr"
  "${LYRA_HOST}/arm-buildroot-linux-gnueabihf/sysroot/usr"
  "${LYRA_HOST}"
)

set(QT_HOST_PATH "${LYRA_HOST}" CACHE PATH "Qt host tools from Buildroot")

# Board builds: skip UI unit tests by default (cross + no on-device Qt Test needed).
set(MOONCODING_BUILD_TESTS OFF CACHE BOOL "Build MoonCoding UI tests" FORCE)
set(MOONCODING_LINUX_USE_SYSTEM_QT ON CACHE BOOL "Use sysroot Qt" FORCE)

# Help cargo cross-link through the same linker.
set(ENV{CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER}
    "${LYRA_HOST}/bin/arm-buildroot-linux-gnueabihf-gcc")
set(ENV{CC_armv7_unknown_linux_gnueabihf}
    "${LYRA_HOST}/bin/arm-buildroot-linux-gnueabihf-gcc")
set(ENV{CXX_armv7_unknown_linux_gnueabihf}
    "${LYRA_HOST}/bin/arm-buildroot-linux-gnueabihf-g++")

message(STATUS "Lyra toolchain: SDK=${LYRA_SDK_ROOT}")
message(STATUS "Lyra toolchain: BR_OUTPUT=${LYRA_BR_OUTPUT}")
message(STATUS "Lyra toolchain: SYSROOT=${LYRA_SYSROOT}")
