#pragma once

#include <QHash>
#include <QJsonArray>
#include <QWidget>

class QLabel;
class QPushButton;
class QResizeEvent;
class QScrollArea;
class QTimer;
class QVBoxLayout;

class ChatWidget final : public QWidget
{
    Q_OBJECT

public:
    explicit ChatWidget(QWidget *parent = nullptr);

public slots:
    void appendUserMessage(const QString &text);
    void beginAssistantMessage();
    void appendThinkingDelta(const QString &delta);
    void appendAssistantDelta(const QString &delta);
    void finishAssistantMessage(const QString &content, quint64 tokensIn, quint64 tokensOut);
    void showToolStart(const QString &id, const QString &name, const QString &input);
    void showToolResult(
        const QString &id,
        const QString &name,
        const QString &output,
        int exitCode,
        quint64 durationMs);
    void showError(const QString &message);
    void showInterrupted(const QString &reason);
    void agentDone(quint64 tokensIn, quint64 tokensOut, quint64 steps);
    void setMessages(const QJsonArray &messages);
    void clear();
    void refreshFonts();

private:
    struct ToolBlock {
        QWidget *widget = nullptr;
        QLabel *icon = nullptr;
        QLabel *name = nullptr;
        QLabel *detail = nullptr;
        QPushButton *toggle = nullptr;
        QLabel *output = nullptr;
        bool expanded = false;
    };

    struct ThinkingSegment {
        QWidget *widget = nullptr;
        QPushButton *toggle = nullptr;
        QLabel *summary = nullptr;
        QLabel *body = nullptr;
        QString text;
        int index = 0;
        bool sealed = false;
    };

    struct AnswerSegment {
        QWidget *widget = nullptr;
        QLabel *body = nullptr;
        QString text;
        int index = 0;
        bool sealed = false;
    };

    struct MessageWidgets {
        QWidget *row = nullptr;
        QWidget *accent = nullptr;
        QWidget *content = nullptr;
        QLabel *roleLabel = nullptr;
        QLabel *meta = nullptr;
        QVBoxLayout *timeline = nullptr;
        QLabel *body = nullptr;
        bool user = false;
    };

    MessageWidgets createMessage(const QString &role, bool user);
    ToolBlock createToolBlock(const QString &iconChar, const QString &name,
                              const QString &detail, const QString &output,
                              bool ok, bool autoExpand = false);
    void scrollToBottom();
    void flushScrollToBottom();
    void setStreamingAccent(bool active);
    void sealActiveThinking();
    void openThinkingSegment();
    void flushThinkingBody();
    void sealActiveAnswer();
    void openAnswerSegment();
    void flushAnswerBody();
    void scheduleAnswerFlush();
    void finalizeCard();
    void reflowMessageBodies();
    void constrainChatColumn();
    static QString clippedPlain(const QString &text, int maxChars);

protected:
    void resizeEvent(QResizeEvent *event) override;

private:
    QScrollArea *m_scrollArea;
    QWidget *m_messageContainer;
    QVBoxLayout *m_messageLayout;
    MessageWidgets m_streamingMessage;
    ThinkingSegment m_activeThinking;
    AnswerSegment m_activeAnswer;
    QString m_streamingText;
    QHash<QString, QString> m_pendingToolNames;
    QHash<QString, QString> m_pendingToolInputs;
    QHash<QString, ToolBlock> m_pendingToolBlocks;
    QTimer *m_thinkingFlushTimer = nullptr;
    QTimer *m_answerFlushTimer = nullptr;
    QTimer *m_scrollTimer = nullptr;
    bool m_scrollPending = false;
    int m_toolStep = 0;
    int m_thinkingCount = 0;
    int m_answerCount = 0;
};
