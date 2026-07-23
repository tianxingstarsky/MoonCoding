// =============================================================================
// opencode_home_widget.h — Home page with project list
//
// Replicates opencode desktop v2's home screen:
//   - Project list with icons, names, paths, "last opened" timestamps
//   - Recently closed projects section
//   - New project button (+)
//   - Empty state with "no sessions yet" CTA
//   - Search/filter projects
//
// opencode design: The home page shows a grid/list of project cards.
// Each card has a folder icon, project name, path, and last-opened time.
// A "New Project" button opens a file dialog to select a directory.
// =============================================================================

#ifndef OPENCODE_HOME_WIDGET_H
#define OPENCODE_HOME_WIDGET_H

#include <QWidget>
#include <QFrame>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QLineEdit>
#include <QListWidget>
#include <QStackedWidget>
#include <QDateTime>
#include <QMouseEvent>
#include <QEnterEvent>

// ---------------------------------------------------------------------------
// ProjectCard — A single project entry in the home page list
// ---------------------------------------------------------------------------
class ProjectCard : public QFrame
{
    Q_OBJECT
public:
    explicit ProjectCard(const QString& name,
                         const QString& path,
                         const QDateTime& lastOpened = {},
                         QWidget* parent = nullptr);

    QString projectName() const { return m_name; }
    QString projectPath() const { return m_path; }

signals:
    void clicked(const QString& path);
    void removeRequested(const QString& path);

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    void setupUi();
    QString m_name;
    QString m_path;
    QLabel* m_iconLabel = nullptr;
    QLabel* m_nameLabel = nullptr;
    QLabel* m_pathLabel = nullptr;
    QLabel* m_timeLabel = nullptr;
    QPushButton* m_removeBtn = nullptr;
};

// ---------------------------------------------------------------------------
// OpenCodeHomeWidget — The home page
// ---------------------------------------------------------------------------
class OpenCodeHomeWidget : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeHomeWidget(QWidget* parent = nullptr);

    // Add a project to the home page.
    void addProject(const QString& name, const QString& path,
                    const QDateTime& lastOpened = {});

    // Add to recently closed list.
    void addRecentlyClosed(const QString& name, const QString& path);

    // Clear all projects.
    void clearProjects();

    // Show the empty state or project list.
    void setEmptyState(bool empty);

signals:
    void projectSelected(const QString& path);
    void newProjectRequested();
    void projectRemoved(const QString& path);

private slots:
    void onSearchTextChanged(const QString& text);
    void onNewProjectClicked();

private:
    void setupUi();
    void updateVisibility();

    // Search bar.
    QLineEdit* m_searchEdit = nullptr;

    // Empty state.
    QWidget* m_emptyState = nullptr;

    // Recent projects section.
    QWidget* m_recentSection = nullptr;
    QLabel* m_recentHeader = nullptr;
    QVBoxLayout* m_recentLayout = nullptr;

    // Recently closed section.
    QWidget* m_closedSection = nullptr;
    QLabel* m_closedHeader = nullptr;
    QVBoxLayout* m_closedLayout = nullptr;

    // New project button.
    QPushButton* m_newProjectBtn = nullptr;

    // Project tracking.
    QList<ProjectCard*> m_recentProjects;
    QList<ProjectCard*> m_closedProjects;
};

#endif // OPENCODE_HOME_WIDGET_H
