// =============================================================================
// opencode_antialias.h — Global font anti-aliasing and CJK font stack
//
// opencode desktop v2 applies font smoothing everywhere. This utility provides
// a single entry point to configure QApplication-wide fonts with:
//   - PreferAntialias (sub-pixel rendering where supported)
//   - Proper CJK fallback chain (Microsoft YaHei → PingFang SC → Noto Sans CJK SC)
//   - Monospace fallback for code
//
// Usage: Call opencode::applyFontConfig(app) after creating QApplication.
// =============================================================================

#ifndef OPENCODE_ANTIALIAS_H
#define OPENCODE_ANTIALIAS_H

#include <QApplication>
#include <QFont>

namespace opencode {

// ---------------------------------------------------------------------------
// Apply anti-aliased fonts globally. Must be called AFTER QApplication
// construction but BEFORE any widgets are created.
// ---------------------------------------------------------------------------
inline void applyFontConfig(QApplication& app)
{
    // opencode desktop v2 font: Inter Variable on production builds,
    // system sans-serif otherwise. On Windows/CJK systems, we use
    // Microsoft YaHei as the primary UI font with anti-aliasing.
    QFont uiFont;
    uiFont.setStyleStrategy(QFont::PreferAntialias);
    uiFont.setHintingPreference(QFont::PreferFullHinting);
    uiFont.setPixelSize(14);

    // CJK font fallback chain — opencode design uses system-native fonts.
    // Priority: Windows (Microsoft YaHei) → macOS (PingFang SC) → Linux (Noto Sans CJK SC)
#ifdef Q_OS_WIN
    uiFont.setFamilies({
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("Segoe UI"),
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("PingFang SC"),
        QStringLiteral("sans-serif")
    });
#elif defined(Q_OS_MACOS)
    uiFont.setFamilies({
        QStringLiteral("PingFang SC"),
        QStringLiteral("SF Pro Display"),
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("sans-serif")
    });
#else
    uiFont.setFamilies({
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("PingFang SC"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("DejaVu Sans"),
        QStringLiteral("sans-serif")
    });
#endif

    app.setFont(uiFont);

    // Also apply anti-aliasing via stylesheet as a fallback.
    // Qt's QSS font-smooth is not standard CSS but may help on some platforms.
    app.setStyleSheet(app.styleSheet() +
        QStringLiteral("* { font-smooth: always; -webkit-font-smoothing: antialiased; }"));
}

// ---------------------------------------------------------------------------
// Create a monospace font for code display (tool output, diffs, etc.)
// ---------------------------------------------------------------------------
inline QFont monospaceFont(int pixelSize = 13)
{
    QFont font;
    font.setStyleStrategy(QFont::PreferAntialias);
    font.setHintingPreference(QFont::PreferFullHinting);
    font.setPixelSize(pixelSize);
    font.setFamilies({
        QStringLiteral("Cascadia Code"),
        QStringLiteral("Consolas"),
        QStringLiteral("IBM Plex Mono"),
        QStringLiteral("Fira Code"),
        QStringLiteral("Source Code Pro"),
        QStringLiteral("Courier New"),
        QStringLiteral("monospace")
    });
    return font;
}

// ---------------------------------------------------------------------------
// Create a small UI font (for labels, context bars, status text)
// ---------------------------------------------------------------------------
inline QFont smallFont(int pixelSize = 12)
{
    QFont font = QApplication::font();
    font.setStyleStrategy(QFont::PreferAntialias);
    font.setHintingPreference(QFont::PreferFullHinting);
    font.setPixelSize(pixelSize);
    return font;
}

} // namespace opencode

#endif // OPENCODE_ANTIALIAS_H
