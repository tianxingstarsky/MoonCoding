#include "appswidget.h"

#include "../rustbridge.h"

#ifdef HAS_QT_WEBENGINE
#include <QWebEnginePage>
#include <QWebEngineSettings>
#include <QWebEngineView>
#endif

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QProcess>
#include <QStackedWidget>
#include <QTextBrowser>
#include <QToolButton>
#include <QUrl>
#include <QVBoxLayout>

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
    stopBackend();
    m_workspace = workspace;
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
        settings->setAttribute(QWebEngineSettings::LocalContentCanAccessRemoteUrls, false);
        settings->setAttribute(QWebEngineSettings::ErrorPageEnabled, true);
        settings->setAttribute(QWebEngineSettings::PluginsEnabled, false);
    }
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
}

void AppsWidget::showFilesPane()
{
    m_previewBtn->setChecked(false);
    m_filesBtn->setChecked(true);
    populateFileList();
    m_stack->setCurrentWidget(m_filesPage);
}

void AppsWidget::loadIndexHtml()
{
    if (m_workspace.isEmpty()) {
        m_status->setText(tr("无项目 · %1").arg(previewModeLabel()));
        m_webView->setHtml(tr("<p>请先创建或打开项目</p>"));
        return;
    }

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

#ifdef HAS_QT_WEBENGINE
    m_webView->setUrl(QUrl::fromLocalFile(indexPath));
#else
    QFile f(indexPath);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        m_status->setText(tr("无法读取 index.html · %1").arg(previewModeLabel()));
        return;
    }
    const QByteArray html = f.readAll();
    const QUrl base = QUrl::fromLocalFile(QDir(m_workspace).absolutePath() + QLatin1Char('/'));
    m_webView->document()->setBaseUrl(base);
    m_webView->setHtml(QString::fromUtf8(html));
#endif
    m_status->setText(tr("预览 · %1 · %2")
                          .arg(QFileInfo(m_workspace).fileName(), previewModeLabel()));
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
    const QString script = QDir(m_workspace).filePath(QStringLiteral("backend.py"));
    if (!QFileInfo::exists(script)) {
        m_status->setText(tr("未找到 backend.py"));
        return;
    }
    stopBackend();
    m_backend = new QProcess(this);
    m_backend->setWorkingDirectory(m_workspace);
    m_backend->setProcessChannelMode(QProcess::MergedChannels);
    connect(m_backend, &QProcess::readyRead, this, [this] {
        if (!m_backend) {
            return;
        }
        const QByteArray chunk = m_backend->readAll();
        if (chunk.contains("READY")) {
            m_status->setText(tr("后端已就绪"));
        }
    });
    connect(m_backend,
            QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
            this,
            [this](int, QProcess::ExitStatus) {
                m_status->setText(tr("后端已退出"));
            });
    m_backend->start(QStringLiteral("python3"), {script});
    if (!m_backend->waitForStarted(2000)) {
        m_backend->start(QStringLiteral("python"), {script});
    }
    if (m_backend->state() == QProcess::Running) {
        m_status->setText(tr("后端启动中…"));
    } else {
        m_status->setText(tr("无法启动 python backend.py"));
    }
}

void AppsWidget::stopBackend()
{
    if (!m_backend) {
        return;
    }
    m_backend->kill();
    m_backend->waitForFinished(1500);
    m_backend->deleteLater();
    m_backend = nullptr;
    m_status->setText(tr("后端已停止"));
}
