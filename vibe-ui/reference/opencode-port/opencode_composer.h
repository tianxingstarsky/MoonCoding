// =============================================================================
// opencode_composer.h — Composer input widget (openCode desktop v2)
//
// Replicates opencode desktop v2's composer component:
//   - Multi-line growing input (wraps OpenCodeInputWidget)
//   - Model selector button (opens ModelPicker dialog)
//   - Add menu button (files, commands, context, shell mode)
//   - Submit/Stop button
//   - Draft-per-tab state management
//   - Context bar: model name · tokens · steps
//   - @-mention autocomplete framework (stub for now)
//
// opencode design: The composer sits at the bottom of the session view,
// with a horizontal toolbar above it for model selection, add attachments,
// etc. The text area grows from 1 to 6 lines, then scrolls.
// =============================================================================

#ifndef OPENCODE_COMPOSER_H
#define OPENCODE_COMPOSER_H

#include <QWidget>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QLabel>
#include <QMenu>
#include <QStackedWidget>
#include <QMap>
#include <QVariantMap>

class OpenCodeInputWidget;
class ModelPicker;

// ---------------------------------------------------------------------------
// AddMenuButton — Dropdown menu for adding files/commands/context
// ---------------------------------------------------------------------------
class AddMenuButton : public QPushButton
{
    Q_OBJECT
public:
    explicit AddMenuButton(QWidget* parent = nullptr);

signals:
    void addFilesRequested();
    void addCommandRequested();
    void addContextRequested();
    void shellModeToggled(bool enabled);

private:
    QMenu* m_menu = nullptr;
};

// ---------------------------------------------------------------------------
// OpenCodeComposer — Full composer widget with toolbar + input
// ---------------------------------------------------------------------------
class OpenCodeComposer : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeComposer(QWidget* parent = nullptr);

    // Set/get the input text.
    void setText(const QString& text);
    QString text() const;

    // Clear the input area.
    void clear();

    // Focus the text editor.
    void focusInput();

    // Agent state control.
    void setAgentBusy(bool busy);
    bool isAgentBusy() const;

    // Model info for display and picker.
    void setModelInfo(const QString& provider, const QString& model);
    void setTokenCount(int count);
    void setStepCount(int count);

    // Add available models for the picker.
    void addAvailableModel(const QString& provider, const QString& model,
                           const QString& description = {});

    // Draft-per-tab state: save/restore composer state.
    void saveDraft(QVariantMap& draft) const;
    void restoreDraft(const QVariantMap& draft);

    // Enable/disable input.
    void setInputEnabled(bool enabled);

signals:
    // Emitted when user submits text.
    void messageSubmitted(const QString& text);

    // Emitted when user presses the Stop/Interrupt button.
    void stopRequested();

    // Emitted when input text changes.
    void textChanged();

    // Model picker signals.
    void modelChanged(const QString& provider, const QString& model);

    // Add menu signals.
    void addFilesRequested();
    void addCommandRequested();
    void addContextRequested();

private slots:
    void onModelButtonClicked();
    void onModelSelected(const QString& provider, const QString& model);
    void onSubmitClicked();
    void onStopClicked();

private:
    void setupUi();
    void updateModelButtonLabel();
    void updateContextLabel();

    // Internal input widget (existing component).
    OpenCodeInputWidget* m_inputWidget = nullptr;

    // Toolbar buttons.
    QPushButton* m_modelBtn = nullptr;       // Model selector
    AddMenuButton* m_addBtn = nullptr;        // Add files/commands/context
    QPushButton* m_submitBtn = nullptr;       // Submit
    QPushButton* m_stopBtn = nullptr;         // Stop

    // Context bar widgets.
    QWidget* m_contextBar = nullptr;
    QLabel* m_contextModelLabel = nullptr;
    QLabel* m_contextTokenLabel = nullptr;
    QLabel* m_contextStepLabel = nullptr;

    // State.
    QString m_currentProvider;
    QString m_currentModel;
    int m_tokenCount = 0;
    int m_stepCount = 0;
    bool m_agentBusy = false;

    // Model picker reference.
    ModelPicker* m_modelPicker = nullptr;

    // Available models for picker.
    struct ModelInfo {
        QString provider;
        QString model;
        QString description;
    };
    QList<ModelInfo> m_availableModels;
};

#endif // OPENCODE_COMPOSER_H
