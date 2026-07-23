// =============================================================================
// opencode_home_widget.cpp — Home page implementation
// =============================================================================

#include "opencode_home_widget.h"

#include <QMouseEvent>
#include <QFileDialog>
#include <QDir>
#include <QScrollArea>
#include <QGraphicsOpacityEffect>

// =============================================================================
// ProjectCard implementation
// =============================================================================

ProjectCard::ProjectCard(const QString& name, const QString& path,
                         const QDateTime& lastOpened, QWidget* parent)
    : QFrame(parent), m_name(name), m_path(path)
{
    setupUi();
    m_nameLabel->setText(name);
    m_pathLabel->setText(QDir::toNativeSeparators(path));

    if (lastOpened.isValid()) {
        QString timeStr;
        qint64 daysAgo = lastOpened.daysTo(QDateTime::currentDateTime());
        if (daysAgo < 1)
            timeStr = tr("Today");
        else if (daysAgo < 2)
            timeStr = tr("Yesterday");
        else if (daysAgo < 7)
            timeStr = tr("%1 days ago").arg(daysAgo);
        else
            timeStr = lastOpened.toString(QStringLiteral("yyyy-MM-dd"));
        m_timeLabel->setText(timeStr);
    } else {
        m_timeLabel->hide();
    }
}

void ProjectCard::setupUi()
{
    setObjectName(QStringLiteral("ProjectCard"));
    setCursor(Qt::PointingHandCursor);
    setFixedHeight(64);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);

    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(16, 10, 12, 10);
    layout->setSpacing(12);

    // Folder icon.
    m_iconLabel = new QLabel(QStringLiteral("📁"), this);
    m_iconLabel->setObjectName(QStringLiteral("ProjectIcon"));
    m_iconLabel->setFixedWidth(28);
    QFont iconFont = m_iconLabel->font();
    iconFont.setPixelSize(22);
    m_iconLabel->setFont(iconFont);
    m_iconLabel->setAlignment(Qt::AlignCenter);
    layout->addWidget(m_iconLabel);

    // Name + path.
    auto* textLayout = new QVBoxLayout();
    textLayout->setSpacing(2);

    m_nameLabel = new QLabel(this);
    m_nameLabel->setObjectName(QStringLiteral("ProjectName"));
    QFont nameFont = m_nameLabel->font();
    nameFont.setBold(true);
    nameFont.setPixelSize(14);
    m_nameLabel->setFont(nameFont);
    textLayout->addWidget(m_nameLabel);

    m_pathLabel = new QLabel(this);
    m_pathLabel->setObjectName(QStringLiteral("ProjectPath"));
    QFont pathFont = m_pathLabel->font();
    pathFont.setPixelSize(11);
    m_pathLabel->setFont(pathFont);
    textLayout->addWidget(m_pathLabel);

    layout->addLayout(textLayout, 1);

    // Last opened time.
    m_timeLabel = new QLabel(this);
    m_timeLabel->setObjectName(QStringLiteral("ProjectTime"));
    QFont timeFont = m_timeLabel->font();
    timeFont.setPixelSize(11);
    m_timeLabel->setFont(timeFont);
    m_timeLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    layout->addWidget(m_timeLabel);

    // Remove button (hidden by default, shown on hover).
    m_removeBtn = new QPushButton(QStringLiteral("✕"), this);
    m_removeBtn->setObjectName(QStringLiteral("ProjectRemoveBtn"));
    m_removeBtn->setFixedSize(22, 22);
    m_removeBtn->setFlat(true);
    m_removeBtn->setCursor(Qt::PointingHandCursor);
    m_removeBtn->hide();
    connect(m_removeBtn, &QPushButton::clicked, this, [this]() {
        emit removeRequested(m_path);
    });
    layout->addWidget(m_removeBtn);
}

void ProjectCard::mousePressEvent(QMouseEvent* event)
{
    if (event->button() == Qt::LeftButton) {
        emit clicked(m_path);
    }
    QFrame::mousePressEvent(event);
}

void ProjectCard::enterEvent(QEnterEvent* event)
{
    Q_UNUSED(event);
    m_removeBtn->show();
    update();
}

void ProjectCard::leaveEvent(QEvent* event)
{
    Q_UNUSED(event);
    m_removeBtn->hide();
    update();
}

// =============================================================================
// OpenCodeHomeWidget implementation
// =============================================================================

OpenCodeHomeWidget::OpenCodeHomeWidget(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeHomeWidget::setupUi()
{
    setObjectName(QStringLiteral("HomeWidget"));

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // Header area with search + new project button.
    auto* header = new QWidget(this);
    header->setObjectName(QStringLiteral("HomeHeader"));

    auto* headerLayout = new QHBoxLayout(header);
    headerLayout->setContentsMargins(24, 20, 24, 12);
    headerLayout->setSpacing(12);

    auto* titleLabel = new QLabel(tr("Projects"), header);
    titleLabel->setObjectName(QStringLiteral("HomeTitle"));
    QFont titleFont = titleLabel->font();
    titleFont.setBold(true);
    titleFont.setPixelSize(20);
    titleLabel->setFont(titleFont);
    headerLayout->addWidget(titleLabel);

    headerLayout->addStretch();

    m_searchEdit = new QLineEdit(header);
    m_searchEdit->setObjectName(QStringLiteral("HomeSearch"));
    m_searchEdit->setPlaceholderText(tr("Search projects..."));
    m_searchEdit->setClearButtonEnabled(true);
    m_searchEdit->setFixedWidth(240);
    connect(m_searchEdit, &QLineEdit::textChanged, this, &OpenCodeHomeWidget::onSearchTextChanged);
    headerLayout->addWidget(m_searchEdit);

    m_newProjectBtn = new QPushButton(tr("+ New Project"), header);
    m_newProjectBtn->setObjectName(QStringLiteral("HomeNewProjectBtn"));
    m_newProjectBtn->setFixedHeight(36);
    m_newProjectBtn->setMinimumWidth(120);
    m_newProjectBtn->setCursor(Qt::PointingHandCursor);
    connect(m_newProjectBtn, &QPushButton::clicked, this, &OpenCodeHomeWidget::onNewProjectClicked);
    headerLayout->addWidget(m_newProjectBtn);

    mainLayout->addWidget(header);

    // Scrollable content area.
    auto* scrollArea = new QScrollArea(this);
    scrollArea->setObjectName(QStringLiteral("HomeScrollArea"));
    scrollArea->setWidgetResizable(true);
    scrollArea->setFrameShape(QFrame::NoFrame);

    auto* contentWidget = new QWidget(scrollArea);
    auto* contentLayout = new QVBoxLayout(contentWidget);
    contentLayout->setContentsMargins(24, 8, 24, 24);
    contentLayout->setSpacing(8);

    // Empty state.
    m_emptyState = new QWidget(contentWidget);
    m_emptyState->setObjectName(QStringLiteral("HomeEmptyState"));
    auto* emptyLayout = new QVBoxLayout(m_emptyState);
    emptyLayout->setAlignment(Qt::AlignCenter);

    auto* emptyIcon = new QLabel(QStringLiteral("📂"), m_emptyState);
    emptyIcon->setAlignment(Qt::AlignCenter);
    QFont emptyFont = emptyIcon->font();
    emptyFont.setPixelSize(48);
    emptyIcon->setFont(emptyFont);
    emptyLayout->addWidget(emptyIcon);

    auto* emptyTitle = new QLabel(tr("No sessions yet"), m_emptyState);
    emptyTitle->setObjectName(QStringLiteral("HomeEmptyTitle"));
    QFont etFont = emptyTitle->font();
    etFont.setBold(true);
    etFont.setPixelSize(18);
    emptyTitle->setFont(etFont);
    emptyTitle->setAlignment(Qt::AlignCenter);
    emptyLayout->addWidget(emptyTitle);

    auto* emptyText = new QLabel(
        tr("Open a project folder to start a new session, or create a new project."),
        m_emptyState);
    emptyText->setObjectName(QStringLiteral("HomeEmptyText"));
    emptyText->setAlignment(Qt::AlignCenter);
    emptyText->setWordWrap(true);
    emptyLayout->addWidget(emptyText);

    auto* emptyBtn = new QPushButton(tr("Open Project"), m_emptyState);
    emptyBtn->setObjectName(QStringLiteral("HomeEmptyBtn"));
    emptyBtn->setFixedHeight(40);
    emptyBtn->setMinimumWidth(160);
    emptyBtn->setCursor(Qt::PointingHandCursor);
    connect(emptyBtn, &QPushButton::clicked, this, &OpenCodeHomeWidget::onNewProjectClicked);
    emptyLayout->addWidget(emptyBtn);

    m_emptyState->hide();
    contentLayout->addWidget(m_emptyState);

    // Recent projects.
    m_recentSection = new QWidget(contentWidget);
    m_recentSection->setObjectName(QStringLiteral("HomeRecentSection"));
    auto* recentSectionLayout = new QVBoxLayout(m_recentSection);
    recentSectionLayout->setContentsMargins(0, 0, 0, 0);
    recentSectionLayout->setSpacing(6);

    m_recentHeader = new QLabel(tr("Recent"), m_recentSection);
    m_recentHeader->setObjectName(QStringLiteral("HomeSectionHeader"));
    QFont sectionFont = m_recentHeader->font();
    sectionFont.setBold(true);
    sectionFont.setPixelSize(13);
    m_recentHeader->setFont(sectionFont);
    recentSectionLayout->addWidget(m_recentHeader);

    m_recentLayout = new QVBoxLayout();
    m_recentLayout->setSpacing(4);
    recentSectionLayout->addLayout(m_recentLayout);

    contentLayout->addWidget(m_recentSection);

    // Recently closed.
    m_closedSection = new QWidget(contentWidget);
    m_closedSection->setObjectName(QStringLiteral("HomeClosedSection"));
    auto* closedSectionLayout = new QVBoxLayout(m_closedSection);
    closedSectionLayout->setContentsMargins(0, 0, 0, 0);
    closedSectionLayout->setSpacing(6);

    m_closedHeader = new QLabel(tr("Recently Closed"), m_closedSection);
    m_closedHeader->setObjectName(QStringLiteral("HomeSectionHeader"));
    m_closedHeader->setFont(sectionFont);
    closedSectionLayout->addWidget(m_closedHeader);

    m_closedLayout = new QVBoxLayout();
    m_closedLayout->setSpacing(4);
    closedSectionLayout->addLayout(m_closedLayout);

    contentLayout->addWidget(m_closedSection);

    contentLayout->addStretch();

    scrollArea->setWidget(contentWidget);
    mainLayout->addWidget(scrollArea, 1);

    // Start in empty state.
    setEmptyState(true);
}

void OpenCodeHomeWidget::addProject(const QString& name, const QString& path,
                                      const QDateTime& lastOpened)
{
    auto* card = new ProjectCard(name, path, lastOpened, m_recentSection);
    connect(card, &ProjectCard::clicked, this, &OpenCodeHomeWidget::projectSelected);
    connect(card, &ProjectCard::removeRequested, this, [this](const QString& p) {
        addRecentlyClosed(QString(), p);
        emit projectRemoved(p);
    });

    m_recentLayout->addWidget(card);
    m_recentProjects.append(card);
    setEmptyState(false);
    updateVisibility();
}

void OpenCodeHomeWidget::addRecentlyClosed(const QString& name, const QString& path)
{
    QString displayName = name.isEmpty() ? QDir(path).dirName() : name;
    auto* card = new ProjectCard(displayName, path, {}, m_closedSection);
    connect(card, &ProjectCard::clicked, this, &OpenCodeHomeWidget::projectSelected);
    m_closedLayout->addWidget(card);
    m_closedProjects.append(card);
    m_closedSection->setVisible(true);
}

void OpenCodeHomeWidget::clearProjects()
{
    for (auto* card : m_recentProjects) {
        m_recentLayout->removeWidget(card);
        card->deleteLater();
    }
    m_recentProjects.clear();

    for (auto* card : m_closedProjects) {
        m_closedLayout->removeWidget(card);
        card->deleteLater();
    }
    m_closedProjects.clear();

    setEmptyState(true);
}

void OpenCodeHomeWidget::setEmptyState(bool empty)
{
    m_emptyState->setVisible(empty);
    m_recentSection->setVisible(!empty && !m_recentProjects.isEmpty());
    m_closedSection->setVisible(!m_closedProjects.isEmpty());
    m_searchEdit->setEnabled(!empty);
}

void OpenCodeHomeWidget::updateVisibility()
{
    bool hasRecent = !m_recentProjects.isEmpty();
    bool hasClosed = !m_closedProjects.isEmpty();
    m_recentSection->setVisible(hasRecent);
    m_closedSection->setVisible(hasClosed);
    m_emptyState->setVisible(!hasRecent && !hasClosed);
}

void OpenCodeHomeWidget::onSearchTextChanged(const QString& text)
{
    for (auto* card : m_recentProjects) {
        bool visible = text.isEmpty()
            || card->projectName().contains(text, Qt::CaseInsensitive)
            || card->projectPath().contains(text, Qt::CaseInsensitive);
        card->setVisible(visible);
    }
    for (auto* card : m_closedProjects) {
        bool visible = text.isEmpty()
            || card->projectName().contains(text, Qt::CaseInsensitive)
            || card->projectPath().contains(text, Qt::CaseInsensitive);
        card->setVisible(visible);
    }
}

void OpenCodeHomeWidget::onNewProjectClicked()
{
    QString dir = QFileDialog::getExistingDirectory(
        this, tr("Open Project Folder"), {},
        QFileDialog::ShowDirsOnly | QFileDialog::DontResolveSymlinks);

    if (!dir.isEmpty()) {
        emit newProjectRequested();
        emit projectSelected(dir);
    }
}
