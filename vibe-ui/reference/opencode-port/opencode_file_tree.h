// =============================================================================
// opencode_file_tree.h — File tree sidebar for project navigation
//
// Replicates opencode desktop v2's file explorer sidebar:
//   - QTreeView with project file structure
//   - File icons by extension (basic mapping)
//   - Click to open files in review panel
//   - Collapsible directories
//   - Rooted at project directory
//
// opencode design: The file tree sits in a left sidebar (or split panel),
// with a subtle background (#0d1117), file/directory icons, and a
// collapsible tree structure.
// =============================================================================

#ifndef OPENCODE_FILE_TREE_H
#define OPENCODE_FILE_TREE_H

#include <QWidget>
#include <QTreeView>
#include <QFileSystemModel>
#include <QVBoxLayout>
#include <QLineEdit>
#include <QSortFilterProxyModel>
#include <QString>
#include <QDir>

// ---------------------------------------------------------------------------
// OpenCodeFileTree — Project file browser sidebar
// ---------------------------------------------------------------------------
class OpenCodeFileTree : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeFileTree(QWidget* parent = nullptr);

    // Set the root directory to display.
    void setRootPath(const QString& path);

    // Filter visible files (e.g., hide node_modules, .git).
    void setShowHiddenFiles(bool show);

    // Get currently selected file path.
    QString selectedFilePath() const;

signals:
    // Emitted when user clicks a file to open it.
    void fileActivated(const QString& filePath);

    // Emitted when the root path changes.
    void rootPathChanged(const QString& newPath);

private slots:
    void onItemActivated(const QModelIndex& index);
    void onFilterTextChanged(const QString& text);

private:
    void setupUi();

    QLineEdit* m_filterEdit = nullptr;
    QTreeView* m_treeView = nullptr;
    QFileSystemModel* m_fileModel = nullptr;
    QSortFilterProxyModel* m_proxyModel = nullptr;
    QString m_rootPath;
};

#endif // OPENCODE_FILE_TREE_H
