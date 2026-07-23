// =============================================================================
// opencode_dialog_overlay.h — Modal overlay system for opencode desktop v2
//
// Replicates opencode desktop v2's modal dialog patterns:
//   - Permission prompts (approve/deny tool calls)
//   - Command palette (Cmd+K / Ctrl+K)
//   - Model picker dialog (searchable, provider sections)
//   - Provider connect dialog
//
// opencode design: All dialogs use the same dark theme, with a centered
// card on a semi-transparent backdrop. Dialogs are non-resizable, have
// a title bar, content area, and action buttons.
// =============================================================================

#ifndef OPENCODE_DIALOG_OVERLAY_H
#define OPENCODE_DIALOG_OVERLAY_H

#include <QDialog>
#include <QVBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QLineEdit>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QCheckBox>
#include <QComboBox>

class QStackedWidget;

// ---------------------------------------------------------------------------
// Base dialog with opencode dark styling
// ---------------------------------------------------------------------------
class OpenCodeDialog : public QDialog
{
    Q_OBJECT
public:
    explicit OpenCodeDialog(const QString& title, QWidget* parent = nullptr);

protected:
    void setupTitleBar(const QString& title);
    QWidget* contentWidget() const { return m_content; }
    QVBoxLayout* contentLayout() const { return m_contentLayout; }

private:
    QWidget* m_content = nullptr;
    QVBoxLayout* m_contentLayout = nullptr;
};

// ---------------------------------------------------------------------------
// PermissionPrompt — Inline tool call approval/denial
//
// opencode design: When the agent wants to execute a tool, a permission
// dialog appears inline (or as a modal) showing the tool details and
// approve/deny buttons.
// ---------------------------------------------------------------------------
class PermissionPrompt : public OpenCodeDialog
{
    Q_OBJECT
public:
    explicit PermissionPrompt(QWidget* parent = nullptr);

    void setToolName(const QString& name);
    void setToolCommand(const QString& command);
    void setToolDescription(const QString& desc);
    void setAlwaysAllow(bool always);

    bool alwaysAllow() const;

signals:
    void approved();
    void denied();

private:
    QLabel* m_toolNameLabel = nullptr;
    QLabel* m_commandLabel = nullptr;
    QLabel* m_descLabel = nullptr;
    QCheckBox* m_alwaysCheck = nullptr;
};

// ---------------------------------------------------------------------------
// CommandPalette — Cmd+K command palette
//
// opencode design: Press Cmd+K (macOS) or Ctrl+K to open a searchable
// command palette with a list of available commands. Matches are
// highlighted, and commands can be executed by Enter key.
// ---------------------------------------------------------------------------
class CommandPalette : public OpenCodeDialog
{
    Q_OBJECT
public:
    explicit CommandPalette(QWidget* parent = nullptr);

    void addCommand(const QString& name, const QString& description,
                    const QString& shortcut = {});
    void clearCommands();

signals:
    void commandSelected(const QString& name);

private slots:
    void onFilterChanged(const QString& text);
    void onItemActivated(QListWidgetItem* item);

private:
    void setupUi();
    QLineEdit* m_searchEdit = nullptr;
    QListWidget* m_commandList = nullptr;
    QStringList m_allCommands;
};

// ---------------------------------------------------------------------------
// ModelPicker — Searchable model selector
//
// opencode design: The model picker is a searchable dropdown/dialog with
// model names grouped by provider. Each model shows its name and a short
// description. Providers are shown as collapsible sections.
// ---------------------------------------------------------------------------
class ModelPicker : public OpenCodeDialog
{
    Q_OBJECT
public:
    explicit ModelPicker(QWidget* parent = nullptr);

    // Add a provider section with models.
    void addProvider(const QString& providerName,
                     const QStringList& models,
                     const QStringList& descriptions = {});

    // Set the currently selected model.
    void setCurrentModel(const QString& provider, const QString& model);

signals:
    void modelSelected(const QString& provider, const QString& model);

private slots:
    void onFilterChanged(const QString& text);
    void onModelActivated(QListWidgetItem* item);

private:
    QLineEdit* m_searchEdit = nullptr;
    QListWidget* m_modelList = nullptr;
    struct ModelEntry {
        QString provider;
        QString model;
        QString description;
    };
    QList<ModelEntry> m_entries;
};

// ---------------------------------------------------------------------------
// ProviderConnectDialog — API key / provider setup
//
// opencode design: When connecting a new provider (e.g., DeepSeek, OpenAI,
// Groq), a dialog appears with fields for API key, base URL, and model list.
// ---------------------------------------------------------------------------
class ProviderConnectDialog : public OpenCodeDialog
{
    Q_OBJECT
public:
    explicit ProviderConnectDialog(QWidget* parent = nullptr);

    void setProviderName(const QString& name);

    QString apiKey() const;
    QString baseUrl() const;
    QString apiType() const;

signals:
    void connectRequested(const QString& provider,
                          const QString& apiKey,
                          const QString& baseUrl);

private:
    QLabel* m_providerLabel = nullptr;
    QLineEdit* m_apiKeyEdit = nullptr;
    QLineEdit* m_baseUrlEdit = nullptr;
    QComboBox* m_apiTypeCombo = nullptr;
};

#endif // OPENCODE_DIALOG_OVERLAY_H
