// =============================================================================
// opencode_file_tree.cpp — File tree sidebar implementation
// =============================================================================

#include "opencode_file_tree.h"

#include <QHeaderView>
#include <QDir>
#include <QFileInfo>

OpenCodeFileTree::OpenCodeFileTree(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeFileTree::setupUi()
{
    setObjectName(QStringLiteral("FileTreeWidget"));

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    // Filter/search bar.
    m_filterEdit = new QLineEdit(this);
    m_filterEdit->setObjectName(QStringLiteral("FileTreeFilter"));
    m_filterEdit->setPlaceholderText(tr("Filter files..."));
    m_filterEdit->setClearButtonEnabled(true);
    layout->addWidget(m_filterEdit);

    // File system model.
    m_fileModel = new QFileSystemModel(this);
    m_fileModel->setFilter(QDir::AllDirs | QDir::Files | QDir::NoDotAndDotDot);
    m_fileModel->setRootPath(QString());
    // Name filters: exclude common generated/binary files.
    m_fileModel->setNameFilters({
        QStringLiteral("*.cpp"), QStringLiteral("*.h"), QStringLiteral("*.c"),
        QStringLiteral("*.py"), QStringLiteral("*.rs"), QStringLiteral("*.go"),
        QStringLiteral("*.ts"), QStringLiteral("*.tsx"), QStringLiteral("*.js"),
        QStringLiteral("*.jsx"), QStringLiteral("*.html"), QStringLiteral("*.css"),
        QStringLiteral("*.json"), QStringLiteral("*.yaml"), QStringLiteral("*.yml"),
        QStringLiteral("*.toml"), QStringLiteral("*.xml"), QStringLiteral("*.md"),
        QStringLiteral("*.txt"), QStringLiteral("*.qss"), QStringLiteral("*.cmake"),
        QStringLiteral("CMakeLists.txt"), QStringLiteral("*.toml"),
        QStringLiteral("*.svg"), QStringLiteral("*.png"), QStringLiteral("*.jpg"),
        QStringLiteral("*")
    });
    m_fileModel->setNameFilterDisables(false);

    // Proxy model for filtering.
    m_proxyModel = new QSortFilterProxyModel(this);
    m_proxyModel->setSourceModel(m_fileModel);
    m_proxyModel->setRecursiveFilteringEnabled(true);

    // Tree view.
    m_treeView = new QTreeView(this);
    m_treeView->setObjectName(QStringLiteral("FileTreeView"));
    m_treeView->setModel(m_proxyModel);
    m_treeView->setHeaderHidden(true);
    m_treeView->setAnimated(true);
    m_treeView->setIndentation(16);
    m_treeView->setExpandsOnDoubleClick(true);
    m_treeView->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_treeView->setDragEnabled(false);
    m_treeView->setSelectionMode(QAbstractItemView::SingleSelection);

    // Hide size/type/date columns, show only name.
    m_treeView->hideColumn(1);
    m_treeView->hideColumn(2);
    m_treeView->hideColumn(3);

    layout->addWidget(m_treeView, 1);

    // Connect signals.
    connect(m_treeView, &QTreeView::activated, this, &OpenCodeFileTree::onItemActivated);
    connect(m_filterEdit, &QLineEdit::textChanged, this, &OpenCodeFileTree::onFilterTextChanged);
}

void OpenCodeFileTree::setRootPath(const QString& path)
{
    if (m_rootPath == path) return;
    m_rootPath = path;

    QModelIndex sourceIdx = m_fileModel->setRootPath(path);
    QModelIndex proxyIdx = m_proxyModel->mapFromSource(sourceIdx);
    m_treeView->setRootIndex(proxyIdx);

    // Expand the first level.
    m_treeView->expand(proxyIdx);

    emit rootPathChanged(path);
}

void OpenCodeFileTree::setShowHiddenFiles(bool show)
{
    QDir::Filters filters = m_fileModel->filter();
    if (show)
        filters |= QDir::Hidden;
    else
        filters &= ~QDir::Hidden;
    m_fileModel->setFilter(filters);
}

QString OpenCodeFileTree::selectedFilePath() const
{
    QModelIndex idx = m_treeView->currentIndex();
    if (!idx.isValid()) return {};
    QModelIndex sourceIdx = m_proxyModel->mapToSource(idx);
    return m_fileModel->filePath(sourceIdx);
}

void OpenCodeFileTree::onItemActivated(const QModelIndex& index)
{
    QModelIndex sourceIdx = m_proxyModel->mapToSource(index);
    QFileInfo fi = m_fileModel->fileInfo(sourceIdx);
    if (fi.isFile()) {
        emit fileActivated(fi.absoluteFilePath());
    }
}

void OpenCodeFileTree::onFilterTextChanged(const QString& text)
{
    m_proxyModel->setFilterFixedString(text);
    // Expand all visible items when filtering.
    if (!text.isEmpty()) {
        m_treeView->expandAll();
    }
}
