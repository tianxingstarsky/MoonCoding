#pragma once

#include <QJsonObject>
#include <QProcess>
#include <QStringList>
#include <QUrl>
#include <QWidget>

class QLabel;
class QListWidget;
class QListWidgetItem;
class QPlainTextEdit;
class QPushButton;
class QStackedWidget;
class QTextBrowser;
class QToolButton;

#ifdef HAS_QT_WEBENGINE
class QWebEngineView;
#endif

class RustBridge;

/// Single-project app surface: preview workspace/index.html + browse project files.
/// Multi-app sidebar is retired (one program per project).
///
/// Preview backend (`backend.py`) is project-scoped: auto-started on preview,
/// may keep running while switching Chat/Tree; destroyed on workspace switch / stop.
class AppsWidget final : public QWidget
{
    Q_OBJECT

public:
    explicit AppsWidget(RustBridge *bridge, QWidget *parent = nullptr);
    ~AppsWidget() override;

    void setWorkspace(const QString &workspace);
    void refresh();

    /// Call once before QApplication when WebEngine may be used (board/desktop).
    static void prepareWebEngineEnvironment();

    /// Handle mooncoding://backend/start|stop from preview links (manual override).
    void handleMooncodingUrl(const QUrl &url);

signals:
    void runCliApp(const QString &appName, const QString &command);

private slots:
    void reloadPreview();
    void showFilesPane();
    void showPreviewPane();
    void onFileActivated(QListWidgetItem *item);
    void onAnchorClicked(const QUrl &url);
    void stopBackend();

private:
    void buildUi();
    void loadIndexHtml();
    void populateFileList();
    void startBackend();
    void ensureBackendRunning();
    void updateBackendButton();
    void writeBackendLease(qint64 pid, quint16 port);
    void clearBackendLease();
    bool adoptRunningLease();
    void killLeasePidIfAny();
    void injectApiBase();
    quint16 backendPort() const;
    QString backendApiBase() const;
    QString backendScriptPath() const;
    QString backendLeasePath() const;
    bool hasBackendScript() const;
    QString languageForPath(const QString &path) const;
    QString previewModeLabel() const;

    RustBridge *m_bridge = nullptr;
    QString m_workspace;
    quint16 m_backendPort = 0;

    QStackedWidget *m_stack = nullptr;
    QWidget *m_previewPage = nullptr;
    QWidget *m_filesPage = nullptr;
    QLabel *m_status = nullptr;
    QToolButton *m_filesBtn = nullptr;
    QToolButton *m_previewBtn = nullptr;
    QToolButton *m_reloadBtn = nullptr;
    QToolButton *m_stopBackendBtn = nullptr;
    QListWidget *m_fileList = nullptr;
    QPlainTextEdit *m_fileView = nullptr;

#ifdef HAS_QT_WEBENGINE
    QWebEngineView *m_webView = nullptr;
#else
    QTextBrowser *m_webView = nullptr;
#endif

    QProcess *m_backend = nullptr;
};
