// =============================================================================
// opencode_chat_widget.cpp — Chat message display implementation
// =============================================================================

#include "opencode_chat_widget.h"
#include "opencode_tool_block.h"

#include <QScrollBar>
#include <QTextCursor>
#include <QFont>
#include <QApplication>
#include <QRegularExpression>

// Forward declaration: basic markdown-to-HTML converter used by MessageBlock::setText.
static QString markdownToRichText(const QString& md);

// =============================================================================
// MessageBlock implementation
// =============================================================================

MessageBlock::MessageBlock(Role role, QWidget* parent)
    : QFrame(parent), m_role(role)
{
    setupUi();
    applyRoleStyle();
}

void MessageBlock::setupUi()
{
    // opencode design: Messages have a horizontal layout:
    //   [accent_bar(4px)] [content_area]
    // No bubbles, no shadows — flat terminal aesthetic.
    auto* hLayout = new QHBoxLayout(this);
    hLayout->setContentsMargins(0, 6, 12, 6);
    hLayout->setSpacing(0);

    // Left accent bar — the key opencode visual element.
    // In opencode TUI: colored left border (4px wide) using lipgloss Border.Left.
    // User messages get accent color; assistant gets muted; tool blocks have status color.
    m_accentBar = new QFrame(this);
    m_accentBar->setFixedWidth(4);
    m_accentBar->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    m_accentBar->setObjectName(QStringLiteral("MessageAccentBar"));
    hLayout->addWidget(m_accentBar);

    // Spacer between accent bar and content (8px, matching opencode padding).
    hLayout->addSpacing(8);

    // Content area: role label + timestamp + text.
    auto* contentLayout = new QVBoxLayout();
    contentLayout->setContentsMargins(0, 0, 0, 0);
    contentLayout->setSpacing(2);

    // Role label row (e.g., "You" / "Claude 4.5 · 2.3s")
    auto* headerRow = new QHBoxLayout();
    headerRow->setContentsMargins(0, 0, 0, 0);
    headerRow->setSpacing(8);

    m_roleLabel = new QLabel(this);
    m_roleLabel->setObjectName(QStringLiteral("MessageRoleLabel"));
    QFont roleFont = m_roleLabel->font();
    roleFont.setPointSize(roleFont.pointSize() - 1);
    roleFont.setBold(true);
    m_roleLabel->setFont(roleFont);
    headerRow->addWidget(m_roleLabel);

    m_timestampLabel = new QLabel(this);
    m_timestampLabel->setObjectName(QStringLiteral("MessageTimestampLabel"));
    QFont tsFont = m_timestampLabel->font();
    tsFont.setPointSize(tsFont.pointSize() - 2);
    m_timestampLabel->setFont(tsFont);
    headerRow->addWidget(m_timestampLabel);

    headerRow->addStretch();
    contentLayout->addLayout(headerRow);

    // Text browser for markdown content.
    // opencode: Uses glamour for markdown rendering. We approximate with
    // QTextBrowser's rich text + custom CSS.
    m_textBrowser = new QTextBrowser(this);
    m_textBrowser->setObjectName(QStringLiteral("MessageTextBrowser"));
    m_textBrowser->setOpenExternalLinks(true);
    m_textBrowser->setReadOnly(true);
    m_textBrowser->setFrameShape(QFrame::NoFrame);
    m_textBrowser->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_textBrowser->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_textBrowser->viewport()->setAutoFillBackground(false);

    // opencode design: Monospace font family for code feel.
    // IBM Plex Mono is the opencode standard. Fall back to system monospace.
    QFont textFont = m_textBrowser->font();
    textFont.setPointSize(textFont.pointSize());
    m_textBrowser->setFont(textFont);

    // Disable the default document margin for tight text fit.
    m_textBrowser->document()->setDocumentMargin(0);

    contentLayout->addWidget(m_textBrowser);
    hLayout->addLayout(contentLayout, 1);

    // Streaming indicator — blinks the timestamp label between "streaming..." and "▍"
    m_cursorTimer = new QTimer(this);
    m_cursorTimer->setInterval(530);
    connect(m_cursorTimer, &QTimer::timeout, this, [this]() {
        m_cursorVisible = !m_cursorVisible;
        if (m_role == Assistant) {
            m_timestampLabel->setText(m_cursorVisible
                ? QStringLiteral("▍")
                : QString());
        }
    });
}

void MessageBlock::applyRoleStyle()
{
    // opencode design: Per-role styling via theme keys.
    // User messages: sender/agent accent color on left bar
    // Assistant messages: muted/border color
    // System messages: info color
    switch (m_role) {
    case User:
        // MoonCoding adaptation: slightly different accent palette than
        // opencode's phosphor-green, tuned for Qt6 rendering.
        m_accentBar->setStyleSheet(
            QStringLiteral("background-color: #58a6ff; border-radius: 2px;"));
        m_roleLabel->setText(tr("You"));
        m_roleLabel->setStyleSheet(QStringLiteral("color: #58a6ff;"));
        break;
    case Assistant:
        m_accentBar->setStyleSheet(
            QStringLiteral("background-color: #8b949e; border-radius: 2px;"));
        m_roleLabel->setText(tr("Assistant"));
        m_roleLabel->setStyleSheet(QStringLiteral("color: #8b949e;"));
        break;
    case System:
        m_accentBar->setStyleSheet(
            QStringLiteral("background-color: #3fb950; border-radius: 2px;"));
        m_roleLabel->setText(tr("System"));
        m_roleLabel->setStyleSheet(QStringLiteral("color: #3fb950;"));
        break;
    }
}

MessageBlock* MessageBlock::appendText(const QString& text)
{
    m_accumulatedText += text;

    // Use QTextCursor to append without replacing entire document.
    // This preserves the streaming cursor position correctly.
    QTextCursor cursor = m_textBrowser->textCursor();
    cursor.movePosition(QTextCursor::End);
    cursor.insertText(text);

    // Ensure the end is visible.
    m_textBrowser->moveCursor(QTextCursor::End);
    return this;
}

void MessageBlock::setText(const QString& markdown)
{
    m_accumulatedText = markdown;
    // opencode: Markdown rendered via glamour/chroma.
    // Qt6 adaptation: Use QTextBrowser's HTML subset for basic markdown.
    // Rich text is approximate — code blocks get monospace via <pre>.
    QString html = markdownToRichText(markdown);
    m_textBrowser->setHtml(html);
}

QString MessageBlock::text() const
{
    return m_accumulatedText;
}

void MessageBlock::showStreamingCursor(bool show)
{
    if (show && !m_cursorTimer->isActive()) {
        m_cursorTimer->start();
    } else if (!show) {
        m_cursorTimer->stop();
    }
}

void MessageBlock::setTimestamp(const QString& ts)
{
    m_timestampLabel->setText(ts);
}

void MessageBlock::setRoleLabel(const QString& label)
{
    m_roleLabel->setText(label);
}

// ---------------------------------------------------------------------------
// Basic markdown-to-rich-text converter.
// opencode uses glamour for full markdown. This is a minimal approximation
// that covers the most common patterns in AI chat responses.
// ---------------------------------------------------------------------------
static QString markdownToRichText(const QString& md)
{
    // MoonCoding adaptation: A full markdown renderer (like cmark or md4c)
    // would be preferable for production. This works for the demo.
    QString result;
    result.reserve(md.size() * 2);
    result += QStringLiteral("<html><body style='color:#c9d1d9;'>");

    const QStringList lines = md.split(QStringLiteral("\n"));
    bool inCodeBlock = false;
    QString codeLang;

    for (const QString& rawLine : lines) {
        QString line = rawLine;

        // Code block detection.
        if (line.startsWith(QStringLiteral("```"))) {
            inCodeBlock = !inCodeBlock;
            if (!inCodeBlock) {
                result += QStringLiteral("</pre>");
            } else {
                codeLang = line.mid(3).trimmed();
                result += QStringLiteral(
                    "<pre style='background-color:#161b22; color:#c9d1d9; "
                    "padding:12px; border-radius:6px; font-family:\"Consolas\","
                    "\"IBM Plex Mono\",monospace; font-size:13px; "
                    "border:1px solid #30363d; overflow-x:auto;'>");
            }
            continue;
        }
        if (inCodeBlock) {
            result += line.toHtmlEscaped() + QStringLiteral("\n");
            continue;
        }

        // Inline code: `text`
        {
            QString processed;
            int i = 0;
            while (i < line.size()) {
                int tick = line.indexOf(QStringLiteral("`"), i);
                if (tick < 0) {
                    processed += line.mid(i).toHtmlEscaped();
                    break;
                }
                processed += line.mid(i, tick - i).toHtmlEscaped();
                int endTick = line.indexOf(QStringLiteral("`"), tick + 1);
                if (endTick < 0) {
                    processed += QStringLiteral("`");
                    i = tick + 1;
                } else {
                    QString codeText = line.mid(tick + 1, endTick - tick - 1);
                    processed += QStringLiteral(
                        "<code style='background-color:#30363d; color:#c9d1d9; "
                        "padding:2px 4px; border-radius:4px; font-family:\"Consolas\","
                        "\"IBM Plex Mono\",monospace; font-size:0.9em;'>")
                        + codeText.toHtmlEscaped() + QStringLiteral("</code>");
                    i = endTick + 1;
                }
            }
            line = processed;
        }

        // Bold: **text**
        line.replace(QRegularExpression(QStringLiteral("\\*\\*(.+?)\\*\\*")),
                     QStringLiteral("<b style='color:#e6edf3;'>\\1</b>"));
        // Italic: *text*
        line.replace(QRegularExpression(QStringLiteral("(?<!\\*)\\*([^*]+)\\*(?!\\*)")),
                     QStringLiteral("<i>\\1</i>"));

        // Headers.
        if (line.startsWith(QStringLiteral("### "))) {
            result += QStringLiteral(
                "<h4 style='color:#79c0ff; margin:8px 0 4px 0;'>")
                + line.mid(4).toHtmlEscaped() + QStringLiteral("</h4>");
        } else if (line.startsWith(QStringLiteral("## "))) {
            result += QStringLiteral(
                "<h3 style='color:#79c0ff; margin:12px 0 4px 0;'>")
                + line.mid(3).toHtmlEscaped() + QStringLiteral("</h3>");
        } else if (line.startsWith(QStringLiteral("# "))) {
            result += QStringLiteral(
                "<h2 style='color:#79c0ff; margin:16px 0 6px 0;'>")
                + line.mid(2).toHtmlEscaped() + QStringLiteral("</h2>");
        } else if (line.startsWith(QStringLiteral("- ")) || line.startsWith(QStringLiteral("* "))) {
            result += QStringLiteral(
                "<span style='margin-left:16px;'>• </span>")
                + line.mid(2) + QStringLiteral("<br>");
        } else if (line.trimmed().isEmpty()) {
            result += QStringLiteral("<br>");
        } else {
            result += line + QStringLiteral("<br>");
        }
    }
    if (inCodeBlock) {
        result += QStringLiteral("</pre>");
    }
    result += QStringLiteral("</body></html>");
    return result;
}

// =============================================================================
// OpenCodeChatWidget implementation
// =============================================================================

OpenCodeChatWidget::OpenCodeChatWidget(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeChatWidget::setupUi()
{
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // Info bar: model name, token count, step count.
    // opencode: Info shown in prompt area and sidebar. We put it at top
    // for visibility in the Qt widget.
    m_infoLabel = new QLabel(this);
    m_infoLabel->setObjectName(QStringLiteral("InfoBar"));
    m_infoLabel->setFixedHeight(28);
    m_infoLabel->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    QFont infoFont = m_infoLabel->font();
    infoFont.setPointSize(infoFont.pointSize() - 1);
    m_infoLabel->setFont(infoFont);
    m_infoLabel->hide(); // Hidden by default, shown when set.
    mainLayout->addWidget(m_infoLabel);

    // Scroll area containing message list.
    m_scrollArea = new QScrollArea(this);
    m_scrollArea->setObjectName(QStringLiteral("ChatScrollArea"));
    m_scrollArea->setWidgetResizable(true);
    m_scrollArea->setFrameShape(QFrame::NoFrame);
    m_scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    // opencode design: Custom scrollbar styling via QSS.
    m_scrollArea->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);

    // Message container.
    m_messageContainer = new QWidget(m_scrollArea);
    m_messageContainer->setObjectName(QStringLiteral("MessageContainer"));
    m_messageLayout = new QVBoxLayout(m_messageContainer);
    m_messageLayout->setContentsMargins(8, 8, 8, 16);
    m_messageLayout->setSpacing(12);
    m_messageLayout->addStretch(1); // Push messages to top.

    m_scrollArea->setWidget(m_messageContainer);
    mainLayout->addWidget(m_scrollArea, 1);

    // Smart scroll detection (opencode: "scroll to follow").
    connect(m_scrollArea->verticalScrollBar(), &QScrollBar::valueChanged,
            this, &OpenCodeChatWidget::onScrollChanged);
}

MessageBlock* OpenCodeChatWidget::addMessage(MessageBlock::Role role,
                                               const QString& text)
{
    auto* block = new MessageBlock(role, m_messageContainer);
    if (!text.isEmpty()) {
        block->setText(text);
    }
    // Insert before the stretch.
    m_messageLayout->insertWidget(m_messageLayout->count() - 1, block);
    m_messages.append(block);
    return block;
}

OpenCodeToolBlock* OpenCodeChatWidget::addToolBlock()
{
    auto* block = new OpenCodeToolBlock(m_messageContainer);
    m_messageLayout->insertWidget(m_messageLayout->count() - 1, block);
    m_toolBlocks.append(block);
    return block;
}

void OpenCodeChatWidget::addSystemMessage(const QString& text)
{
    auto* block = addMessage(MessageBlock::System, text);
    block->setRoleLabel(tr("Info"));
    block->setText(text);
}

void OpenCodeChatWidget::appendToLastMessage(const QString& chunk)
{
    if (m_messages.isEmpty()) return;
    auto* last = m_messages.last();
    last->appendText(chunk);

    // Auto-scroll to follow streaming content.
    if (m_autoScroll) {
        scrollToBottom();
    }
}

void OpenCodeChatWidget::clear()
{
    // Remove all message widgets from layout (leaving the stretch).
    while (m_messageLayout->count() > 1) {
        QLayoutItem* item = m_messageLayout->takeAt(0);
        if (item->widget()) {
            item->widget()->deleteLater();
        }
        delete item;
    }
    m_messages.clear();
    m_toolBlocks.clear();
}

void OpenCodeChatWidget::scrollToBottom()
{
    auto* sb = m_scrollArea->verticalScrollBar();
    sb->setValue(sb->maximum());
}

bool OpenCodeChatWidget::isAtBottom() const
{
    auto* sb = m_scrollArea->verticalScrollBar();
    return sb->value() >= sb->maximum() - 10; // 10px tolerance.
}

void OpenCodeChatWidget::onScrollChanged(int value)
{
    // opencode pattern: If user scrolls up, stop auto-following.
    // Resume auto-follow when user scrolls back to bottom.
    auto* sb = m_scrollArea->verticalScrollBar();
    m_autoScroll = (value >= sb->maximum() - 20);

    if (!m_autoScroll) {
        emit userScrolledUp();
    }
}

void OpenCodeChatWidget::setInfoText(const QString& text)
{
    m_infoLabel->setText(text);
    m_infoLabel->setVisible(!text.isEmpty());
}
