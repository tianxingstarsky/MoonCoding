#pragma once

#include <QHash>
#include <QList>
#include <QStringList>
#include <QWidget>

class QHBoxLayout;
class QLabel;
class QPushButton;
class QScrollArea;
class QToolButton;
class QVBoxLayout;

/// Touch soft keyboard with phone-like proportions (open, in-app).
/// EN / 拼音（整句自动分词 + 横滑候选）+ 123 / #+=。
class SoftKeyboard final : public QWidget
{
    Q_OBJECT

public:
    explicit SoftKeyboard(QWidget *parent = nullptr);

signals:
    void textCommitted(const QString &text);
    void backspacePressed();
    void enterPressed();
    void hideRequested();

public slots:
    void clearComposing();

private slots:
    void onCharClicked();
    void onCandidateClicked();
    void onSpecialClicked();
    void pageCandidates(int delta);

private:
    enum Mode { English, Pinyin };
    enum Page { Letters, Digits, Symbols };

    void buildUi();
    void rebuildKeys();
    void rebuildLetterPage();
    void rebuildDigitPage();
    void rebuildSymbolPage();
    void addCharRow(QVBoxLayout *keysLayout, const QString &chars, int sidePadStretch = 0);
    void addBottomRow(bool fromSymbolPage);
    void updateCandidates();
    void renderCandidatePage();
    void commitPinyinChoice(const QString &hanzi, const QString &matchedPy);
    void appendLatin(const QString &ch);
    void addCandidate(const QString &word, const QString &matchedPy);
    void prependCandidate(const QString &word, const QString &matchedPy);
    void addAutoSegmentSentence(const QString &py);
    void updateCandidatesFromEngine();
    void commitEngineChoice(int candId);
    QStringList lookupPinyin(const QString &py) const;
    QString longestDictPrefix(const QString &s) const;
    void loadDictionary();
    QPushButton *makeKey(const QString &label, const QString &code,
                         int stretch = 1, const QString &objectName = QStringLiteral("imeKey"));

    Mode m_mode = Pinyin;
    Page m_page = Letters;
    bool m_shift = false;
    bool m_useEngine = false;
    QString m_composing;
    QHash<QString, QStringList> m_dict;
    QStringList m_allCandidates;
    QStringList m_allMatchedPy;
    QList<int> m_allCandIds;
    int m_candidatePage = 0;
    int m_maxKeyLen = 1;

    QLabel *m_composeLabel = nullptr;
    QWidget *m_candidateHost = nullptr;
    QPushButton *m_prevPageBtn = nullptr;
    QPushButton *m_nextPageBtn = nullptr;
    QScrollArea *m_candidateScroll = nullptr;
    QWidget *m_candidateStrip = nullptr;
    QHBoxLayout *m_candidateLayout = nullptr;
    QVBoxLayout *m_letterHostLayout = nullptr;
    QToolButton *m_modeBtn = nullptr;
};
