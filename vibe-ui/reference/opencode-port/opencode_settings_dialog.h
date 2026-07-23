// =============================================================================
// opencode_settings_dialog.h — Settings dialog with 4 tabs
//
// Replicates opencode desktop v2's settings panel:
//   - General tab: layout mode (v2/legacy), theme selector, language
//   - Shortcuts tab: keybinding configuration table
//   - Providers tab: API key management, provider list, connect/disconnect
//   - Models tab: searchable model list grouped by provider
//
// opencode design: Settings is a modal dialog with a tab bar on the left
// (or top) and content area on the right. Each tab is a self-contained
// form with opencode dark styling.
// =============================================================================

#ifndef OPENCODE_SETTINGS_DIALOG_H
#define OPENCODE_SETTINGS_DIALOG_H

#include <QDialog>
#include <QTabWidget>
#include <QVBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QLineEdit>
#include <QListWidget>
#include <QComboBox>
#include <QCheckBox>
#include <QTableWidget>
#include <QGroupBox>

class QStackedWidget;

// ---------------------------------------------------------------------------
// OpenCodeSettingsDialog — Settings window
// ---------------------------------------------------------------------------
class OpenCodeSettingsDialog : public QDialog
{
    Q_OBJECT
public:
    explicit OpenCodeSettingsDialog(QWidget* parent = nullptr);

    // General tab values.
    QString layoutMode() const;       // "v2" or "legacy"
    QString theme() const;
    QString language() const;

    // Set current values.
    void setLayoutMode(const QString& mode);
    void setTheme(const QString& theme);
    void setLanguage(const QString& lang);

    // Provider management.
    void addProvider(const QString& name, const QString& baseUrl,
                     bool connected = false);
    void clearProviders();

    // Model management.
    void addModel(const QString& provider, const QString& model);
    void clearModels();

signals:
    void layoutModeChanged(const QString& mode);
    void themeChanged(const QString& theme);
    void languageChanged(const QString& lang);
    void providerConnectRequested(const QString& provider);
    void providerDisconnectRequested(const QString& provider);
    void settingsApplied();

private slots:
    void onApply();
    void onCancel();
    void onProviderDoubleClicked(QListWidgetItem* item);

private:
    void setupUi();
    QWidget* createGeneralTab();
    QWidget* createShortcutsTab();
    QWidget* createProvidersTab();
    QWidget* createModelsTab();

    // Tab widget.
    QTabWidget* m_tabWidget = nullptr;

    // General tab widgets.
    QComboBox* m_layoutCombo = nullptr;
    QComboBox* m_themeCombo = nullptr;
    QComboBox* m_languageCombo = nullptr;

    // Shortcuts tab.
    QTableWidget* m_shortcutsTable = nullptr;

    // Providers tab.
    QListWidget* m_providerList = nullptr;
    QPushButton* m_addProviderBtn = nullptr;
    QPushButton* m_removeProviderBtn = nullptr;

    // Models tab.
    QLineEdit* m_modelSearch = nullptr;
    QListWidget* m_modelList = nullptr;

    // Buttons.
    QPushButton* m_applyBtn = nullptr;
    QPushButton* m_cancelBtn = nullptr;
};

#endif // OPENCODE_SETTINGS_DIALOG_H
