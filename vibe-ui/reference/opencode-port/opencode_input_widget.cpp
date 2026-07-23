// =============================================================================
// opencode_input_widget.cpp — Chat input widget implementation
// =============================================================================

#include "opencode_input_widget.h"

#include <QKeyEvent>
#include <QFont>
#include <QFontMetrics>
#include <QTextBlock>
#include <QApplication>

// =============================================================================
// Construction
// =============================================================================

OpenCodeInputWidget::OpenCodeInputWidget(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeInputWidget::setupUi()
{
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // -----------------------------------------------------------------------
    // Context bar — shows model, tokens, steps.
    // opencode: Info shown inside prompt box or sidebar.
    // -----------------------------------------------------------------------
    m_contextBar = new QFrame(this);
    m_contextBar->setObjectName(QStringLiteral("ContextBar"));
    m_contextBar->setFixedHeight(26);

    auto* contextLayout = new QHBoxLayout(m_contextBar);
    contextLayout->setContentsMargins(12, 2, 12, 2);
    contextLayout->setSpacing(16);

    m_modelLabel = new QLabel(m_contextBar);
    m_modelLabel->setObjectName(QStringLiteral("ContextModelLabel"));
    QFont ctxFont = m_modelLabel->font();
    ctxFont.setPointSize(ctxFont.pointSize() - 1);
    m_modelLabel->setFont(ctxFont);
    contextLayout->addWidget(m_modelLabel);

    m_tokenLabel = new QLabel(m_contextBar);
    m_tokenLabel->setObjectName(QStringLiteral("ContextTokenLabel"));
    m_tokenLabel->setFont(ctxFont);
    contextLayout->addWidget(m_tokenLabel);

    m_stepLabel = new QLabel(m_contextBar);
    m_stepLabel->setObjectName(QStringLiteral("ContextStepLabel"));
    m_stepLabel->setFont(ctxFont);
    contextLayout->addWidget(m_stepLabel);

    contextLayout->addStretch();
    mainLayout->addWidget(m_contextBar);

    // -----------------------------------------------------------------------
    // Input frame — the main prompt area.
    // opencode: Framed with ┃ left border and ╹▀▀▀ bottom.
    // Qt6: Styled QFrame with heavy left border + bottom accent.
    // -----------------------------------------------------------------------
    m_inputFrame = new QFrame(this);
    m_inputFrame->setObjectName(QStringLiteral("InputFrame"));

    auto* inputLayout = new QHBoxLayout(m_inputFrame);
    inputLayout->setContentsMargins(12, 8, 8, 8);
    inputLayout->setSpacing(8);

    // Text editor — multi-line, grows from 1 to 6 lines.
    m_textEdit = new QPlainTextEdit(m_inputFrame);
    m_textEdit->setObjectName(QStringLiteral("InputTextEdit"));
    m_textEdit->setPlaceholderText(tr("Ask anything..."));
    m_textEdit->setFrameShape(QFrame::NoFrame);
    m_textEdit->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_textEdit->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

    // opencode design: The prompt text area uses monospace or system font,
    // sized to feel natural in a terminal context.
    QFont inputFont = m_textEdit->font();
    inputFont.setPointSize(inputFont.pointSize());
    m_textEdit->setFont(inputFont);

    // Capture Enter key for submit, allow Shift+Enter for newline.
    m_textEdit->installEventFilter(this);

    // Track text changes for height adjustment.
    connect(m_textEdit, &QPlainTextEdit::textChanged, this, [this]() {
        adjustHeight();
        emit textChanged();
    });

    // Calculate base single-line height.
    QFontMetrics fm(m_textEdit->font());
    m_minHeight = fm.lineSpacing() + 8; // line + some padding

    m_textEdit->setMinimumHeight(m_minHeight);
    m_textEdit->setMaximumHeight(m_minHeight * m_maxVisibleLines);

    inputLayout->addWidget(m_textEdit, 1);

    // Submit button — only visible when text is non-empty.
    m_submitBtn = new QPushButton(m_inputFrame);
    m_submitBtn->setObjectName(QStringLiteral("SubmitButton"));
    m_submitBtn->setText(tr("Send"));
    m_submitBtn->setFixedSize(56, 32);
    m_submitBtn->setEnabled(false);
    m_submitBtn->hide();
    connect(m_submitBtn, &QPushButton::clicked, this, &OpenCodeInputWidget::onSubmitClicked);

    // Show submit button only when there's text.
    connect(m_textEdit, &QPlainTextEdit::textChanged, this, [this]() {
        bool hasText = !m_textEdit->toPlainText().trimmed().isEmpty();
        m_submitBtn->setVisible(hasText && !m_agentBusy);
        m_submitBtn->setEnabled(hasText);
    });

    inputLayout->addWidget(m_submitBtn);

    // Stop/Interrupt button — visible when agent is busy processing.
    // opencode: Ctrl+C or Esc interrupts. We add a visual button.
    m_stopBtn = new QPushButton(m_inputFrame);
    m_stopBtn->setObjectName(QStringLiteral("StopButton"));
    m_stopBtn->setText(QStringLiteral("■"));  // Unicode stop square
    m_stopBtn->setFixedSize(32, 32);
    m_stopBtn->setToolTip(tr("Stop generating (Esc)"));
    m_stopBtn->hide();
    connect(m_stopBtn, &QPushButton::clicked, this, &OpenCodeInputWidget::onStopClicked);

    inputLayout->addWidget(m_stopBtn);

    mainLayout->addWidget(m_inputFrame);

    // Initial context.
    updateContextLabel();
}

// =============================================================================
// Public API
// =============================================================================

void OpenCodeInputWidget::setText(const QString& text)
{
    m_textEdit->setPlainText(text);
}

QString OpenCodeInputWidget::text() const
{
    return m_textEdit->toPlainText();
}

void OpenCodeInputWidget::clear()
{
    m_textEdit->clear();
}

void OpenCodeInputWidget::focusInput()
{
    m_textEdit->setFocus();
    // Move cursor to end.
    auto cursor = m_textEdit->textCursor();
    cursor.movePosition(QTextCursor::End);
    m_textEdit->setTextCursor(cursor);
}

void OpenCodeInputWidget::setAgentBusy(bool busy)
{
    m_agentBusy = busy;
    m_stopBtn->setVisible(busy);
    m_submitBtn->setVisible(!busy && !m_textEdit->toPlainText().trimmed().isEmpty());
    m_textEdit->setReadOnly(busy);

    if (busy) {
        m_textEdit->setStyleSheet(
            QStringLiteral("#InputTextEdit { color: #484f58; }"));
    } else {
        m_textEdit->setStyleSheet({});
    }
}

void OpenCodeInputWidget::setModelInfo(const QString& model)
{
    m_modelName = model;
    updateContextLabel();
}

void OpenCodeInputWidget::setTokenCount(int count)
{
    m_tokenCount = count;
    updateContextLabel();
}

void OpenCodeInputWidget::setStepCount(int count)
{
    m_stepCount = count;
    updateContextLabel();
}

void OpenCodeInputWidget::setContextInfo(const QString& info)
{
    m_modelLabel->setText(info);
}

void OpenCodeInputWidget::setInputEnabled(bool enabled)
{
    m_textEdit->setEnabled(enabled);
    m_submitBtn->setEnabled(enabled && !m_textEdit->toPlainText().trimmed().isEmpty());
}

void OpenCodeInputWidget::setWorkingDirectory(const QString& path)
{
    // Stored for future @-mention file completion.
    Q_UNUSED(path);
}

// =============================================================================
// Event handling
// =============================================================================

bool OpenCodeInputWidget::eventFilter(QObject* obj, QEvent* event)
{
    if (obj == m_textEdit && event->type() == QEvent::KeyPress) {
        auto* keyEvent = static_cast<QKeyEvent*>(event);

        // Enter key (without Shift) = Submit.
        // opencode: Enter submits. In opencode TUI, the editor is vim-like,
        // but for Qt we use standard Enter-to-send convention.
        if (keyEvent->key() == Qt::Key_Return || keyEvent->key() == Qt::Key_Enter) {
            if (!(keyEvent->modifiers() & Qt::ShiftModifier)) {
                // Submit if there's text and agent is not busy.
                QString t = m_textEdit->toPlainText().trimmed();
                if (!t.isEmpty() && !m_agentBusy) {
                    emit messageSubmitted(t);
                    clear();
                }
                return true; // Eat the event.
            }
            // Shift+Enter: allow newline (default behavior).
        }

        // Esc = Stop/Interrupt (opencode convention).
        if (keyEvent->key() == Qt::Key_Escape && m_agentBusy) {
            emit stopRequested();
            return true;
        }
    }
    return QWidget::eventFilter(obj, event);
}

// =============================================================================
// Slots
// =============================================================================

void OpenCodeInputWidget::onSubmitClicked()
{
    QString t = m_textEdit->toPlainText().trimmed();
    if (!t.isEmpty() && !m_agentBusy) {
        emit messageSubmitted(t);
        clear();
    }
}

void OpenCodeInputWidget::onStopClicked()
{
    emit stopRequested();
}

void OpenCodeInputWidget::adjustHeight()
{
    // Dynamically grow/shrink the text edit height based on content.
    // opencode: The textarea grows from 1 line up to 6 lines.
    int docHeight = static_cast<int>(m_textEdit->document()->size().height());
    int newHeight = qBound(m_minHeight, docHeight, m_minHeight * m_maxVisibleLines);
    m_textEdit->setFixedHeight(newHeight);
}

void OpenCodeInputWidget::updateContextLabel()
{
    QStringList parts;
    if (!m_modelName.isEmpty()) {
        parts << m_modelName;
    }
    if (m_tokenCount > 0) {
        if (m_tokenCount >= 1000) {
            parts << tr("%1k tokens").arg(m_tokenCount / 1000.0, 0, 'f', 1);
        } else {
            parts << tr("%1 tokens").arg(m_tokenCount);
        }
    }
    if (m_stepCount > 0) {
        parts << tr("Step %1").arg(m_stepCount);
    }

    m_modelLabel->setText(parts.join(QStringLiteral(" · ")));
}
