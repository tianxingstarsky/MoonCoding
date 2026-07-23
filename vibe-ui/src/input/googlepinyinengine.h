#pragma once

#include <QString>
#include <QStringList>

/// Thin wrapper around AOSP Google Pinyin IME (Apache-2.0).
/// Full system dictionary: resources/ime/dict_pinyin.dat (~1MB).
class GooglePinyinEngine final
{
public:
    static GooglePinyinEngine &instance();

    bool open(const QString &sysDictPath);
    void close();
    bool isReady() const { return m_ready; }

    void resetSearch();
    /// Returns candidate count for the full pinyin string.
    int search(const QString &pinyin);
    int candidateCount() const { return m_count; }
    QString candidateAt(int index) const;
    /// Choose candidate; returns new candidate count (engine state advances).
    int choose(int index);

private:
    GooglePinyinEngine() = default;
    ~GooglePinyinEngine();
    GooglePinyinEngine(const GooglePinyinEngine &) = delete;
    GooglePinyinEngine &operator=(const GooglePinyinEngine &) = delete;

    bool m_ready = false;
    int m_count = 0;
};
