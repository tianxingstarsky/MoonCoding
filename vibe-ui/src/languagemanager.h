#pragma once

#include <QObject>
#include <QTranslator>

class LanguageManager final : public QObject
{
    Q_OBJECT

public:
    static LanguageManager &instance();
    void setLanguage(const QString &lang);
    QString currentLanguage() const;
    void apply();

signals:
    void languageChanged(const QString &lang);

private:
    LanguageManager();
    QTranslator m_translator;
    QString m_current;
    bool m_translatorLoaded = false;
};
