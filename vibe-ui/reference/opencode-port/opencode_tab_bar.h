// =============================================================================
// opencode_tab_bar.h — Chrome-style draggable tab bar
//
// Replicates opencode desktop v2's tab system:
//   - Draggable tabs with close buttons
//   - Hover preview popover (project/path/branch info)
//   - Add tab (+) button
//   - Fading overflow (gradient fade at edges instead of clipping)
//   - Mod+1 through Mod+9 shortcuts
//   - Unread/activity indicator on tabs with pending work
//   - Home tab (fixed, always first)
//
// opencode design: Tabs sit in a horizontal chrome bar at the top of the
// window, styled with the muted surface color (#161b22) and bordered below.
// Active tab is slightly brighter; inactive tabs are muted.
//
// Qt6 implementation: QTabBar subclass with custom paint, drag support,
// and QShortcut bindings.
// =============================================================================

#ifndef OPENCODE_TAB_BAR_H
#define OPENCODE_TAB_BAR_H

#include <QTabBar>
#include <QWidget>
#include <QMap>
#include <QShortcut>
#include <QTimer>
#include <QLabel>
#include <QFrame>
#include <QPushButton>
#include <QMouseEvent>
#include <QPaintEvent>
#include <QEnterEvent>

class OpenCodeTabBar;

// ---------------------------------------------------------------------------
// OpenCodeTabPreview — Hover popover showing project/path/branch info
// ---------------------------------------------------------------------------
class OpenCodeTabPreview : public QFrame
{
    Q_OBJECT
public:
    explicit OpenCodeTabPreview(QWidget* parent = nullptr);

    void setProjectName(const QString& name);
    void setProjectPath(const QString& path);
    void setBranch(const QString& branch);
    void setLastOpened(const QString& time);

    void showAt(const QPoint& globalPos);
    void hidePreview();

protected:
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    void setupUi();
    QLabel* m_projectName = nullptr;
    QLabel* m_projectPath = nullptr;
    QLabel* m_branchLabel = nullptr;
    QLabel* m_timeLabel = nullptr;
    QTimer* m_hideTimer = nullptr;
};

// ---------------------------------------------------------------------------
// OpenCodeTabBar — The main tab bar widget
// ---------------------------------------------------------------------------
class OpenCodeTabBar : public QTabBar
{
    Q_OBJECT

public:
    explicit OpenCodeTabBar(QWidget* parent = nullptr);

    // Tab management.
    int addSessionTab(const QString& title, const QString& projectPath = {},
                      const QString& branch = {});
    int homeTabIndex() const { return m_homeTabIndex; }

    // Set tab metadata for hover preview.
    void setTabProject(int index, const QString& name, const QString& path, const QString& branch);
    void setTabLastOpened(int index, const QString& time);

    // Activity indicator (unread/pending).
    void setTabHasActivity(int index, bool hasActivity);
    void clearActivity(int index);

    // Access tab data.
    QString tabTitle(int index) const;
    QString tabProjectPath(int index) const;

    // Keyboard shortcuts: Mod+1 through Mod+9, Mod+T, Mod+N.
    void installTabShortcuts(QWidget* parent);

signals:
    void newTabRequested();
    void tabCloseRequested(int index);
    void homeTabActivated();

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void paintEvent(QPaintEvent* event) override;
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;
    bool event(QEvent* event) override;

private slots:
    void onHoverTimer();
    void onTabMoved(int from, int to);

private:
    void setupUi();
    void updateFadeEdges();

    // Tab metadata.
    struct TabInfo {
        QString title;
        QString projectName;
        QString projectPath;
        QString branch;
        QString lastOpened;
        bool hasActivity = false;
    };
    QMap<int, TabInfo> m_tabInfo;

    // Preview popover.
    OpenCodeTabPreview* m_preview = nullptr;
    QTimer* m_hoverTimer = nullptr;
    int m_hoveredIndex = -1;

    // Drag support.
    bool m_dragging = false;
    QPoint m_dragStartPos;
    int m_dragTabIndex = -1;

    // Home tab (fixed first position).
    int m_homeTabIndex = -1;

    // Fade edges (overlay widgets).
    QWidget* m_leftFade = nullptr;
    QWidget* m_rightFade = nullptr;

    // Add button.
    QPushButton* m_addButton = nullptr;
};

#endif // OPENCODE_TAB_BAR_H
