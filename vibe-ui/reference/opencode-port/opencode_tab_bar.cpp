// =============================================================================
// opencode_tab_bar.cpp — Chrome-style tab bar implementation
// =============================================================================

#include "opencode_tab_bar.h"

#include <QPainter>
#include <QStyleOption>
#include <QStyle>
#include <QApplication>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGraphicsDropShadowEffect>
#include <QCursor>
#include <QToolTip>

// =============================================================================
// OpenCodeTabPreview implementation
// =============================================================================

OpenCodeTabPreview::OpenCodeTabPreview(QWidget* parent)
    : QFrame(parent, Qt::ToolTip | Qt::FramelessWindowHint)
{
    setupUi();
}

void OpenCodeTabPreview::setupUi()
{
    setObjectName(QStringLiteral("TabPreview"));
    setFixedWidth(280);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setAttribute(Qt::WA_TransparentForMouseEvents, false);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(16, 12, 16, 12);
    layout->setSpacing(4);

    m_projectName = new QLabel(this);
    m_projectName->setObjectName(QStringLiteral("TabPreviewName"));
    QFont nameFont = m_projectName->font();
    nameFont.setBold(true);
    nameFont.setPixelSize(14);
    m_projectName->setFont(nameFont);
    layout->addWidget(m_projectName);

    m_projectPath = new QLabel(this);
    m_projectPath->setObjectName(QStringLiteral("TabPreviewPath"));
    QFont pathFont = m_projectPath->font();
    pathFont.setPixelSize(11);
    m_projectPath->setFont(pathFont);
    m_projectPath->setWordWrap(true);
    layout->addWidget(m_projectPath);

    m_branchLabel = new QLabel(this);
    m_branchLabel->setObjectName(QStringLiteral("TabPreviewBranch"));
    QFont branchFont = m_branchLabel->font();
    branchFont.setPixelSize(11);
    branchFont.setFamily(QStringLiteral("Consolas"));
    m_branchLabel->setFont(branchFont);
    layout->addWidget(m_branchLabel);

    m_timeLabel = new QLabel(this);
    m_timeLabel->setObjectName(QStringLiteral("TabPreviewTime"));
    QFont timeFont = m_timeLabel->font();
    timeFont.setPixelSize(11);
    m_timeLabel->setFont(timeFont);
    layout->addWidget(m_timeLabel);

    hide();

    m_hideTimer = new QTimer(this);
    m_hideTimer->setSingleShot(true);
    m_hideTimer->setInterval(200);
    connect(m_hideTimer, &QTimer::timeout, this, &OpenCodeTabPreview::hidePreview);
}

void OpenCodeTabPreview::setProjectName(const QString& name)
{
    m_projectName->setText(name);
}

void OpenCodeTabPreview::setProjectPath(const QString& path)
{
    m_projectPath->setText(path);
}

void OpenCodeTabPreview::setBranch(const QString& branch)
{
    if (branch.isEmpty())
        m_branchLabel->hide();
    else {
        m_branchLabel->setText(QStringLiteral("⎇ ") + branch);
        m_branchLabel->show();
    }
}

void OpenCodeTabPreview::setLastOpened(const QString& time)
{
    m_timeLabel->setText(time);
}

void OpenCodeTabPreview::showAt(const QPoint& globalPos)
{
    m_hideTimer->stop();
    QPoint pos = globalPos + QPoint(0, 8);
    move(pos);
    adjustSize();
    show();
    raise();
}

void OpenCodeTabPreview::hidePreview()
{
    hide();
}

void OpenCodeTabPreview::enterEvent(QEnterEvent* event)
{
    Q_UNUSED(event);
    m_hideTimer->stop();
}

void OpenCodeTabPreview::leaveEvent(QEvent* event)
{
    Q_UNUSED(event);
    m_hideTimer->start();
}

// =============================================================================
// OpenCodeTabBar implementation
// =============================================================================

OpenCodeTabBar::OpenCodeTabBar(QWidget* parent)
    : QTabBar(parent)
{
    setupUi();
}

void OpenCodeTabBar::setupUi()
{
    setObjectName(QStringLiteral("OpenCodeTabBar"));
    setTabsClosable(true);
    setMovable(true);
    setUsesScrollButtons(true);
    setElideMode(Qt::ElideRight);
    setExpanding(false);
    setDrawBase(false);
    setDocumentMode(true);

    // opencode: Minimum tap target size for touch.
    setMinimumHeight(40);
    setCursor(Qt::ArrowCursor);

    // Preview popover.
    m_preview = new OpenCodeTabPreview(window());
    m_preview->hide();

    // Hover timer for preview popover (400ms delay, like opencode).
    m_hoverTimer = new QTimer(this);
    m_hoverTimer->setSingleShot(true);
    m_hoverTimer->setInterval(400);
    connect(m_hoverTimer, &QTimer::timeout, this, &OpenCodeTabBar::onHoverTimer);

    // Track tab moves.
    connect(this, &QTabBar::tabMoved, this, &OpenCodeTabBar::onTabMoved);

    // Set size policy.
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);

    // Add home tab (always first, fixed).
    m_homeTabIndex = addTab(QStringLiteral("⌂"));
    setTabToolTip(m_homeTabIndex, tr("Home"));
    setTabData(m_homeTabIndex, QStringLiteral("__home__"));
}

void OpenCodeTabBar::installTabShortcuts(QWidget* parent)
{
    // Mod+1 through Mod+9: switch to tab 1-9.
    for (int i = 0; i < 9; ++i) {
        auto* shortcut = new QShortcut(
            QKeySequence(QStringLiteral("Ctrl+%1").arg(i + 1)), parent);
        int tabIdx = i;
        connect(shortcut, &QShortcut::activated, this, [this, tabIdx]() {
            int idx = m_homeTabIndex + 1 + tabIdx; // Skip home tab.
            if (idx < count())
                setCurrentIndex(idx);
        });
    }

    // Mod+T: open new tab.
    auto* newTabShortcut = new QShortcut(QKeySequence(QStringLiteral("Ctrl+T")), parent);
    connect(newTabShortcut, &QShortcut::activated, this, &OpenCodeTabBar::newTabRequested);

    // Mod+N: new tab (alternative).
    auto* newTabShortcut2 = new QShortcut(QKeySequence(QStringLiteral("Ctrl+N")), parent);
    connect(newTabShortcut2, &QShortcut::activated, this, &OpenCodeTabBar::newTabRequested);
}

int OpenCodeTabBar::addSessionTab(const QString& title, const QString& projectPath,
                                   const QString& branch)
{
    int idx = addTab(title);
    TabInfo info;
    info.title = title;
    info.projectPath = projectPath;
    info.branch = branch;
    m_tabInfo[idx] = info;

    setTabToolTip(idx, projectPath.isEmpty() ? title : projectPath);
    setTabData(idx, projectPath);
    return idx;
}

void OpenCodeTabBar::setTabProject(int index, const QString& name, const QString& path,
                                    const QString& branch)
{
    if (!m_tabInfo.contains(index)) {
        TabInfo info;
        m_tabInfo[index] = info;
    }
    m_tabInfo[index].projectName = name;
    m_tabInfo[index].projectPath = path;
    m_tabInfo[index].branch = branch;
}

void OpenCodeTabBar::setTabLastOpened(int index, const QString& time)
{
    if (m_tabInfo.contains(index))
        m_tabInfo[index].lastOpened = time;
}

void OpenCodeTabBar::setTabHasActivity(int index, bool hasActivity)
{
    if (m_tabInfo.contains(index))
        m_tabInfo[index].hasActivity = hasActivity;
    update(); // Trigger repaint to show/hide indicator.
}

void OpenCodeTabBar::clearActivity(int index)
{
    setTabHasActivity(index, false);
}

QString OpenCodeTabBar::tabTitle(int index) const
{
    if (m_tabInfo.contains(index))
        return m_tabInfo[index].title;
    return tabText(index);
}

QString OpenCodeTabBar::tabProjectPath(int index) const
{
    if (m_tabInfo.contains(index))
        return m_tabInfo[index].projectPath;
    return tabData(index).toString();
}

// ---------------------------------------------------------------------------
// Mouse events — drag support and close handling
// ---------------------------------------------------------------------------

void OpenCodeTabBar::mousePressEvent(QMouseEvent* event)
{
    int idx = tabAt(event->pos());
    if (idx >= 0 && event->button() == Qt::LeftButton) {
        // QTabBar with setTabsClosable(true) handles close button clicks
        // automatically via tabCloseRequested signal.
        // Only block closing the home tab.
        if (idx == m_homeTabIndex) {
            // Check if this is a click near the right edge (close button area).
            QRect tabRect = this->tabRect(idx);
            int closeMargin = 30; // Right-side close button area.
            if (event->pos().x() > tabRect.right() - closeMargin) {
                event->ignore();
                return;
            }
        }
    }

    if (idx >= 0 && event->button() == Qt::LeftButton && idx != m_homeTabIndex) {
        m_dragging = true;
        m_dragStartPos = event->pos();
        m_dragTabIndex = idx;
    }

    QTabBar::mousePressEvent(event);
}

void OpenCodeTabBar::mouseMoveEvent(QMouseEvent* event)
{
    if (m_dragging) {
        // opencode: Drag tabs to reorder. QTabBar handles this with setMovable(true).
    }

    // Track hover for preview popover.
    int idx = tabAt(event->pos());
    if (idx != m_hoveredIndex) {
        m_hoveredIndex = idx;
        m_preview->hidePreview();
        m_hoverTimer->stop();

        if (idx >= 0 && idx != m_homeTabIndex && m_tabInfo.contains(idx)) {
            m_hoverTimer->start();
        }
    }

    QTabBar::mouseMoveEvent(event);
}

void OpenCodeTabBar::mouseReleaseEvent(QMouseEvent* event)
{
    m_dragging = false;
    m_dragTabIndex = -1;
    QTabBar::mouseReleaseEvent(event);
}

// ---------------------------------------------------------------------------
// Hover preview popover
// ---------------------------------------------------------------------------

void OpenCodeTabBar::onHoverTimer()
{
    if (m_hoveredIndex < 0 || m_hoveredIndex == m_homeTabIndex
        || !m_tabInfo.contains(m_hoveredIndex))
        return;

    const auto& info = m_tabInfo[m_hoveredIndex];
    if (info.projectPath.isEmpty()) return;

    m_preview->setProjectName(info.projectName.isEmpty() ? info.title : info.projectName);
    m_preview->setProjectPath(info.projectPath);
    m_preview->setBranch(info.branch);
    m_preview->setLastOpened(info.lastOpened);

    QPoint tabCenter = tabRect(m_hoveredIndex).center();
    QPoint globalPos = mapToGlobal(QPoint(tabCenter.x(), height()));
    m_preview->showAt(globalPos);
}

void OpenCodeTabBar::enterEvent(QEnterEvent* event)
{
    Q_UNUSED(event);
    m_hoveredIndex = -1;
    QTabBar::enterEvent(event);
}

void OpenCodeTabBar::leaveEvent(QEvent* event)
{
    m_hoveredIndex = -1;
    m_hoverTimer->stop();
    m_preview->hidePreview();
    QTabBar::leaveEvent(event);
}

// ---------------------------------------------------------------------------
// Event filter for close button clicks
// ---------------------------------------------------------------------------

bool OpenCodeTabBar::event(QEvent* event)
{
    if (event->type() == QEvent::MouseButtonRelease) {
        auto* me = static_cast<QMouseEvent*>(event);
        if (me->button() == Qt::MiddleButton) {
            int idx = tabAt(me->pos());
            if (idx >= 0 && idx != m_homeTabIndex) {
                emit tabCloseRequested(idx);
                return true;
            }
        }
    }
    return QTabBar::event(event);
}

// ---------------------------------------------------------------------------
// Tab close handling
// ---------------------------------------------------------------------------

// OpenCodeTabBar intercepts tabCloseRequested to prevent closing the home tab.
// The QTabBar base class handles the actual close button clicks.

void OpenCodeTabBar::onTabMoved(int from, int to)
{
    // Home tab must stay first.
    if (from == m_homeTabIndex || to == 0) {
        // Block home tab from moving.
        // In a full implementation, we'd move it back.
        // For the demo, we accept the limitation.
    }
}

// ---------------------------------------------------------------------------
// Custom paint — activity indicators and opencode styling
// ---------------------------------------------------------------------------

void OpenCodeTabBar::paintEvent(QPaintEvent* event)
{
    QTabBar::paintEvent(event);

    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    // Draw activity indicators (small dot on tabs with pending work).
    for (int i = 0; i < count(); ++i) {
        if (i == m_homeTabIndex) continue;
        if (!m_tabInfo.contains(i) || !m_tabInfo[i].hasActivity) continue;

        QRect tabRect = this->tabRect(i);
        // Small blue dot at top-right of tab.
        QPoint dotCenter(tabRect.right() - 10, tabRect.top() + 8);
        painter.setBrush(QColor(QStringLiteral("#58a6ff")));
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(dotCenter, 4, 4);
    }
}

// ---------------------------------------------------------------------------
// Fade edges (not fully implemented — reserved for future)
// ---------------------------------------------------------------------------

void OpenCodeTabBar::updateFadeEdges()
{
    // opencode desktop v2: When tabs overflow horizontally, the edges
    // fade with a gradient instead of being clipped. This would be
    // implemented with QLinearGradient overlay widgets.
}
