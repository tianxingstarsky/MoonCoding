cmake_minimum_required(VERSION 3.22)
project(MoonCoding VERSION 0.2.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

option(MOONCODING_BUILD_TESTS "Build MoonCoding UI tests" ON)

find_package(Qt6 6.2 REQUIRED COMPONENTS Widgets)
if(MOONCODING_BUILD_TESTS)
    find_package(Qt6 6.2 REQUIRED COMPONENTS Test)
    enable_testing()
endif()

set(MOONCODING_RUST_DIR "${CMAKE_SOURCE_DIR}/vibe-agent")
set(MOONCODING_RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust-target")
set(MOONCODING_VIBE_DIR "${CMAKE_SOURCE_DIR}/vibe")
set(MOONCODING_VIBE_TARGET_DIR "${CMAKE_BINARY_DIR}/vibe-target")
file(GLOB_RECURSE MOONCODING_RUST_SOURCES CONFIGURE_DEPENDS
    "${MOONCODING_RUST_DIR}/src/*.rs"
    "${MOONCODING_RUST_DIR}/prompts/*"
)
file(GLOB_RECURSE MOONCODING_VIBE_SOURCES CONFIGURE_DEPENDS
    "${MOONCODING_VIBE_DIR}/src/*.rs"
)
set(MOONCODING_RUST_DEPENDS
    "${MOONCODING_RUST_DIR}/Cargo.toml"
    ${MOONCODING_RUST_SOURCES}
)
if(EXISTS "${MOONCODING_RUST_DIR}/Cargo.lock")
    list(APPEND MOONCODING_RUST_DEPENDS "${MOONCODING_RUST_DIR}/Cargo.lock")
endif()
set(MOONCODING_VIBE_DEPENDS
    "${MOONCODING_VIBE_DIR}/Cargo.toml"
    ${MOONCODING_VIBE_SOURCES}
)
if(EXISTS "${MOONCODING_VIBE_DIR}/Cargo.lock")
    list(APPEND MOONCODING_VIBE_DEPENDS "${MOONCODING_VIBE_DIR}/Cargo.lock")
endif()

if(WIN32)
    set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/release/vibe_agent.dll")
    set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/release/vibe.exe")
elseif(APPLE)
    set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/release/libvibe_agent.dylib")
    set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/release/vibe")
else()
    set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/release/libvibe_agent.so")
    set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/release/vibe")
endif()

# Optional Rust --target for board cross-builds (e.g. armv7-unknown-linux-gnueabihf).
set(MOONCODING_RUST_TARGET "" CACHE STRING "Optional cargo --target triple for vibe-agent/vibe")
set(MOONCODING_CARGO_TARGET_ARGS "")
if(MOONCODING_RUST_TARGET)
    set(MOONCODING_CARGO_TARGET_ARGS --target "${MOONCODING_RUST_TARGET}")
    if(WIN32)
        set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/vibe_agent.dll")
        set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/vibe.exe")
    elseif(APPLE)
        set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/libvibe_agent.dylib")
        set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/vibe")
    else()
        set(MOONCODING_RUST_LIBRARY "${MOONCODING_RUST_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/libvibe_agent.so")
        set(MOONCODING_VIBE_EXECUTABLE "${MOONCODING_VIBE_TARGET_DIR}/${MOONCODING_RUST_TARGET}/release/vibe")
    endif()
endif()

add_custom_command(
    OUTPUT "${MOONCODING_RUST_LIBRARY}"
    COMMAND cargo build --manifest-path "${MOONCODING_RUST_DIR}/Cargo.toml" --release
            --target-dir "${MOONCODING_RUST_TARGET_DIR}"
            ${MOONCODING_CARGO_TARGET_ARGS}
    DEPENDS
        ${MOONCODING_RUST_DEPENDS}
    WORKING_DIRECTORY "${MOONCODING_RUST_DIR}"
    COMMENT "Building MoonCoding Rust backend"
    VERBATIM
)
add_custom_target(vibe_agent_backend ALL DEPENDS "${MOONCODING_RUST_LIBRARY}")

add_custom_command(
    OUTPUT "${MOONCODING_VIBE_EXECUTABLE}"
    COMMAND cargo build --manifest-path "${MOONCODING_VIBE_DIR}/Cargo.toml" --release
            --target-dir "${MOONCODING_VIBE_TARGET_DIR}"
            ${MOONCODING_CARGO_TARGET_ARGS}
    DEPENDS
        ${MOONCODING_VIBE_DEPENDS}
    WORKING_DIRECTORY "${MOONCODING_VIBE_DIR}"
    COMMENT "Building MoonCoding block protocol engine"
    VERBATIM
)
add_custom_target(vibe_protocol ALL DEPENDS "${MOONCODING_VIBE_EXECUTABLE}")

add_subdirectory(vibe-ui)
