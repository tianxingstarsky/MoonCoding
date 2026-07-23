// =============================================================================
// opencode_input_widget.h — Chat input widget
//
// Replicates opencode TUI's prompt/input area:
//   - Multi-line growing text input (1→6 lines, then scrolls)
//   - Submit on Enter (Shift+Enter for newline)
//   - Interrupt/Stop button (shown when agent is busy)
//   - Context info bar: model name, token count, step count
//   - Placeholder: "Ask anything..."
//   - Framed with ┃ left border (opencode terminal aesthetic)
//
// opencode design: The input area sits at the bottom of the screen,
// framed by ┃ (heavy vertical) on the left and ╹▀▀▀ on the bottom.
// Model info is shown inline: "Build Claude Sonnet 4.5"
//
// Qt6 translation: QFrame with styled border + QPlainTextEdit with
// dynamic height. The ┃ frame is emulated via QSS left border styling.
//
// MoonCoding adaptation: Uses Qt's text editing primitives instead of
// Bubble Tea's textarea component. Adds placeholder text and modern
// button interactions.
// =============================================================================

#ifndef OPENCODE_INPUT_WIDGET_H
#define OPENCODE_INPUT_WIDGET_H

#include <QWidget>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QLabel>
#include <QTimer>

class OpenCodeInputWidget : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeInputWidget(QWidget* parent = nullptr);

    // Set/get the input text.
    void setText(const QString& text);
    QString text() const;

    // Clear the input area.
    void clear();

    // Focus the text editor.
    void focusInput();

    // Agent state control (shows/hides Stop button).
    void setAgentBusy(bool busy);
    bool isAgentBusy() const { return m_agentBusy; }

    // Context info.
    void setModelInfo(const QString& model);
    void setTokenCount(int count);
    void setStepCount(int count);
    void setContextInfo(const QString& info);

    // Enable/disable input.
    void setInputEnabled(bool enabled);

    // File path for @ mentions (future: file completion popup).
    void setWorkingDirectory(const QString& path);

signals:
    // Emitted when user submits text (Enter key).
    void messageSubmitted(const QString& text);

    // Emitted when user presses the Stop/Interrupt button.
    void stopRequested();

    // Emitted when input text changes.
    void textChanged();

protected:
    bool eventFilter(QObject* obj, QEvent* event) override;

private slots:
    void onSubmitClicked();
    void onStopClicked();
    void adjustHeight();

private:
    void setupUi();
    void updateContextLabel();

    // Context bar widgets.
    QFrame* m_contextBar = nullptr;
    QLabel* m_modelLabel = nullptr;
    QLabel* m_tokenLabel = nullptr;
    QLabel* m_stepLabel = nullptr;

    // Input area.
    QFrame* m_inputFrame = nullptr;
    QPlainTextEdit* m_textEdit = nullptr;
    QPushButton* m_submitBtn = nullptr;      // Submit/send button
    QPushButton* m_stopBtn = nullptr;        // Interrupt/stop button

    // State.
    bool m_agentBusy = false;
    QString m_modelName;
    int m_tokenCount = 0;
    int m_stepCount = 0;
    int m_maxVisibleLines = 6;               // opencode: grows up to 6 lines
    int m_minHeight = 0;                     // Base single-line height
};

#endif // OPENCODE_INPUT_WIDGET_H
