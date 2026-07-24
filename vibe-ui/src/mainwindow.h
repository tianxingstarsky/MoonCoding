#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QMainWindow>

class AppsWidget;
class AppRunner;
class BoardImeController;
class BoardNetRecover;
class ChatWidget;
class InputWidget;
class ModelFetcher;
class QLabel;
class QLineEdit;
class QListWidget;
class QListWidgetItem;
class QPushButton;
class QSplitter;
class QStackedWidget;
class QTimer;
class QToolButton;
class RustBridge;
class TreeWidget;
class WifiPanel;

class MainWindow final : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(const QString &workspace, QWidget *parent = nullptr);

protected:
    void closeEvent(QCloseEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void submitMessage(const QString &message);
    void showSettings();
    void showWifiPage();
    void showModelsPage();
    void toggleTheme();
    void updateTokenStatus(quint64 tokensIn, quint64 tokensOut, quint64 steps);
    void showProjectMenu();
    void showNewProjectPage();
    void showOpenProjectPage();
    void createProjectHere(const QString &name);
    void seedNewProjectFiles(const QString &projectPath);
    void switchProject(const QString &workspace);
    void deleteProject(const QString &workspace);
    void deleteCurrentProject();
    void newConversation();
    void showChatPage();
    void showTreePage();
    void showAppsPage();
    void toggleHistoryPanel();
    void onHistoryItemActivated(QListWidgetItem *item);
    void refreshNetworkStatus();

private:
    enum PageIndex {
        ChatPage = 0,
        TreePage,
        AppsPage,
        NewProjectPage,
        OpenProjectPage,
        SettingsPage,
        WifiPage,
        ModelsPage
    };

    QJsonObject loadBackendOptions() const;
    QString projectsRoot() const;
    void applyTheme(bool light);
    void connectSignals();
    void switchWorkspace(const QString &workspace);
    void switchSession(const QString &sessionId);
    void updateRecentProjects(const QString &workspace);
    void removeFromRecentProjects(const QString &workspace);
    bool isUnderProjectsRoot(const QString &workspace) const;
    bool hasAnyProjects() const;
    QString fallbackProjectAfterDelete(const QString &deletedPath) const;
    void enterNoProjectState();
    void populateHistory(const QJsonArray &sessions);
    void updateActiveNodeBanner(const QJsonObject &tree);
    void applyResponsiveLayout();
    void updateNavStates();
    void persistSessionId() const;
    void reparentTreeToSidePanel();
    void reparentTreeToFullPage();
    void retranslateUi();
    void goToPage(PageIndex idx);
    void showFlashMessage(const QString &msg, int durationMs);
    void enableBoardTouchScroll();
    /// Apply current settings font, then ask to keep it within 10s; revert on cancel/timeout.
    bool confirmFontPreviewOrRevert(int previousSize, const QString &previousFamily);
    void confirmResetAllSettings();
    int configuredContextWindowK() const;

    QWidget *buildNewProjectPage();
    QWidget *buildOpenProjectPage();
    QWidget *buildSettingsPage();
    QWidget *buildWifiPage();
    QWidget *buildModelsPage();
    void refreshModelsPage();
    void updateSettingsModelButton();

    QString m_workspace;
    QString m_sessionId;
    RustBridge *m_bridge;
    ChatWidget *m_chat;
    TreeWidget *m_tree;
    AppsWidget *m_apps = nullptr;
    AppRunner *m_appRunner = nullptr;
    InputWidget *m_input;
    BoardImeController *m_ime = nullptr;
    ModelFetcher *m_modelFetcher = nullptr;
    WifiPanel *m_wifiPanel = nullptr;
    BoardNetRecover *m_netRecover = nullptr;
    QPushButton *m_settingsModelBtn = nullptr;
    QLabel *m_modelsStatusLabel = nullptr;
    QListWidget *m_modelsList = nullptr;
    QPushButton *m_modelsRefreshBtn = nullptr;
    QStackedWidget *m_pages;
    QWidget *m_chatPage;
    QWidget *m_treePage;
    QWidget *m_treeSideHost;
    QSplitter *m_chatSplitter;
    QWidget *m_historyPanel;
    QListWidget *m_historyList;
    QLineEdit *m_historySearch;
    QToolButton *m_projectButton;
    QPushButton *m_chatNav;
    QPushButton *m_treeNav;
    QPushButton *m_appsNav;
    QPushButton *m_historyNav;
    QLabel *m_activeNodeLabel;
    QLabel *m_connectionLabel;
    QLabel *m_tokenLabel;
    QLabel *m_networkLabel = nullptr;
    qint64 m_networkPressAt = 0;
    QLabel *m_flashBanner;
    QTimer *m_flashTimer;
    QTimer *m_networkTimer = nullptr;
    qint64 m_lastAutoNetHealAt = 0;
    QWidget *m_mainHeader;
    QWidget *m_subHeader;
    QLabel *m_subHeaderTitle;
    bool m_lightTheme = false;
    bool m_closing = false;
    bool m_historyVisible = false;
    bool m_treePanelVisible = false;
    int m_currentPage = 0;
};
