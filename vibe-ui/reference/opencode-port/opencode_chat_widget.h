// =============================================================================
// opencode_chat_widget.h — Chat message display widget
//
// Replicates opencode TUI's chat display pattern:
//   - Flat message design with subtle left accent bar (NOT bubbles)
//   - Streaming text support via QTextBrowser
//   - Inline tool call blocks (collapsible)
//   - Auto-scroll with smart "scroll to follow"
//   - Markdown-like rendering (headers, code blocks, bold/italic)
//
// opencode design: Messages are rendered flat with a colored left border that
// indicates the source (user = accent color, assistant = muted, tool = status).
// No message bubbles — the terminal aesthetic is preserved.
//
// MoonCoding adaptation: QTextBrowser replaces Bubble Tea's virtual text
// rendering; QVBoxLayout replaces Elm-like update/view architecture.
// =============================================================================

#ifndef OPENCODE_CHAT_WIDGET_H
#define OPENCODE_CHAT_WIDGET_H

#include <QWidget>
#include <QVBoxLayout>
#include <QScrollArea>
#include <QTextBrowser>
#include <QLabel>
#include <QTimer>
#include <QMap>

class OpenCodeToolBlock;

// ---------------------------------------------------------------------------
// MessageBlock — A single message (user or assistant) in the chat flow
// ---------------------------------------------------------------------------
class MessageBlock : public QFrame
{
    Q_OBJECT

public:
    enum Role { User, Assistant, System };

    explicit MessageBlock(Role role, QWidget* parent = nullptr);

    // Append text (for streaming). Returns this for chaining.
    MessageBlock* appendText(const QString& text);

    // Set complete message text (replaces content, stops streaming cursor).
    void setText(const QString& markdown);

    // Direct access to the text browser for tool result insertion.
    QTextBrowser* textBrowser() const { return m_textBrowser; }

    // Get current accumulated text.
    QString text() const;

    // Show/hide a streaming cursor.
    void showStreamingCursor(bool show);

    // Hours (displayed next to user/assistant label).
    void setTimestamp(const QString& ts);
    void setRoleLabel(const QString& label);

private:
    void setupUi();
    void applyRoleStyle();

    Role m_role;
    QLabel* m_roleLabel = nullptr;
    QLabel* m_timestampLabel = nullptr;
    QTextBrowser* m_textBrowser = nullptr;
    QFrame* m_accentBar = nullptr;      // Left accent border (opencode style)
    QTimer* m_cursorTimer = nullptr;
    bool m_cursorVisible = false;
    QString m_accumulatedText;
};

// ---------------------------------------------------------------------------
// OpenCodeChatWidget — Scrollable chat history with streaming support
// ---------------------------------------------------------------------------
class OpenCodeChatWidget : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeChatWidget(QWidget* parent = nullptr);

    // Add a new message block. Returns the block for chaining.
    MessageBlock* addMessage(MessageBlock::Role role, const QString& text = {});

    // Insert a tool call block inline between messages.
    OpenCodeToolBlock* addToolBlock();

    // Add a system/info message (e.g., "Session started", "Model: Claude 4.5").
    void addSystemMessage(const QString& text);

    // Streaming support: append text to the last message block.
    void appendToLastMessage(const QString& chunk);

    // Clear all messages.
    void clear();

    // Scroll control (opencode: smart auto-scroll, respects manual scroll-up).
    void scrollToBottom();
    bool isAtBottom() const;

    // Set the model/context info bar text.
    void setInfoText(const QString& text);

signals:
    void userScrolledUp();   // User has scrolled away from bottom

private slots:
    void onScrollChanged(int value);

private:
    void setupUi();

    QScrollArea* m_scrollArea = nullptr;
    QWidget* m_messageContainer = nullptr;
    QVBoxLayout* m_messageLayout = nullptr;
    QLabel* m_infoLabel = nullptr;       // Model info / context bar
    bool m_autoScroll = true;            // Smart scroll-to-follow
    QList<MessageBlock*> m_messages;     // All message blocks
    QList<OpenCodeToolBlock*> m_toolBlocks; // All tool blocks
};

#endif // OPENCODE_CHAT_WIDGET_H
