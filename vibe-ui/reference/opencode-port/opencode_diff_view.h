// =============================================================================
// opencode_diff_view.h — Diff/review panel for file changes
//
// Replicates opencode desktop v2's review panel:
//   - File tabs for multiple open files
//   - Side-by-side or unified diff view
//   - Syntax highlighting (basic: keywords, strings, comments)
//   - Line numbers
//   - Added/removed line indicators (green/red background)
//
// opencode design: The review panel appears as a right-side panel or
// bottom split, showing file diffs with color-coded changes. Files are
// shown as tabs at the top, with a persistent diff viewer below.
// =============================================================================

#ifndef OPENCODE_DIFF_VIEW_H
#define OPENCODE_DIFF_VIEW_H

#include <QWidget>
#include <QTabBar>
#include <QTextEdit>
#include <QStackedWidget>
#include <QVBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QMap>

// ---------------------------------------------------------------------------
// DiffLineBlock — A single diff hunk display widget
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// OpenCodeDiffView — Multi-tab diff/review panel
// ---------------------------------------------------------------------------
class OpenCodeDiffView : public QWidget
{
    Q_OBJECT

public:
    enum DiffMode {
        Unified,       // Single column with +/- indicators
        SideBySide     // Two columns (left=old, right=new) — opencode default
    };

    explicit OpenCodeDiffView(QWidget* parent = nullptr);

    // Open a file for viewing (read-only, no diff).
    void openFile(const QString& filePath, const QString& content);

    // Show a diff between old and new content.
    void showDiff(const QString& filePath,
                  const QString& oldContent,
                  const QString& newContent);

    // Set the diff display mode.
    void setDiffMode(DiffMode mode);

    // Close a specific file tab.
    void closeFile(const QString& filePath);

    // Close all tabs.
    void closeAll();

    // Get currently viewed file.
    QString currentFile() const;

signals:
    void fileClosed(const QString& filePath);
    void acceptChanges(const QString& filePath);
    void rejectChanges(const QString& filePath);

private slots:
    void onTabChanged(int index);
    void onTabCloseRequested(int index);

private:
    void setupUi();
    void setupFileTab(const QString& filePath,
                      QWidget* view,
                      bool isDiff = false);
    QString generateDiffHtml(const QString& oldContent,
                              const QString& newContent);
    QString generateFileHtml(const QString& filePath,
                              const QString& content);
    QString highlightSyntax(const QString& code, const QString& filePath);

    // Tab system for open files.
    QTabBar* m_tabBar = nullptr;
    QStackedWidget* m_viewStack = nullptr;
    QWidget* m_emptyView = nullptr;

    // File tracking.
    struct FileEntry {
        QString path;
        QWidget* view = nullptr;
        bool isDiff = false;
    };
    QMap<QString, FileEntry> m_openFiles;

    // Mode.
    DiffMode m_diffMode = Unified;
};

#endif // OPENCODE_DIFF_VIEW_H
