set(MOONCODING_UI_SOURCES
    src/main.cpp
    src/mainwindow.cpp
    src/mainwindow.h
    src/rustbridge.cpp
    src/rustbridge.h
    src/languagemanager.cpp
    src/languagemanager.h
    src/opencode_antialias.h
    src/apps/appswidget.cpp
    src/apps/appswidget.h
    src/apps/apprunner.cpp
    src/apps/apprunner.h
    src/apps/appschemarenderer.cpp
    src/apps/appschemarenderer.h
    src/apps/apphostwidget.cpp
    src/apps/apphostwidget.h
    src/chat/chatwidget.cpp
    src/chat/chatwidget.h
    src/input/inputwidget.cpp
    src/input/inputwidget.h
    src/input/softkeyboard.cpp
    src/input/softkeyboard.h
    src/input/boardime.cpp
    src/input/boardime.h
    src/input/touchscroll.h
    src/input/googlepinyinengine.cpp
    src/input/googlepinyinengine.h
    src/settings/wifipanel.cpp
    src/settings/wifipanel.h
    src/settings/boardnetrecover.cpp
    src/settings/boardnetrecover.h
    src/settings/modelfetcher.cpp
    src/settings/modelfetcher.h
    src/tree/treemodel.cpp
    src/tree/treemodel.h
    src/tree/treewidget.cpp
    src/tree/treewidget.h
    src/tree/nodedialog.cpp
    src/tree/nodedialog.h
    resources/app.qrc
)

set(MOONCODING_GOOGLEPINYIN_SOURCES
    third_party/googlepinyin/src/dictbuilder.cpp
    third_party/googlepinyin/src/dictlist.cpp
    third_party/googlepinyin/src/dicttrie.cpp
    third_party/googlepinyin/src/lpicache.cpp
    third_party/googlepinyin/src/matrixsearch.cpp
    third_party/googlepinyin/src/mystdlib.cpp
    third_party/googlepinyin/src/ngram.cpp
    third_party/googlepinyin/src/pinyinime.cpp
    third_party/googlepinyin/src/searchutility.cpp
    third_party/googlepinyin/src/spellingtable.cpp
    third_party/googlepinyin/src/spellingtrie.cpp
    third_party/googlepinyin/src/splparser.cpp
    third_party/googlepinyin/src/sync.cpp
    third_party/googlepinyin/src/userdict.cpp
    third_party/googlepinyin/src/utf16char.cpp
    third_party/googlepinyin/src/utf16reader.cpp
)

# Buildroot Qt6 Core may be built without zstd while host rcc defaults to zstd.
set(CMAKE_AUTORCC_OPTIONS "--no-zstd")

qt_add_executable(mooncoding WIN32 MACOSX_BUNDLE
    ${MOONCODING_UI_SOURCES}
    ${MOONCODING_GOOGLEPINYIN_SOURCES}
)
add_dependencies(mooncoding vibe_agent_backend vibe_protocol)

target_include_directories(mooncoding PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
    "${CMAKE_CURRENT_SOURCE_DIR}/third_party/googlepinyin/src"
    "${MOONCODING_RUST_DIR}/include"
)
target_link_libraries(mooncoding PRIVATE Qt6::Widgets)

option(MOONCODING_REQUIRE_WEBENGINE
    "Fail configure if Qt6 WebEngineWidgets is missing (board verification)"
    OFF)
option(MOONCODING_DISABLE_WEBENGINE
    "Force QTextBrowser preview even if WebEngineWidgets is available"
    OFF)
find_package(Qt6 QUIET COMPONENTS WebEngineWidgets)
if(MOONCODING_DISABLE_WEBENGINE)
    message(STATUS "MoonCoding: WebEngine disabled by MOONCODING_DISABLE_WEBENGINE — QTextBrowser")
elseif(Qt6WebEngineWidgets_FOUND)
    target_compile_definitions(mooncoding PRIVATE HAS_QT_WEBENGINE=1)
    target_link_libraries(mooncoding PRIVATE Qt6::WebEngineWidgets)
    message(STATUS "MoonCoding: Qt6 WebEngineWidgets enabled")
else()
    if(MOONCODING_REQUIRE_WEBENGINE)
        message(FATAL_ERROR
            "MOONCODING_REQUIRE_WEBENGINE=ON but Qt6 WebEngineWidgets was not found. "
            "Install/build qt6webengine into the Qt prefix/sysroot.")
    endif()
    message(STATUS "MoonCoding: Qt6 WebEngineWidgets not found — HTML preview uses QTextBrowser")
endif()
if(UNIX AND NOT APPLE)
    set_target_properties(mooncoding PROPERTIES
        BUILD_RPATH "$ORIGIN"
        INSTALL_RPATH "$ORIGIN"
    )
    find_package(Threads REQUIRED)
    target_link_libraries(mooncoding PRIVATE Threads::Threads)
endif()

# Ship Google Pinyin system dictionary beside the binary (in-app IME).
add_custom_command(TARGET mooncoding POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E make_directory
            "$<TARGET_FILE_DIR:mooncoding>/ime"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${CMAKE_CURRENT_SOURCE_DIR}/resources/ime/dict_pinyin.dat"
            "$<TARGET_FILE_DIR:mooncoding>/ime/dict_pinyin.dat"
    COMMENT "Copying Google Pinyin dictionary beside MoonCoding UI"
)

# Also ship board net recovery script beside the binary (S99 / manual use).
add_custom_command(TARGET mooncoding POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${CMAKE_CURRENT_SOURCE_DIR}/resources/board/board-net-ready.sh"
            "$<TARGET_FILE_DIR:mooncoding>/board-net-ready.sh"
    COMMENT "Copying board network recovery script beside MoonCoding UI"
)

add_custom_command(TARGET mooncoding POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${MOONCODING_RUST_LIBRARY}"
            "$<TARGET_FILE_DIR:mooncoding>"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${MOONCODING_VIBE_EXECUTABLE}"
            "$<TARGET_FILE_DIR:mooncoding>"
    COMMAND "${CMAKE_COMMAND}" -E make_directory
            "$<TARGET_FILE_DIR:mooncoding>/translations"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${CMAKE_CURRENT_BINARY_DIR}/translations/mooncoding_en.qm"
            "$<TARGET_FILE_DIR:mooncoding>/translations/"
    COMMENT "Copying Rust backend and translations beside MoonCoding UI"
)

# Translations are required on every platform (POST_BUILD copies the .qm).
get_target_property(MOONCODING_QMAKE_EXECUTABLE Qt6::qmake IMPORTED_LOCATION)
get_filename_component(MOONCODING_QT_BIN_DIR "${MOONCODING_QMAKE_EXECUTABLE}" DIRECTORY)
find_program(MOONCODING_LRELEASE
    NAMES lrelease-qt6 lrelease
    HINTS
        "${MOONCODING_QT_BIN_DIR}"
        "/usr/lib/qt6/bin"
        "/usr/lib/x86_64-linux-gnu/qt6/bin"
    REQUIRED
)
add_custom_command(
    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/translations/mooncoding_en.qm"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/translations"
    COMMAND "${MOONCODING_LRELEASE}" "${CMAKE_CURRENT_SOURCE_DIR}/translations/mooncoding_en.ts" -qm "${CMAKE_CURRENT_BINARY_DIR}/translations/mooncoding_en.qm"
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/translations/mooncoding_en.ts"
    COMMENT "Compiling mooncoding_en.ts translation"
)
add_custom_target(mooncoding_translations DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/translations/mooncoding_en.qm")
add_dependencies(mooncoding mooncoding_translations)

if(WIN32)
    find_program(MOONCODING_WINDEPLOYQT
        NAMES windeployqt
        HINTS "${MOONCODING_QT_BIN_DIR}"
        REQUIRED
    )
    add_custom_command(TARGET mooncoding POST_BUILD
        COMMAND "${MOONCODING_WINDEPLOYQT}"
                --no-translations
                --no-system-d3d-compiler
                "$<TARGET_FILE:mooncoding>"
        COMMENT "Deploying Qt runtime beside MoonCoding UI"
        VERBATIM
    )
endif()

option(
    MOONCODING_LINUX_USE_SYSTEM_QT
    "Enable Linux install targets that depend on distribution-provided Qt 6"
    OFF
)
if(UNIX AND NOT APPLE AND NOT MOONCODING_LINUX_USE_SYSTEM_QT)
    message(WARNING
        "Linux install targets are disabled to avoid an incomplete package. "
        "Use an AppImage/linuxdeploy pipeline, or configure "
        "-DMOONCODING_LINUX_USE_SYSTEM_QT=ON for a system-Qt package.")
else()
    install(TARGETS mooncoding
        BUNDLE DESTINATION .
        RUNTIME DESTINATION bin
    )
    if(APPLE)
        install(FILES
            "${MOONCODING_RUST_LIBRARY}"
            "${MOONCODING_VIBE_EXECUTABLE}"
            DESTINATION "mooncoding.app/Contents/MacOS"
        )
    else()
        install(FILES
            "${MOONCODING_RUST_LIBRARY}"
            "${MOONCODING_VIBE_EXECUTABLE}"
            DESTINATION bin
        )
    endif()
    if(WIN32 OR APPLE)
        qt_generate_deploy_app_script(
            TARGET mooncoding
            OUTPUT_SCRIPT mooncoding_deploy_script
        )
        install(SCRIPT ${mooncoding_deploy_script})
    endif()
endif()

if(MOONCODING_BUILD_TESTS)
    qt_add_executable(mooncoding_ui_tests
        tests/treemodel_test.cpp
        src/input/inputwidget.cpp
        src/input/inputwidget.h
        src/tree/treemodel.cpp
        src/tree/treemodel.h
    )
    target_include_directories(mooncoding_ui_tests PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/src")
    target_link_libraries(mooncoding_ui_tests PRIVATE Qt6::Widgets Qt6::Test)
    add_test(NAME mooncoding_ui_tests COMMAND mooncoding_ui_tests)
    if(WIN32)
        set_tests_properties(mooncoding_ui_tests PROPERTIES
            ENVIRONMENT_MODIFICATION
                "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>"
        )
    endif()
endif()
