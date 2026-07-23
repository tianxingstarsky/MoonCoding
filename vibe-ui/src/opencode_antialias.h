// =============================================================================
// opencode_antialias.h — Global font anti-aliasing and CJK font stack
// =============================================================================

#ifndef OPENCODE_ANTIALIAS_H
#define OPENCODE_ANTIALIAS_H

#include <QApplication>
#include <QFont>
#include <QStringList>

namespace opencode {

inline QStringList uiFontFamilies()
{
#ifdef Q_OS_WIN
    return {
        QStringLiteral("Microsoft YaHei UI"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("Segoe UI"),
        QStringLiteral("Segoe UI Symbol"),
        QStringLiteral("Segoe UI Emoji"),
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("PingFang SC"),
        QStringLiteral("Arial Unicode MS"),
        QStringLiteral("sans-serif")
    };
#elif defined(Q_OS_MACOS)
    return {
        QStringLiteral("PingFang SC"),
        QStringLiteral("SF Pro Display"),
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("Apple Color Emoji"),
        QStringLiteral("sans-serif")
    };
#else
    return {
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("Noto Sans SC"),
        QStringLiteral("Noto Sans CJK"),
        QStringLiteral("WenQuanYi Micro Hei"),
        QStringLiteral("Source Han Sans SC"),
        QStringLiteral("Droid Sans Fallback"),
        QStringLiteral("Liberation Sans"),
        QStringLiteral("DejaVu Sans"),
        QStringLiteral("PingFang SC"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("sans-serif")
    };
#endif
}

inline QStringList monoFontFamilies()
{
    return {
        QStringLiteral("Cascadia Mono"),
        QStringLiteral("Cascadia Code"),
        QStringLiteral("Consolas"),
        QStringLiteral("Microsoft YaHei UI"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("Noto Sans Mono CJK SC"),
        QStringLiteral("Courier New"),
        QStringLiteral("monospace")
    };
}

inline void installFontSubstitutions()
{
    // QSS often pins Consolas/Courier; without substitutions CJK becomes "?".
    const QStringList cjkFallback = {
        QStringLiteral("Noto Sans CJK SC"),
        QStringLiteral("Noto Sans SC"),
        QStringLiteral("WenQuanYi Micro Hei"),
        QStringLiteral("Source Han Sans SC"),
        QStringLiteral("Droid Sans Fallback"),
        QStringLiteral("Microsoft YaHei UI"),
        QStringLiteral("Microsoft YaHei"),
        QStringLiteral("Liberation Sans"),
        QStringLiteral("DejaVu Sans")
    };
    for (const QString &family : {
             QStringLiteral("Consolas"),
             QStringLiteral("Courier New"),
             QStringLiteral("Cascadia Mono"),
             QStringLiteral("Cascadia Code"),
             QStringLiteral("monospace"),
             QStringLiteral("Microsoft YaHei"),
             QStringLiteral("Microsoft YaHei UI"),
             QStringLiteral("Segoe UI")
         }) {
        QFont::insertSubstitutions(family, cjkFallback);
    }
}

inline void applyFontConfig(QApplication &app)
{
    installFontSubstitutions();

    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");

    QFont uiFont;
    if (board) {
        // Cortex-A7 + linuxfb: AA + full hinting + VF fonts dominate CPU.
        uiFont.setStyleStrategy(QFont::NoAntialias);
        uiFont.setHintingPreference(QFont::PreferNoHinting);
        uiFont.setPixelSize(15);
    } else {
        uiFont.setStyleStrategy(QFont::PreferAntialias);
        uiFont.setHintingPreference(QFont::PreferFullHinting);
        uiFont.setPixelSize(14);
    }
    uiFont.setFamilies(uiFontFamilies());
    uiFont.setStyleHint(QFont::SansSerif);
    app.setFont(uiFont);
}

inline QFont monospaceFont(int pixelSize = 13)
{
    QFont font;
    font.setStyleStrategy(QFont::PreferAntialias);
    font.setHintingPreference(QFont::PreferFullHinting);
    font.setPixelSize(pixelSize);
    font.setFamilies(monoFontFamilies());
    font.setStyleHint(QFont::Monospace);
    return font;
}

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
