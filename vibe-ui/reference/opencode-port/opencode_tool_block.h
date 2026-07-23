// =============================================================================
// opencode_tool_block.h — Collapsible tool call display widget
//
// Replicates opencode TUI's bordered tool card pattern:
//   ╭─ ✓ bash ──────────────────────────────────────────────╮
//   │ $ echo 'hello' > /tmp/test.txt                         │
//   ├──────────────────────────────────────────────────────── │
//   │ (no output)                                            │
//   ╰────────────────────────────────────────────────────────╯
//
// opencode design: Tool calls are rendered as bordered cards inline in
// the conversation. Uses Unicode box-drawing characters for structure.
// Color-coded: green for success, red for failure, blue for running.
//
// Qt6 translation: QFrame with styled borders + collapsible content area.
// Unicode box-drawing is approximated with QSS border styles since we're
// in a GUI, not a terminal.
//
// MoonCoding adaptation: Expands the collapsible pattern with animated
// transitions that wouldn't be possible in a pure terminal TUI.
// =============================================================================

#ifndef OPENCODE_TOOL_BLOCK_H
#define OPENCODE_TOOL_BLOCK_H

#include <QFrame>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QTextEdit>
#include <QPushButton>
#include <QTimer>
#include <QElapsedTimer>
#include <QPropertyAnimation>

class OpenCodeToolBlock : public QFrame
{
    Q_OBJECT
    Q_PROPERTY(int contentHeight READ contentHeight WRITE setContentHeight)

public:
    enum ToolType {
        Bash,        // → Shell command (read/write)
        Read,        // → File read
        Write,       // ← File write
        Grep,        // → Search
        Glob,        // → File find
        Lsp,         // → Language server
        Other
    };

    enum State {
        Pending,     // Not started yet
        Running,     // Executing (spinner animation)
        Success,     // Completed successfully ✓
        Failed,      // Failed ✗
        Cancelled    // User interrupted
    };

    explicit OpenCodeToolBlock(QWidget* parent = nullptr);

    // opencode: Tool call prefix is → for read, ← for write operations.
    void setToolType(ToolType type);

    // Set the tool name (e.g., "bash", "read", "write").
    void setToolName(const QString& name);

    // Set the command/arguments preview (e.g., "$ echo 'hello'").
    void setCommand(const QString& command);

    // Set execution state (updates colors and indicators).
    void setState(State state);

    // Set the output text (shown in collapsible area).
    void setOutput(const QString& output);

    // Append output (for streaming tool output).
    void appendOutput(const QString& chunk);

    // Set execution time display.
    void setExecutionTime(const QString& timeStr);
    void setExecutionTimeMs(qint64 ms);

    // opencode completion marker: "▣ ToolName · model · time"
    void setCompletionInfo(const QString& info);

    // Expand/collapse the output area.
    void setExpanded(bool expanded);
    bool isExpanded() const { return m_expanded; }
    void toggleExpanded();

    // Animation helpers.
    int contentHeight() const { return m_contentHeight; }
    void setContentHeight(int h);

signals:
    void expanded();
    void collapsed();
    void retryRequested();

private slots:
    void onToggleClicked();
    void onRetryClicked();
    void updateSpinner();

private:
    void setupUi();
    void applyStateStyle();
    void updateIndicator();

    // Header row widgets.
    QFrame* m_headerFrame = nullptr;
    QLabel* m_indicatorLabel = nullptr;      // ✓ / ✗ / ⬤ (spinner)
    QLabel* m_toolNameLabel = nullptr;       // "bash", "read", etc.
    QLabel* m_commandLabel = nullptr;        // "$ echo 'hello'"
    QLabel* m_timeLabel = nullptr;           // "2.3s"
    QPushButton* m_toggleBtn = nullptr;      // ▼/▲ expand/collapse
    QPushButton* m_retryBtn = nullptr;       // ↻ retry

    // Collapsible content area.
    QFrame* m_contentFrame = nullptr;
    QTextEdit* m_outputEdit = nullptr;       // Monospace output display

    // State.
    ToolType m_toolType = Other;
    State m_state = Pending;
    bool m_expanded = false;
    int m_contentHeight = 0;

    // Spinner animation.
    QTimer* m_spinnerTimer = nullptr;
    int m_spinnerIndex = 0;
    // opencode spinner frames (Unicode spinner chars).
    static constexpr const char* SPINNER_FRAMES[] = {"◜", "◠", "◝", "◞", "◡", "◟"};
    static constexpr int SPINNER_FRAME_COUNT = 6;

    // Timing.
    QElapsedTimer m_startTime;
    qint64 m_elapsedMs = 0;
};

#endif // OPENCODE_TOOL_BLOCK_H
