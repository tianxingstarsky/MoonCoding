// =============================================================================
// opencode_tool_block.cpp — Collapsible tool call display implementation
// =============================================================================

#include "opencode_tool_block.h"

#include <QFont>
#include <QPropertyAnimation>
#include <QGraphicsOpacityEffect>

// =============================================================================
// Construction
// =============================================================================

OpenCodeToolBlock::OpenCodeToolBlock(QWidget* parent)
    : QFrame(parent)
{
    setupUi();
    applyStateStyle();
}

void OpenCodeToolBlock::setupUi()
{
    setObjectName(QStringLiteral("ToolBlock"));
    // opencode design: Tool cards use 1px border with rounded corners (4px).
    // The card has a slightly elevated background vs the chat surface.
    setFrameShape(QFrame::StyledPanel);

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // -----------------------------------------------------------------------
    // Header — always visible. Contains: indicator | tool name | command | time | toggle
    // opencode pattern: ╭─ ✓ bash ─────────────────────────────────────╮
    // -----------------------------------------------------------------------
    m_headerFrame = new QFrame(this);
    m_headerFrame->setObjectName(QStringLiteral("ToolBlockHeader"));
    m_headerFrame->setCursor(Qt::PointingHandCursor);
    m_headerFrame->installEventFilter(this); // For click-to-toggle

    auto* headerLayout = new QHBoxLayout(m_headerFrame);
    headerLayout->setContentsMargins(12, 8, 8, 8);
    headerLayout->setSpacing(6);

    // Indicator: Unicode symbol for state.
    m_indicatorLabel = new QLabel(QStringLiteral("⬤"), m_headerFrame);
    m_indicatorLabel->setObjectName(QStringLiteral("ToolIndicator"));
    QFont indicatorFont(QStringLiteral("Segoe UI Symbol"), 12);
    m_indicatorLabel->setFont(indicatorFont);
    m_indicatorLabel->setFixedWidth(18);
    m_indicatorLabel->setAlignment(Qt::AlignCenter);
    headerLayout->addWidget(m_indicatorLabel);

    // Tool name (e.g., "bash", "read").
    m_toolNameLabel = new QLabel(m_headerFrame);
    m_toolNameLabel->setObjectName(QStringLiteral("ToolName"));
    QFont nameFont = m_toolNameLabel->font();
    nameFont.setBold(true);
    nameFont.setPointSize(nameFont.pointSize());
    m_toolNameLabel->setFont(nameFont);
    headerLayout->addWidget(m_toolNameLabel);

    // Command preview (e.g., "$ echo 'hello'").
    m_commandLabel = new QLabel(m_headerFrame);
    m_commandLabel->setObjectName(QStringLiteral("ToolCommand"));
    QFont cmdFont = m_commandLabel->font();
    cmdFont.setFamily(QStringLiteral("Consolas"));
    cmdFont.setPointSize(cmdFont.pointSize() - 1);
    m_commandLabel->setFont(cmdFont);
    m_commandLabel->setTextFormat(Qt::PlainText);
    m_commandLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    headerLayout->addWidget(m_commandLabel, 1);

    // Execution time (e.g., "2.3s").
    m_timeLabel = new QLabel(m_headerFrame);
    m_timeLabel->setObjectName(QStringLiteral("ToolTime"));
    QFont timeFont = m_timeLabel->font();
    timeFont.setPointSize(timeFont.pointSize() - 1);
    timeFont.setFamily(QStringLiteral("Consolas"));
    m_timeLabel->setFont(timeFont);
    headerLayout->addWidget(m_timeLabel);

    // Retry button (shown on failure).
    m_retryBtn = new QPushButton(QStringLiteral("↻"), m_headerFrame);
    m_retryBtn->setObjectName(QStringLiteral("ToolRetryButton"));
    m_retryBtn->setFixedSize(22, 22);
    m_retryBtn->setToolTip(tr("Retry"));
    m_retryBtn->hide();
    headerLayout->addWidget(m_retryBtn);
    connect(m_retryBtn, &QPushButton::clicked, this, &OpenCodeToolBlock::onRetryClicked);

    // Toggle button (▼ collapsed, ▲ expanded).
    m_toggleBtn = new QPushButton(QStringLiteral("▼"), m_headerFrame);
    m_toggleBtn->setObjectName(QStringLiteral("ToolToggleButton"));
    m_toggleBtn->setFixedSize(22, 22);
    m_toggleBtn->setFlat(true);
    m_toggleBtn->setCursor(Qt::PointingHandCursor);
    headerLayout->addWidget(m_toggleBtn);
    connect(m_toggleBtn, &QPushButton::clicked, this, &OpenCodeToolBlock::onToggleClicked);

    mainLayout->addWidget(m_headerFrame);

    // -----------------------------------------------------------------------
    // Content — collapsible output area.
    // opencode separator: ├───────────────────────────────────────────
    // -----------------------------------------------------------------------
    m_contentFrame = new QFrame(this);
    m_contentFrame->setObjectName(QStringLiteral("ToolBlockContent"));

    auto* contentLayout = new QVBoxLayout(m_contentFrame);
    contentLayout->setContentsMargins(12, 8, 12, 12);
    contentLayout->setSpacing(0);

    // Separator line (replaces opencode's ├── Unicode separator).
    auto* separator = new QFrame(m_contentFrame);
    separator->setObjectName(QStringLiteral("ToolSeparator"));
    separator->setFixedHeight(1);
    separator->setFrameShape(QFrame::HLine);
    contentLayout->addWidget(separator);
    contentLayout->addSpacing(6);

    // Output text area (monospace, read-only).
    m_outputEdit = new QTextEdit(m_contentFrame);
    m_outputEdit->setObjectName(QStringLiteral("ToolOutput"));
    m_outputEdit->setReadOnly(true);
    m_outputEdit->setFrameShape(QFrame::NoFrame);
    m_outputEdit->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_outputEdit->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    // opencode design: Monospace output with fixed line height.
    QFont outputFont(QStringLiteral("Consolas"), 11);
    outputFont.setStyleHint(QFont::Monospace);
    m_outputEdit->setFont(outputFont);
    m_outputEdit->setMinimumHeight(40);
    m_outputEdit->setMaximumHeight(300);
    contentLayout->addWidget(m_outputEdit);

    m_contentFrame->setVisible(false);
    mainLayout->addWidget(m_contentFrame);

    // -----------------------------------------------------------------------
    // Spinner timer for running state animation.
    // -----------------------------------------------------------------------
    m_spinnerTimer = new QTimer(this);
    m_spinnerTimer->setInterval(120); // ~8fps, smooth enough for terminal feel
    connect(m_spinnerTimer, &QTimer::timeout, this, &OpenCodeToolBlock::updateSpinner);
}

// =============================================================================
// Public API
// =============================================================================

void OpenCodeToolBlock::setToolType(ToolType type)
{
    m_toolType = type;
    // opencode Unicode direction prefixes.
    switch (type) {
    case Read:
    case Grep:
    case Glob:
    case Lsp:
        m_indicatorLabel->setText(QStringLiteral("→"));
        break;
    case Write:
        m_indicatorLabel->setText(QStringLiteral("←"));
        break;
    case Bash:
        m_indicatorLabel->setText(QStringLiteral("$"));
        break;
    default:
        m_indicatorLabel->setText(QStringLiteral("⬤"));
        break;
    }
}

void OpenCodeToolBlock::setToolName(const QString& name)
{
    m_toolNameLabel->setText(name);
}

void OpenCodeToolBlock::setCommand(const QString& command)
{
    // Truncate long commands — opencode style.
    QString display = command;
    if (display.length() > 80) {
        display = display.left(77) + QStringLiteral("...");
    }
    m_commandLabel->setText(display);
    m_commandLabel->setToolTip(command); // Full command on hover.
}

void OpenCodeToolBlock::setState(State state)
{
    State oldState = m_state;
    m_state = state;

    switch (state) {
    case Pending:
        m_spinnerTimer->stop();
        break;
    case Running:
        m_startTime.start();
        m_spinnerTimer->start();
        break;
    case Success:
    case Failed:
    case Cancelled:
        m_spinnerTimer->stop();
        m_elapsedMs = m_startTime.elapsed();
        setExecutionTimeMs(m_elapsedMs);
        break;
    }
    applyStateStyle();
    updateIndicator();

    // Auto-expand on failure (opencode pattern: show errors inline).
    if (state == Failed && !m_expanded) {
        setExpanded(true);
    }
}

void OpenCodeToolBlock::setOutput(const QString& output)
{
    m_outputEdit->setPlainText(output);
}

void OpenCodeToolBlock::appendOutput(const QString& chunk)
{
    m_outputEdit->moveCursor(QTextCursor::End);
    m_outputEdit->insertPlainText(chunk);
    // Auto-scroll output to bottom.
    m_outputEdit->moveCursor(QTextCursor::End);
    m_outputEdit->ensureCursorVisible();
}

void OpenCodeToolBlock::setExecutionTime(const QString& timeStr)
{
    m_timeLabel->setText(timeStr);
}

void OpenCodeToolBlock::setExecutionTimeMs(qint64 ms)
{
    if (ms < 1000) {
        m_timeLabel->setText(QStringLiteral("%1ms").arg(ms));
    } else if (ms < 60000) {
        m_timeLabel->setText(QStringLiteral("%1s").arg(ms / 1000.0, 0, 'f', 1));
    } else {
        int secs = static_cast<int>(ms / 1000);
        m_timeLabel->setText(QStringLiteral("%1m %2s").arg(secs / 60).arg(secs % 60));
    }
}

void OpenCodeToolBlock::setCompletionInfo(const QString& info)
{
    // opencode: "▣ ToolName · model · time"
    m_toolNameLabel->setText(info);
}

void OpenCodeToolBlock::setExpanded(bool expanded)
{
    if (m_expanded == expanded) return;
    m_expanded = expanded;

    m_toggleBtn->setText(m_expanded ? QStringLiteral("\u25B2") : QStringLiteral("\u25BC"));

    // opencode: No animations in terminal TUI — instant expand/collapse.
    // MoonCoding adaptation: Smooth height transition using QPropertyAnimation.
    // The standard Qt collapsible pattern: animate maximumHeight between 0 and content height.
    if (m_expanded) {
        m_contentFrame->setVisible(true);
        m_contentFrame->setMaximumHeight(0);
        // Use output content height instead of sizeHint (which may be 0 before first show)
        int targetHeight = qMax(80, m_outputEdit->document()->size().toSize().height() + 60);
        auto* anim = new QPropertyAnimation(m_contentFrame, "maximumHeight", this);
        anim->setDuration(200);
        anim->setStartValue(0);
        anim->setEndValue(targetHeight);
        anim->setEasingCurve(QEasingCurve::InOutCubic);
        connect(anim, &QPropertyAnimation::finished, this, [this]() {
            m_contentFrame->setMaximumHeight(QWIDGETSIZE_MAX);
            emit this->expanded();
        });
        anim->start(QAbstractAnimation::DeleteWhenStopped);
    } else {
        int startHeight = m_contentFrame->height();
        auto* anim = new QPropertyAnimation(m_contentFrame, "maximumHeight", this);
        anim->setDuration(200);
        anim->setStartValue(startHeight);
        anim->setEndValue(0);
        anim->setEasingCurve(QEasingCurve::InOutCubic);
        connect(anim, &QPropertyAnimation::finished, this, [this]() {
            m_contentFrame->setVisible(false);
            emit this->collapsed();
        });
        anim->start(QAbstractAnimation::DeleteWhenStopped);
    }
}

void OpenCodeToolBlock::toggleExpanded()
{
    setExpanded(!m_expanded);
}

// =============================================================================
// Private helpers
// =============================================================================

void OpenCodeToolBlock::applyStateStyle()
{
    // opencode design: Color-coded states.
    // Running = blue/accent, Success = green, Failed = red, Cancelled = gray.
    const char* borderColor = "#30363d";     // Default muted border
    const char* headerBg = "#161b22";        // Header background
    const char* accentColor = "#58a6ff";     // Blue accent

    switch (m_state) {
    case Running:
        borderColor = "#58a6ff";  // Phosphor-blue (opencode accent)
        headerBg = "#0d2847";
        break;
    case Success:
        borderColor = "#3fb950";  // Phosphor-green (opencode success)
        headerBg = "#0d2818";
        break;
    case Failed:
        borderColor = "#f85149";  // Red/critical
        headerBg = "#280d0d";
        break;
    case Cancelled:
        borderColor = "#8b949e";  // Muted gray
        headerBg = "#161b22";
        break;
    case Pending:
        borderColor = "#30363d";
        headerBg = "#161b22";
        break;
    }

    setStyleSheet(QStringLiteral(
        "#ToolBlock {"
        "  border: 1px solid %1;"
        "  border-radius: 6px;"
        "  background-color: #0d1117;"
        "}"
        "#ToolBlockHeader {"
        "  background-color: %2;"
        "  border-top-left-radius: 5px;"
        "  border-top-right-radius: 5px;"
        "}"
        "#ToolBlockContent {"
        "  background-color: #0d1117;"
        "  border-bottom-left-radius: 5px;"
        "  border-bottom-right-radius: 5px;"
        "}"
    ).arg(QString::fromLatin1(borderColor),
          QString::fromLatin1(headerBg)));

    // Show/hide retry button.
    m_retryBtn->setVisible(m_state == Failed);
}

void OpenCodeToolBlock::updateIndicator()
{
    // opencode design: Unicode indicators in the card header.
    switch (m_state) {
    case Running:
        // Spinner is handled by updateSpinner().
        m_indicatorLabel->setStyleSheet(QStringLiteral("color: #58a6ff;"));
        break;
    case Success:
        m_indicatorLabel->setText(QStringLiteral("✓"));
        m_indicatorLabel->setStyleSheet(
            QStringLiteral("color: #3fb950; font-weight: bold;"));
        break;
    case Failed:
        m_indicatorLabel->setText(QStringLiteral("✗"));
        m_indicatorLabel->setStyleSheet(
            QStringLiteral("color: #f85149; font-weight: bold;"));
        break;
    case Cancelled:
        m_indicatorLabel->setText(QStringLiteral("⊘"));
        m_indicatorLabel->setStyleSheet(QStringLiteral("color: #8b949e;"));
        break;
    case Pending:
        m_indicatorLabel->setText(QStringLiteral("○"));
        m_indicatorLabel->setStyleSheet(QStringLiteral("color: #484f58;"));
        break;
    }
}

void OpenCodeToolBlock::updateSpinner()
{
    // opencode: Animated spinner using Unicode characters.
    // Cycle through ◜ ◠ ◝ ◞ ◡ ◟
    m_spinnerIndex = (m_spinnerIndex + 1) % SPINNER_FRAME_COUNT;
    m_indicatorLabel->setText(
        QString::fromUtf8(SPINNER_FRAMES[m_spinnerIndex]));
    m_indicatorLabel->setStyleSheet(QStringLiteral("color: #58a6ff; font-weight: bold;"));

    // Update elapsed time while running.
    m_elapsedMs = m_startTime.elapsed();
    setExecutionTimeMs(m_elapsedMs);
}

// =============================================================================
// Slots
// =============================================================================

void OpenCodeToolBlock::onToggleClicked()
{
    toggleExpanded();
}

void OpenCodeToolBlock::onRetryClicked()
{
    emit retryRequested();
}

void OpenCodeToolBlock::setContentHeight(int h)
{
    m_contentHeight = h;
}
