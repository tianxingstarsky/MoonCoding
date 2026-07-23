#include "languagemanager.h"

#include <QApplication>
#include <QDir>
#include <QSettings>

LanguageManager &LanguageManager::instance()
{
    static LanguageManager mgr;
    return mgr;
}

LanguageManager::LanguageManager()
    : m_current(QStringLiteral("zh"))
{
}

void LanguageManager::setLanguage(const QString &lang)
{
    if (lang == m_current) {
        return;
    }
    m_current = lang;
    QSettings().setValue(QStringLiteral("appearance/language"), lang);
    apply();
}

void LanguageManager::apply()
{
    if (m_translatorLoaded) {
        qApp->removeTranslator(&m_translator);
        m_translatorLoaded = false;
    }
    if (m_current == QStringLiteral("en")) {
        const QString qmPath =
            QCoreApplication::applicationDirPath() + QStringLiteral("/translations/mooncoding_en.qm");
        if (m_translator.load(qmPath)) {
            qApp->installTranslator(&m_translator);
            m_translatorLoaded = true;
        }
    }
    emit languageChanged(m_current);
}

QString LanguageManager::currentLanguage() const
{
    return m_current;
}
