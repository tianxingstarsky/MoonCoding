# =============================================================================
# CMakeLists.txt — Build system for opencode-port Qt6 reference demo
#
# Standalone build:
#   mkdir build && cd build
#   cmake .. -G Ninja  (or -G "Visual Studio 17 2022" on Windows)
#   cmake --build .
#
# Part of MoonCoding project:
#   Include this directory via add_subdirectory() from the parent
#   CMakeLists.txt in vibe-ui/.
#
# Requires: Qt6 (Widgets, Core), C++17
# =============================================================================

cmake_minimum_required(VERSION 3.20)

project(OpenCodePortDemo
    VERSION 2.0.0
    DESCRIPTION "OpenCode desktop v2 UI design reference — ported to Qt6 widgets"
    LANGUAGES CXX
)

# ---------------------------------------------------------------------------
# C++ standard
# ---------------------------------------------------------------------------
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# ---------------------------------------------------------------------------
# Find Qt6
# ---------------------------------------------------------------------------
find_package(Qt6 REQUIRED COMPONENTS Widgets Core)

qt_standard_project_setup()

# ---------------------------------------------------------------------------
# Source files — All components of the opencode desktop v2 reference
# ---------------------------------------------------------------------------
set(SOURCES
    # Core chat/message components (existing, reused)
    opencode_chat_widget.cpp
    opencode_tool_block.cpp
    opencode_input_widget.cpp

    # New v2 components
    opencode_tab_bar.cpp
    opencode_home_widget.cpp
    opencode_session_view.cpp
    opencode_composer.cpp
    opencode_file_tree.cpp
    opencode_diff_view.cpp
    opencode_dialog_overlay.cpp
    opencode_settings_dialog.cpp

    # Demo application entry point
    opencode_demo_main.cpp
)

set(HEADERS
    # Core headers
    opencode_chat_widget.h
    opencode_tool_block.h
    opencode_input_widget.h

    # Anti-aliasing utility (header-only)
    opencode_antialias.h

    # New v2 headers
    opencode_tab_bar.h
    opencode_home_widget.h
    opencode_session_view.h
    opencode_composer.h
    opencode_file_tree.h
    opencode_diff_view.h
    opencode_dialog_overlay.h
    opencode_settings_dialog.h
)

# ---------------------------------------------------------------------------
# Executable target
# ---------------------------------------------------------------------------
qt_add_executable(OpenCodePortDemo
    ${SOURCES}
    ${HEADERS}
)

# Enable AUTOMOC for Q_OBJECT macros.
set_target_properties(OpenCodePortDemo PROPERTIES
    AUTOMOC ON
    WIN32_EXECUTABLE ON   # No console window on Windows (change to OFF for debugging)
)

target_link_libraries(OpenCodePortDemo PRIVATE
    Qt6::Widgets
    Qt6::Core
)

# ---------------------------------------------------------------------------
# Copy QSS stylesheet alongside the binary for runtime loading.
# ---------------------------------------------------------------------------
add_custom_command(TARGET OpenCodePortDemo POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${CMAKE_CURRENT_SOURCE_DIR}/opencode_styles.qss"
        "$<TARGET_FILE_DIR:OpenCodePortDemo>/opencode_styles.qss"
    COMMENT "Copying opencode_styles.qss to build directory"
)

# ---------------------------------------------------------------------------
# Install (optional — for system-wide deployment)
# ---------------------------------------------------------------------------
install(TARGETS OpenCodePortDemo
    RUNTIME DESTINATION bin
)

install(FILES opencode_styles.qss
    DESTINATION share/opencode-port-demo
)

# ---------------------------------------------------------------------------
# Alternative: Build as a static library for inclusion in MoonCoding
#
# Uncomment the following to build as a library instead of executable:
#
# qt_add_library(OpenCodePortLib STATIC
#     opencode_chat_widget.cpp
#     opencode_tool_block.cpp
#     opencode_input_widget.cpp
#     opencode_tab_bar.cpp
#     opencode_home_widget.cpp
#     opencode_session_view.cpp
#     opencode_composer.cpp
#     opencode_file_tree.cpp
#     opencode_diff_view.cpp
#     opencode_dialog_overlay.cpp
#     opencode_settings_dialog.cpp
# )
# target_include_directories(OpenCodePortLib PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
# target_link_libraries(OpenCodePortLib PUBLIC Qt6::Widgets Qt6::Core)
# set_target_properties(OpenCodePortLib PROPERTIES AUTOMOC ON)
# ---------------------------------------------------------------------------
