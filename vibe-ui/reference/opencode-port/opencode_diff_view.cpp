// =============================================================================
// opencode_diff_view.cpp — Diff/review panel implementation
// =============================================================================

#include "opencode_diff_view.h"

#include <QSplitter>
#include <QScrollBar>
#include <QTextBlock>
#include <QPainter>
#include <QRegularExpression>
#include <QFileInfo>
#include <QDir>

// ---------------------------------------------------------------------------
// Helper: Generate HTML for a file view with syntax highlighting
// ---------------------------------------------------------------------------
static const char* kDiffViewCss = R"(
body { background-color:#0d1117; color:#c9d1d9; font-family:Consolas,monospace;
       font-size:13px; margin:0; padding:8px; }
.added { background-color:#0d2818; }
.removed { background-color:#280d0d; }
.added-inline { background-color:#1a3a2a; }
.removed-inline { background-color:#3a1a1a; }
.header { color:#8b949e; margin:4px 0; }
.line-num { color:#484f58; min-width:48px; display:inline-block; user-select:none; }
.keyword { color:#ff7b72; }
.string { color:#a5d6ff; }
.comment { color:#8b949e; font-style:italic; }
.type { color:#ffa657; }
.function { color:#d2a8ff; }
.number { color:#79c0ff; }
)";

OpenCodeDiffView::OpenCodeDiffView(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeDiffView::setupUi()
{
    setObjectName(QStringLiteral("DiffViewWidget"));

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    // Tab bar for open files.
    m_tabBar = new QTabBar(this);
    m_tabBar->setObjectName(QStringLiteral("DiffTabBar"));
    m_tabBar->setTabsClosable(true);
    m_tabBar->setMovable(false);
    m_tabBar->setExpanding(false);
    m_tabBar->setDocumentMode(true);
    m_tabBar->setDrawBase(false);
    m_tabBar->setElideMode(Qt::ElideLeft);
    m_tabBar->setMinimumHeight(32);
    layout->addWidget(m_tabBar);

    // Stacked views.
    m_viewStack = new QStackedWidget(this);
    m_viewStack->setObjectName(QStringLiteral("DiffViewStack"));
    layout->addWidget(m_viewStack, 1);

    // Empty state.
    m_emptyView = new QWidget(m_viewStack);
    m_emptyView->setObjectName(QStringLiteral("DiffEmptyView"));
    auto* emptyLayout = new QVBoxLayout(m_emptyView);
    auto* emptyLabel = new QLabel(tr("No file open. Click a file in the tree to view it."),
                                   m_emptyView);
    emptyLabel->setObjectName(QStringLiteral("DiffEmptyLabel"));
    emptyLabel->setAlignment(Qt::AlignCenter);
    emptyLabel->setWordWrap(true);
    emptyLayout->addWidget(emptyLabel);
    m_viewStack->addWidget(m_emptyView);
    m_viewStack->setCurrentWidget(m_emptyView);

    connect(m_tabBar, &QTabBar::currentChanged, this, &OpenCodeDiffView::onTabChanged);
    connect(m_tabBar, &QTabBar::tabCloseRequested, this, &OpenCodeDiffView::onTabCloseRequested);
}

void OpenCodeDiffView::openFile(const QString& filePath, const QString& content)
{
    // Check if already open.
    if (m_openFiles.contains(filePath)) {
        // Switch to existing tab.
        for (int i = 0; i < m_tabBar->count(); ++i) {
            if (m_tabBar->tabData(i).toString() == filePath) {
                m_tabBar->setCurrentIndex(i);
                return;
            }
        }
    }

    auto* view = new QTextEdit(this);
    view->setObjectName(QStringLiteral("DiffContent"));
    view->setReadOnly(true);
    view->setFrameShape(QFrame::NoFrame);
    QString html = generateFileHtml(filePath, content);
    view->setHtml(html);
    m_viewStack->addWidget(view);

    setupFileTab(filePath, view, false);
    m_tabBar->setCurrentIndex(m_tabBar->count() - 1);
}

void OpenCodeDiffView::showDiff(const QString& filePath,
                                  const QString& oldContent,
                                  const QString& newContent)
{
    // Remove existing tab for this file if present.
    if (m_openFiles.contains(filePath)) {
        closeFile(filePath);
    }

    auto* view = new QTextEdit(this);
    view->setObjectName(QStringLiteral("DiffContent"));
    view->setReadOnly(true);
    view->setFrameShape(QFrame::NoFrame);
    QString diffHtml = generateDiffHtml(oldContent, newContent);
    view->setHtml(diffHtml);
    m_viewStack->addWidget(view);

    setupFileTab(filePath, view, true);
    m_tabBar->setCurrentIndex(m_tabBar->count() - 1);
}

void OpenCodeDiffView::setupFileTab(const QString& filePath, QWidget* view, bool isDiff)
{
    QFileInfo fi(filePath);
    QString tabLabel = isDiff
        ? fi.fileName() + QStringLiteral(" ") + tr("(changes)")
        : fi.fileName();

    int idx = m_tabBar->addTab(tabLabel);
    m_tabBar->setTabData(idx, filePath);
    m_tabBar->setTabToolTip(idx, filePath);

    FileEntry entry;
    entry.path = filePath;
    entry.view = view;
    entry.isDiff = isDiff;
    m_openFiles[filePath] = entry;
}

void OpenCodeDiffView::setDiffMode(DiffMode mode)
{
    m_diffMode = mode;
    // Re-render current diff if any.
    for (auto it = m_openFiles.begin(); it != m_openFiles.end(); ++it) {
        if (it->isDiff) {
            // Would re-generate diff in new mode here.
        }
    }
}

void OpenCodeDiffView::closeFile(const QString& filePath)
{
    if (!m_openFiles.contains(filePath)) return;

    auto& entry = m_openFiles[filePath];
    if (entry.view) {
        m_viewStack->removeWidget(entry.view);
        entry.view->deleteLater();
    }

    // Remove tab.
    for (int i = 0; i < m_tabBar->count(); ++i) {
        if (m_tabBar->tabData(i).toString() == filePath) {
            m_tabBar->removeTab(i);
            break;
        }
    }

    m_openFiles.remove(filePath);

    if (m_openFiles.isEmpty()) {
        m_viewStack->setCurrentWidget(m_emptyView);
    }

    emit fileClosed(filePath);
}

void OpenCodeDiffView::closeAll()
{
    QStringList files = m_openFiles.keys();
    for (const auto& f : files) {
        closeFile(f);
    }
}

QString OpenCodeDiffView::currentFile() const
{
    int idx = m_tabBar->currentIndex();
    if (idx < 0) return {};
    return m_tabBar->tabData(idx).toString();
}

void OpenCodeDiffView::onTabChanged(int index)
{
    if (index < 0) {
        m_viewStack->setCurrentWidget(m_emptyView);
        return;
    }
    QString path = m_tabBar->tabData(index).toString();
    if (m_openFiles.contains(path) && m_openFiles[path].view) {
        m_viewStack->setCurrentWidget(m_openFiles[path].view);
    }
}

void OpenCodeDiffView::onTabCloseRequested(int index)
{
    QString path = m_tabBar->tabData(index).toString();
    closeFile(path);
}

// ---------------------------------------------------------------------------
// Basic syntax highlighting for common languages
// ---------------------------------------------------------------------------

QString OpenCodeDiffView::highlightSyntax(const QString& code, const QString& filePath)
{
    QFileInfo fi(filePath);
    QString ext = fi.suffix().toLower();

    // For code files, do basic highlighting.
    static const QStringList codeExts = {
        QStringLiteral("cpp"), QStringLiteral("h"), QStringLiteral("c"),
        QStringLiteral("py"), QStringLiteral("rs"), QStringLiteral("go"),
        QStringLiteral("ts"), QStringLiteral("tsx"), QStringLiteral("js"),
        QStringLiteral("jsx"), QStringLiteral("java"), QStringLiteral("kt"),
        QStringLiteral("swift"), QStringLiteral("rb")
    };

    QString escaped = code.toHtmlEscaped();

    if (!codeExts.contains(ext) && ext != QStringLiteral("css")
        && ext != QStringLiteral("html")) {
        return escaped;
    }

    // C-style comments: // ...
    escaped.replace(QRegularExpression(QStringLiteral("(//[^\n]*)")),
                    QStringLiteral("<span class='comment'>\\1</span>"));

    // C-style block comments: /* ... */
    escaped.replace(QRegularExpression(QStringLiteral("(/\\*[\\s\\S]*?\\*/)")),
                    QStringLiteral("<span class='comment'>\\1</span>"));

    // Strings: "..."
    escaped.replace(QRegularExpression(QStringLiteral("(\"[^\"]*\")")),
                    QStringLiteral("<span class='string'>\\1</span>"));

    // Single-quote strings: '...'
    escaped.replace(QRegularExpression(QStringLiteral("('[^']*')")),
                    QStringLiteral("<span class='string'>\\1</span>"));

    // Keywords (C/C++/Java/TS common).
    static const QStringList keywords = {
        QStringLiteral("class"), QStringLiteral("struct"), QStringLiteral("enum"),
        QStringLiteral("interface"), QStringLiteral("extends"), QStringLiteral("implements"),
        QStringLiteral("public"), QStringLiteral("private"), QStringLiteral("protected"),
        QStringLiteral("static"), QStringLiteral("const"), QStringLiteral("virtual"),
        QStringLiteral("override"), QStringLiteral("final"), QStringLiteral("abstract"),
        QStringLiteral("new"), QStringLiteral("delete"), QStringLiteral("return"),
        QStringLiteral("if"), QStringLiteral("else"), QStringLiteral("for"),
        QStringLiteral("while"), QStringLiteral("do"), QStringLiteral("switch"),
        QStringLiteral("case"), QStringLiteral("break"), QStringLiteral("continue"),
        QStringLiteral("try"), QStringLiteral("catch"), QStringLiteral("throw"),
        QStringLiteral("import"), QStringLiteral("export"), QStringLiteral("from"),
        QStringLiteral("async"), QStringLiteral("await"), QStringLiteral("function"),
        QStringLiteral("let"), QStringLiteral("var"), QStringLiteral("const"),
        QStringLiteral("sizeof"), QStringLiteral("typedef"), QStringLiteral("template"),
        QStringLiteral("namespace"), QStringLiteral("using"), QStringLiteral("typeof"),
        QStringLiteral("void"), QStringLiteral("int"), QStringLiteral("long"),
        QStringLiteral("float"), QStringLiteral("double"), QStringLiteral("bool"),
        QStringLiteral("char"), QStringLiteral("auto"), QStringLiteral("true"),
        QStringLiteral("false"), QStringLiteral("null"), QStringLiteral("undefined"),
        QStringLiteral("this"), QStringLiteral("super"), QStringLiteral("self"),
    };

    for (const auto& kw : keywords) {
        QRegularExpression re(QStringLiteral("\\b(%1)\\b").arg(kw));
        escaped.replace(re,
            QStringLiteral("<span class='keyword'>\\1</span>"));
    }

    // Numbers.
    escaped.replace(QRegularExpression(QStringLiteral("\\b(\\d+\\.?\\d*)\\b")),
                    QStringLiteral("<span class='number'>\\1</span>"));

    return escaped;
}

QString OpenCodeDiffView::generateFileHtml(const QString& filePath,
                                              const QString& content)
{
    QFileInfo fi(filePath);
    QString ext = fi.suffix().toLower();
    QString lang = ext.isEmpty() ? QStringLiteral("text") : ext;

    QStringList lines = content.split(QStringLiteral("\n"));
    QString body;

    for (int i = 0; i < lines.size(); ++i) {
        QString lineNum = QStringLiteral("<span class='line-num'>%1</span>").arg(i + 1);
        QString lineContent = highlightSyntax(lines[i], filePath);
        body += lineNum + QStringLiteral(" ") + lineContent + QStringLiteral("<br>");
    }

    return QStringLiteral(
        "<html><head><style>%1</style></head>"
        "<body>"
        "<div class='header'>%2 (%3 lines, %4)</div>"
        "%5"
        "</body></html>")
        .arg(QString::fromLatin1(kDiffViewCss),
             fi.fileName().toHtmlEscaped(),
             QString::number(lines.size()),
             lang)
        .arg(body);
}

QString OpenCodeDiffView::generateDiffHtml(const QString& oldContent,
                                              const QString& newContent)
{
    QStringList oldLines = oldContent.split(QStringLiteral("\n"));
    QStringList newLines = newContent.split(QStringLiteral("\n"));

    // Simple line-by-line diff (unified mode for now).
    // In a full implementation, this would use a proper diff algorithm
    // (e.g., Myers diff). The demo uses a basic line-matching approach.
    QString body;

    int maxLines = qMax(oldLines.size(), newLines.size());
    int oldIdx = 0, newIdx = 0;

    while (oldIdx < oldLines.size() || newIdx < newLines.size()) {
        QString oldLine = oldIdx < oldLines.size() ? oldLines[oldIdx] : QString();
        QString newLine = newIdx < newLines.size() ? newLines[newIdx] : QString();

        if (oldIdx < oldLines.size() && newIdx < newLines.size()
            && oldLine == newLine) {
            // Unchanged line.
            QString num = QStringLiteral("<span class='line-num'>%1</span>").arg(oldIdx + 1);
            body += num + QStringLiteral(" ") + oldLine.toHtmlEscaped()
                    + QStringLiteral("<br>");
            oldIdx++;
            newIdx++;
        } else if (oldIdx < oldLines.size() && newIdx < newLines.size()) {
            // Changed lines.
            QString oldNum = QString::number(oldIdx + 1);
            QString newNum = QString::number(newIdx + 1);
            body += QStringLiteral("<span class='removed'><span class='line-num'>- %1</span> %2</span><br>")
                    .arg(oldNum, oldLine.toHtmlEscaped());
            body += QStringLiteral("<span class='added'><span class='line-num'>+ %1</span> %2</span><br>")
                    .arg(newNum, newLine.toHtmlEscaped());
            oldIdx++;
            newIdx++;
        } else if (oldIdx < oldLines.size()) {
            // Removed lines.
            QString num = QString::number(oldIdx + 1);
            body += QStringLiteral("<span class='removed'><span class='line-num'>- %1</span> %2</span><br>")
                    .arg(num, oldLines[oldIdx].toHtmlEscaped());
            oldIdx++;
        } else {
            // Added lines.
            QString num = QString::number(newIdx + 1);
            body += QStringLiteral("<span class='added'><span class='line-num'>+ %1</span> %2</span><br>")
                    .arg(num, newLines[newIdx].toHtmlEscaped());
            newIdx++;
        }
    }

    return QStringLiteral(
        "<html><head><style>%1</style></head>"
        "<body>"
        "<div class='header'>%2 — %3 changes</div>"
        "%4"
        "</body></html>")
        .arg(QString::fromLatin1(kDiffViewCss),
             tr("Diff view"),
             tr("unified"))
        .arg(body);
}
