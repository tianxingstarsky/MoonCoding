#include "appswidget.h"

#include "../rustbridge.h"

#ifdef HAS_QT_WEBENGINE
#include <QWebEnginePage>
#include <QWebEngineSettings>
#include <QWebEngineView>
#endif

#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QProcess>
#include <QProcessEnvironment>
#include <QStackedWidget>
#include <QTextBrowser>
#include <QToolButton>
#include <QUrl>
#include <QVBoxLayout>

#include <QtGlobal>

namespace {

// Must match vibe-agent `preview_backend::port_for_workspace` (FNV-1a, base 18765, span 2000).
quint16 portForWorkspaceKey(const QString &key)
{
    quint32 hash = 2166136261u;
    const QByteArray bytes = key.toUtf8();
    for (unsigned char b : bytes) {
        hash ^= quint32(b);
        hash *= 16777619u;
    }
    return static_cast<quint16>(18765u + (hash % 2000u));
}

QString normalizeWorkspaceKey(const QString &workspace)
{
    const QString canonical = QFileInfo(workspace).canonicalFilePath();
    QString key = canonical.isEmpty() ? workspace : canonical;
    key.replace(QLatin1Char('\\'), QLatin1Char('/'));
    return key.toLower();
}

} // namespace

#ifdef HAS_QT_WEBENGINE
namespace {

class MoonCodingWebPage final : public QWebEnginePage
{
public:
    explicit MoonCodingWebPage(AppsWidget *host, QObject *parent = nullptr)
        : QWebEnginePage(parent)
        , m_host(host)
    {
    }

protected:
    bool acceptNavigationRequest(const QUrl &url,
                                 NavigationType type,
                                 bool isMainFrame) override
    {
        Q_UNUSED(type);
        Q_UNUSED(isMainFrame);
        if (url.scheme() == QLatin1String("mooncoding") && m_host) {
            m_host->handleMooncodingUrl(url);
            return false;
        }
        return QWebEnginePage::acceptNavigationRequest(url, type, isMainFrame);
    }

private:
    AppsWidget *m_host = nullptr;
};

} // namespace
#endif

void AppsWidget::prepareWebEngineEnvironment()
{
#ifdef HAS_QT_WEBENGINE
    auto put = [](const char *key, const char *value) {
        if (qEnvironmentVariableIsEmpty(key)) {
            qputenv(key, value);
        }
    };
    // Chromium flags must be set before QApplication.
    put("QTWEBENGINE_DISABLE_SANDBOX", "1");
    put("QTWEBENGINE_CHROMIUM_FLAGS",
        "--disable-gpu --disable-gpu-compositing --no-sandbox --disable-dev-shm-usage");
    put("QT_QUICK_BACKEND", "software");

    // Paths need a QCoreApplication instance — skip if called too early.
    if (QCoreApplication::instance() == nullptr) {
        return;
    }
    const QString appDir = QCoreApplication::applicationDirPath();
    if (!appDir.isEmpty()) {
        const QString processPath = appDir + QStringLiteral("/libexec/QtWebEngineProcess");
        if (QFileInfo::exists(processPath) && qEnvironmentVariableIsEmpty("QTWEBENGINEPROCESS_PATH")) {
            qputenv("QTWEBENGINEPROCESS_PATH", QFile::encodeName(processPath));
        }
        const QString resources = appDir + QStringLiteral("/resources");
        if (QDir(resources).exists() && qEnvironmentVariableIsEmpty("QTWEBENGINE_RESOURCES_PATH")) {
            qputenv("QTWEBENGINE_RESOURCES_PATH", QFile::encodeName(resources));
        }
        const QString locales = appDir + QStringLiteral("/translations/qtwebengine_locales");
        if (QDir(locales).exists() && qEnvironmentVariableIsEmpty("QTWEBENGINE_LOCALES_PATH")) {
            qputenv("QTWEBENGINE_LOCALES_PATH", QFile::encodeName(locales));
        }
    }
#endif
}

AppsWidget::AppsWidget(RustBridge *bridge, QWidget *parent)
    : QWidget(parent)
    , m_bridge(bridge)
{
    Q_UNUSED(m_bridge);
    buildUi();
}

AppsWidget::~AppsWidget()
{
    stopBackend();
}

void AppsWidget::setWorkspace(const QString &workspace)
{
    if (m_workspace == workspace) {
        return;
    }
    // Project switch: destroy backend immediately so the port is freed.
    stopBackend();
    m_workspace = workspace;
    m_backendPort = workspace.isEmpty() ? 0 : portForWorkspaceKey(normalizeWorkspaceKey(workspace));
    refresh();
}

QString AppsWidget::previewModeLabel() const
{
#ifdef HAS_QT_WEBENGINE
    return tr("WebEngine");
#else
    return tr("简化预览");
#endif
}

void AppsWidget::buildUi()
{
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(0, 0, 0, 0);
    root->setSpacing(0);

    auto *toolbar = new QWidget(this);
    toolbar->setObjectName(QStringLiteral("appToolbar"));
    auto *tb = new QHBoxLayout(toolbar);
    tb->setContentsMargins(8, 6, 8, 6);
    tb->setSpacing(8);

    m_previewBtn = new QToolButton(toolbar);
    m_previewBtn->setText(tr("预览"));
    m_previewBtn->setCheckable(true);
    m_previewBtn->setChecked(true);
    m_filesBtn = new QToolButton(toolbar);
    m_filesBtn->setText(tr("文件"));
    m_filesBtn->setCheckable(true);
    m_reloadBtn = new QToolButton(toolbar);
    m_reloadBtn->setText(tr("刷新"));
    m_stopBackendBtn = new QToolButton(toolbar);
    m_stopBackendBtn->setText(tr("停后端"));
    m_stopBackendBtn->setVisible(false);
    m_status = new QLabel(tr("项目预览"), toolbar);
    m_status->setObjectName(QStringLiteral("mutedLabel"));
    m_status->setWordWrap(true);

    tb->addWidget(m_previewBtn);
    tb->addWidget(m_filesBtn);
    tb->addWidget(m_reloadBtn);
    tb->addWidget(m_stopBackendBtn);
    tb->addWidget(m_status, 1);

    m_stack = new QStackedWidget(this);

    m_previewPage = new QWidget(m_stack);
    auto *previewLayout = new QVBoxLayout(m_previewPage);
    previewLayout->setContentsMargins(0, 0, 0, 0);
#ifdef HAS_QT_WEBENGINE
    m_webView = new QWebEngineView(m_previewPage);
    auto *page = new MoonCodingWebPage(this, m_webView);
    m_webView->setPage(page);
    if (auto *settings = page->settings()) {
        settings->setAttribute(QWebEngineSettings::JavascriptEnabled, true);
        settings->setAttribute(QWebEngineSettings::LocalContentCanAccessFileUrls, true);
        // Allow fetch() from file:// preview to http://127.0.0.1:<port>.
        settings->setAttribute(QWebEngineSettings::LocalContentCanAccessRemoteUrls, true);
        settings->setAttribute(QWebEngineSettings::ErrorPageEnabled, true);
        settings->setAttribute(QWebEngineSettings::PluginsEnabled, false);
    }
    connect(m_webView, &QWebEngineView::loadFinished, this, [this](bool ok) {
        if (ok) {
            injectApiBase();
        }
    });
#else
    m_webView = new QTextBrowser(m_previewPage);
    m_webView->setOpenLinks(false);
    m_webView->setOpenExternalLinks(false);
    connect(m_webView, &QTextBrowser::anchorClicked, this, &AppsWidget::onAnchorClicked);
#endif
    previewLayout->addWidget(m_webView, 1);

    m_filesPage = new QWidget(m_stack);
    auto *filesLayout = new QVBoxLayout(m_filesPage);
    filesLayout->setContentsMargins(8, 8, 8, 8);
    m_fileList = new QListWidget(m_filesPage);
    m_fileView = new QPlainTextEdit(m_filesPage);
    m_fileView->setReadOnly(true);
    m_fileView->setPlaceholderText(tr("选择一个文件查看内容"));
    filesLayout->addWidget(m_fileList, 1);
    filesLayout->addWidget(m_fileView, 2);
    connect(m_fileList, &QListWidget::itemActivated, this, &AppsWidget::onFileActivated);
    connect(m_fileList, &QListWidget::itemClicked, this, &AppsWidget::onFileActivated);

    m_stack->addWidget(m_previewPage);
    m_stack->addWidget(m_filesPage);

    root->addWidget(toolbar);
    root->addWidget(m_stack, 1);

    connect(m_previewBtn, &QToolButton::clicked, this, &AppsWidget::showPreviewPane);
    connect(m_filesBtn, &QToolButton::clicked, this, &AppsWidget::showFilesPane);
    connect(m_reloadBtn, &QToolButton::clicked, this, &AppsWidget::reloadPreview);
    connect(m_stopBackendBtn, &QToolButton::clicked, this, &AppsWidget::stopBackend);
}

void AppsWidget::refresh()
{
    loadIndexHtml();
    populateFileList();
    showPreviewPane();
}

void AppsWidget::reloadPreview()
{
    loadIndexHtml();
    showPreviewPane();
}

void AppsWidget::showPreviewPane()
{
    m_previewBtn->setChecked(true);
    m_filesBtn->setChecked(false);
    m_stack->setCurrentWidget(m_previewPage);
    // Opening preview auto-starts backend when marked; keeps running if already up.
    ensureBackendRunning();
}

void AppsWidget::showFilesPane()
{
    // Stay on the same project: do NOT stop the backend (background OK).
    m_previewBtn->setChecked(false);
    m_filesBtn->setChecked(true);
    populateFileList();
    m_stack->setCurrentWidget(m_filesPage);
}

QString AppsWidget::backendScriptPath() const
{
    return QDir(m_workspace).filePath(QStringLiteral("backend.py"));
}

QString AppsWidget::backendLeasePath() const
{
    return QDir(m_workspace).filePath(QStringLiteral(".mooncoding/preview_backend.json"));
}

bool AppsWidget::hasBackendScript() const
{
    return !m_workspace.isEmpty() && QFileInfo::exists(backendScriptPath());
}

quint16 AppsWidget::backendPort() const
{
    if (m_backendPort != 0) {
        return m_backendPort;
    }
    if (m_workspace.isEmpty()) {
        return 0;
    }
    return portForWorkspaceKey(normalizeWorkspaceKey(m_workspace));
}

QString AppsWidget::backendApiBase() const
{
    const quint16 port = backendPort();
    if (port == 0) {
        return QString();
    }
    return QStringLiteral("http://127.0.0.1:%1").arg(port);
}

void AppsWidget::updateBackendButton()
{
    if (!m_stopBackendBtn) {
        return;
    }
    const bool show = hasBackendScript();
    m_stopBackendBtn->setVisible(show);
    m_stopBackendBtn->setEnabled(show);
}

void AppsWidget::writeBackendLease(qint64 pid, quint16 port)
{
    if (m_workspace.isEmpty() || pid <= 0 || port == 0) {
        return;
    }
    const QString path = backendLeasePath();
    QDir().mkpath(QFileInfo(path).absolutePath());
    QJsonObject obj;
    obj.insert(QStringLiteral("pid"), static_cast<double>(pid));
    obj.insert(QStringLiteral("port"), static_cast<int>(port));
    obj.insert(QStringLiteral("workspace"), normalizeWorkspaceKey(m_workspace));
    obj.insert(QStringLiteral("script"), QStringLiteral("backend.py"));
    obj.insert(QStringLiteral("api_base"), backendApiBase());
    obj.insert(QStringLiteral("started_at_unix"),
               static_cast<double>(QDateTime::currentSecsSinceEpoch()));
    QFile f(path);
    if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        f.write(QJsonDocument(obj).toJson(QJsonDocument::Indented));
    }
}

void AppsWidget::clearBackendLease()
{
    if (m_workspace.isEmpty()) {
        return;
    }
    QFile::remove(backendLeasePath());
}

bool AppsWidget::adoptRunningLease()
{
    const QString path = backendLeasePath();
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        return false;
    }
    const QJsonObject obj = QJsonDocument::fromJson(f.readAll()).object();
    const qint64 pid = static_cast<qint64>(obj.value(QStringLiteral("pid")).toDouble());
    if (pid <= 0) {
        return false;
    }
#ifdef Q_OS_WIN
    QProcess probe;
    probe.start(QStringLiteral("tasklist"),
                {QStringLiteral("/FI"), QStringLiteral("PID eq %1").arg(pid)});
    if (!probe.waitForFinished(1500)) {
        return false;
    }
    const QString out = QString::fromLocal8Bit(probe.readAllStandardOutput());
    if (!out.contains(QString::number(pid))) {
        clearBackendLease();
        return false;
    }
#else
    // kill -0 <pid>
    if (QProcess::execute(QStringLiteral("kill"), {QStringLiteral("-0"), QString::number(pid)}) != 0) {
        clearBackendLease();
        return false;
    }
#endif
    const int port = obj.value(QStringLiteral("port")).toInt();
    if (port > 0) {
        m_backendPort = static_cast<quint16>(port);
    }
    return true;
}

void AppsWidget::killLeasePidIfAny()
{
    const QString path = backendLeasePath();
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        return;
    }
    const QJsonObject obj = QJsonDocument::fromJson(f.readAll()).object();
    const qint64 pid = static_cast<qint64>(obj.value(QStringLiteral("pid")).toDouble());
    if (pid <= 0) {
        return;
    }
#ifdef Q_OS_WIN
    QProcess::execute(QStringLiteral("taskkill"),
                      {QStringLiteral("/PID"), QString::number(pid), QStringLiteral("/T"),
                       QStringLiteral("/F")});
#else
    QProcess::execute(QStringLiteral("kill"), {QStringLiteral("-TERM"), QString::number(pid)});
    QProcess::execute(QStringLiteral("kill"), {QStringLiteral("-KILL"), QString::number(pid)});
#endif
}

void AppsWidget::injectApiBase()
{
    if (!hasBackendScript()) {
        return;
    }
    const QString api = backendApiBase();
    if (api.isEmpty()) {
        return;
    }
#ifdef HAS_QT_WEBENGINE
    if (!m_webView || !m_webView->page()) {
        return;
    }
    const QString safeApi = QString(api).replace(QLatin1Char('\\'), QLatin1String("\\\\"))
                                .replace(QLatin1Char('\''), QLatin1String("\\'"));
    const QString script = QStringLiteral(
                               "window.__MOONCODING_API_BASE__='%1';"
                               "window.__MOONCODING_BACKEND_PORT__=%2;")
                               .arg(safeApi)
                               .arg(backendPort());
    m_webView->page()->runJavaScript(script);
#else
    Q_UNUSED(api);
#endif
}

void AppsWidget::ensureBackendRunning()
{
    updateBackendButton();
    if (!hasBackendScript()) {
        return;
    }
    if (m_backend && m_backend->state() == QProcess::Running) {
        m_status->setText(tr("预览 · %1 · %2 · 后端 :%3")
                              .arg(QFileInfo(m_workspace).fileName(),
                                   previewModeLabel(),
                                   QString::number(backendPort())));
        injectApiBase();
        return;
    }
    if (adoptRunningLease()) {
        m_status->setText(tr("预览 · %1 · %2 · 后端后台 :%3")
                              .arg(QFileInfo(m_workspace).fileName(),
                                   previewModeLabel(),
                                   QString::number(backendPort())));
        injectApiBase();
        return;
    }
    startBackend();
}

void AppsWidget::loadIndexHtml()
{
    if (m_workspace.isEmpty()) {
        m_status->setText(tr("无项目 · %1").arg(previewModeLabel()));
        updateBackendButton();
        m_webView->setHtml(tr("<p>请先创建或打开项目</p>"));
        return;
    }

    m_backendPort = portForWorkspaceKey(normalizeWorkspaceKey(m_workspace));
    updateBackendButton();

    const QString indexPath = QDir(m_workspace).absoluteFilePath(QStringLiteral("index.html"));
    if (!QFileInfo::exists(indexPath)) {
        m_status->setText(tr("缺少 index.html — 让 AI 在本工作区创建入口 · %1")
                              .arg(previewModeLabel()));
        const QString missing = tr(
            "<html><body style='font-family:sans-serif;padding:24px'>"
            "<h2>还没有 index.html</h2>"
            "<p>当前工作区：%1</p>"
            "<p>请在聊天中让 AI <b>新建</b> index.html（竖屏手机布局）。</p>"
            "</body></html>")
                                    .arg(m_workspace);
        m_webView->setHtml(missing);
        return;
    }

    // Auto-start before loading so the page can fetch immediately.
    ensureBackendRunning();

#ifdef HAS_QT_WEBENGINE
    m_webView->setUrl(QUrl::fromLocalFile(indexPath));
#else
    QFile f(indexPath);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        m_status->setText(tr("无法读取 index.html · %1").arg(previewModeLabel()));
        return;
    }
    QByteArray html = f.readAll();
    if (hasBackendScript()) {
        const QByteArray inject =
            QByteArrayLiteral("<script>window.__MOONCODING_API_BASE__='")
            + backendApiBase().toUtf8()
            + QByteArrayLiteral("';window.__MOONCODING_BACKEND_PORT__=")
            + QByteArray::number(backendPort())
            + QByteArrayLiteral(";</script>");
        const int head = html.indexOf("<head>");
        if (head >= 0) {
            html.insert(head + 6, inject);
        } else {
            html.prepend(inject);
        }
    }
    const QUrl base = QUrl::fromLocalFile(QDir(m_workspace).absolutePath() + QLatin1Char('/'));
    m_webView->document()->setBaseUrl(base);
    m_webView->setHtml(QString::fromUtf8(html));
#endif
    if (hasBackendScript()
        && ((m_backend && m_backend->state() == QProcess::Running) || adoptRunningLease())) {
        m_status->setText(tr("预览 · %1 · %2 · 后端 :%3")
                              .arg(QFileInfo(m_workspace).fileName(),
                                   previewModeLabel(),
                                   QString::number(backendPort())));
    } else {
        m_status->setText(tr("预览 · %1 · %2")
                              .arg(QFileInfo(m_workspace).fileName(), previewModeLabel()));
    }
}

void AppsWidget::populateFileList()
{
    m_fileList->clear();
    if (m_workspace.isEmpty()) {
        return;
    }
    QDir dir(m_workspace);
    const QStringList names = dir.entryList(
        QStringList{QStringLiteral("*.html"), QStringLiteral("*.css"), QStringLiteral("*.js"),
                    QStringLiteral("*.mjs"), QStringLiteral("*.py"), QStringLiteral("*.md"),
                    QStringLiteral("*.json"), QStringLiteral("*.toml")},
        QDir::Files,
        QDir::Name);
    for (const QString &name : names) {
        auto *item = new QListWidgetItem(name, m_fileList);
        item->setData(Qt::UserRole, dir.filePath(name));
        item->setToolTip(languageForPath(name));
    }
}

void AppsWidget::onFileActivated(QListWidgetItem *item)
{
    if (!item) {
        return;
    }
    const QString path = item->data(Qt::UserRole).toString();
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        m_fileView->setPlainText(tr("无法打开：%1").arg(path));
        return;
    }
    QByteArray data = f.readAll();
    if (data.size() > 200000) {
        data = data.left(200000) + "\n…";
    }
    const QString lang = languageForPath(path);
    m_fileView->setPlainText(
        tr("// 语言：%1\n// 路径：%2\n\n%3")
            .arg(lang, path, QString::fromUtf8(data)));
}

QString AppsWidget::languageForPath(const QString &path) const
{
    const QString lower = path.toLower();
    if (lower.endsWith(QLatin1String(".html")) || lower.endsWith(QLatin1String(".htm"))) {
        return QStringLiteral("HTML");
    }
    if (lower.endsWith(QLatin1String(".css"))) {
        return QStringLiteral("CSS");
    }
    if (lower.endsWith(QLatin1String(".js")) || lower.endsWith(QLatin1String(".mjs"))) {
        return QStringLiteral("JavaScript");
    }
    if (lower.endsWith(QLatin1String(".py"))) {
        return QStringLiteral("Python");
    }
    if (lower.endsWith(QLatin1String(".md"))) {
        return QStringLiteral("Markdown");
    }
    if (lower.endsWith(QLatin1String(".json"))) {
        return QStringLiteral("JSON");
    }
    return QStringLiteral("Text");
}

void AppsWidget::onAnchorClicked(const QUrl &url)
{
    handleMooncodingUrl(url);
}

void AppsWidget::handleMooncodingUrl(const QUrl &url)
{
    if (url.scheme() != QLatin1String("mooncoding")) {
        return;
    }
    const QString path = url.path();
    const QString host = url.host();
    const QString combined = host + path;
    if (combined.contains(QLatin1String("backend/start"))
        || path.endsWith(QLatin1String("start"))) {
        startBackend();
    } else if (combined.contains(QLatin1String("backend/stop"))
               || path.endsWith(QLatin1String("stop"))) {
        stopBackend();
    }
}

void AppsWidget::startBackend()
{
    if (m_workspace.isEmpty()) {
        return;
    }
    const QString script = backendScriptPath();
    if (!QFileInfo::exists(script)) {
        m_status->setText(tr("未找到 backend.py"));
        updateBackendButton();
        return;
    }
    if (m_backend && m_backend->state() == QProcess::Running) {
        return;
    }
    if (adoptRunningLease()) {
        m_status->setText(tr("后端已在后台运行 :%1").arg(backendPort()));
        return;
    }

    if (m_backend) {
        m_backend->deleteLater();
        m_backend = nullptr;
    }

    m_backendPort = portForWorkspaceKey(normalizeWorkspaceKey(m_workspace));
    m_backend = new QProcess(this);
    m_backend->setWorkingDirectory(m_workspace);
    m_backend->setProcessChannelMode(QProcess::MergedChannels);
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    env.insert(QStringLiteral("MOONCODING_BACKEND_PORT"), QString::number(m_backendPort));
    env.insert(QStringLiteral("MOONCODING_BACKEND_HOST"), QStringLiteral("127.0.0.1"));
    env.insert(QStringLiteral("MOONCODING_API_BASE"), backendApiBase());
    m_backend->setProcessEnvironment(env);

    connect(m_backend, &QProcess::readyRead, this, [this] {
        if (!m_backend) {
            return;
        }
        const QByteArray chunk = m_backend->readAll();
        if (chunk.contains("READY")) {
            m_status->setText(tr("后端已就绪 :%1").arg(backendPort()));
            injectApiBase();
        }
    });
    connect(m_backend,
            QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
            this,
            [this](int, QProcess::ExitStatus) {
                clearBackendLease();
                if (m_backend) {
                    m_backend->deleteLater();
                    m_backend = nullptr;
                }
                m_status->setText(tr("后端已退出"));
                updateBackendButton();
            });

    m_backend->start(QStringLiteral("python"), {script});
    if (!m_backend->waitForStarted(2000) || m_backend->state() != QProcess::Running) {
        m_backend->start(QStringLiteral("python3"), {script});
        m_backend->waitForStarted(2000);
    }
#ifdef Q_OS_WIN
    if (m_backend->state() != QProcess::Running) {
        m_backend->start(QStringLiteral("py"), {QStringLiteral("-3"), script});
        m_backend->waitForStarted(2000);
    }
#endif
    if (m_backend->state() == QProcess::Running) {
        writeBackendLease(m_backend->processId(), m_backendPort);
        m_status->setText(tr("后端启动中… :%1").arg(m_backendPort));
        injectApiBase();
    } else {
        m_status->setText(tr("无法启动 python backend.py"));
        m_backend->deleteLater();
        m_backend = nullptr;
    }
    updateBackendButton();
}

void AppsWidget::stopBackend()
{
    if (m_backend) {
        m_backend->kill();
        m_backend->waitForFinished(1500);
        m_backend->deleteLater();
        m_backend = nullptr;
    } else {
        killLeasePidIfAny();
    }
    clearBackendLease();
    if (!m_workspace.isEmpty()) {
        m_status->setText(tr("后端已停止"));
    }
    updateBackendButton();
}
