// =============================================================================
// opencode_session_view.cpp — Session view implementation
// =============================================================================

#include "opencode_session_view.h"
#include "opencode_chat_widget.h"
#include "opencode_composer.h"
#include "opencode_diff_view.h"
#include "opencode_file_tree.h"

#include <QVariantMap>
#include <QFileInfo>
#include <QDir>
#include <QFile>

OpenCodeSessionView::OpenCodeSessionView(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeSessionView::setupUi()
{
    setObjectName(QStringLiteral("SessionView"));

    auto* outerLayout = new QVBoxLayout(this);
    outerLayout->setContentsMargins(0, 0, 0, 0);
    outerLayout->setSpacing(0);

    // -----------------------------------------------------------------------
    // Main horizontal split: conversation (left) | review panel (right)
    // -----------------------------------------------------------------------
    m_mainSplitter = new QSplitter(Qt::Horizontal, this);
    m_mainSplitter->setObjectName(QStringLiteral("SessionSplitter"));
    m_mainSplitter->setHandleWidth(1);

    // --- Left: conversation area ---
    auto* conversationPanel = new QWidget(m_mainSplitter);
    conversationPanel->setObjectName(QStringLiteral("ConversationPanel"));

    auto* convLayout = new QVBoxLayout(conversationPanel);
    convLayout->setContentsMargins(0, 0, 0, 0);
    convLayout->setSpacing(0);

    // Chat widget (scrollable message history).
    m_chatWidget = new OpenCodeChatWidget(conversationPanel);
    m_chatWidget->setObjectName(QStringLiteral("SessionChatWidget"));
    convLayout->addWidget(m_chatWidget, 1);

    // Composer (input area at bottom).
    m_composer = new OpenCodeComposer(conversationPanel);
    convLayout->addWidget(m_composer);

    m_mainSplitter->addWidget(conversationPanel);

    // --- Right: review panel (file tree + diff view) ---
    auto* reviewPanel = new QWidget(m_mainSplitter);
    reviewPanel->setObjectName(QStringLiteral("ReviewPanel"));

    m_reviewSplitter = new QSplitter(Qt::Vertical, reviewPanel);
    m_reviewSplitter->setObjectName(QStringLiteral("ReviewSplitter"));
    m_reviewSplitter->setHandleWidth(1);

    // File tree (top section of review panel).
    m_fileTree = new OpenCodeFileTree(m_reviewSplitter);
    m_fileTree->setObjectName(QStringLiteral("SessionFileTree"));
    m_reviewSplitter->addWidget(m_fileTree);

    // Diff view (bottom section of review panel).
    m_diffView = new OpenCodeDiffView(m_reviewSplitter);
    m_diffView->setObjectName(QStringLiteral("SessionDiffView"));
    m_reviewSplitter->addWidget(m_diffView);

    // Set default ratios: file tree 30%, diff view 70%.
    m_reviewSplitter->setStretchFactor(0, 3);
    m_reviewSplitter->setStretchFactor(1, 7);

    auto* reviewLayout = new QVBoxLayout(reviewPanel);
    reviewLayout->setContentsMargins(0, 0, 0, 0);
    reviewLayout->addWidget(m_reviewSplitter);

    m_mainSplitter->addWidget(reviewPanel);

    // Set default ratios: conversation 60%, review 40%.
    m_mainSplitter->setStretchFactor(0, 6);
    m_mainSplitter->setStretchFactor(1, 4);

    outerLayout->addWidget(m_mainSplitter, 1);

    // -----------------------------------------------------------------------
    // Jump-to-latest button (floating, shown when scrolled up)
    // -----------------------------------------------------------------------
    m_jumpToLatestBtn = new QPushButton(QStringLiteral("↓"), this);
    m_jumpToLatestBtn->setObjectName(QStringLiteral("JumpToLatestBtn"));
    m_jumpToLatestBtn->setFixedSize(36, 36);
    m_jumpToLatestBtn->setToolTip(tr("Jump to latest"));
    m_jumpToLatestBtn->setCursor(Qt::PointingHandCursor);
    m_jumpToLatestBtn->hide();
    connect(m_jumpToLatestBtn, &QPushButton::clicked,
            this, &OpenCodeSessionView::onJumpToLatestClicked);

    // Connect chat scroll signal.
    connect(m_chatWidget, &OpenCodeChatWidget::userScrolledUp,
            this, &OpenCodeSessionView::onUserScrolledUp);

    // Connect file tree to diff view.
    connect(m_fileTree, &OpenCodeFileTree::fileActivated,
            this, &OpenCodeSessionView::onFileActivated);
}

// =============================================================================
// Public API
// =============================================================================

void OpenCodeSessionView::setSessionName(const QString& name)
{
    m_sessionName = name;
}

QString OpenCodeSessionView::sessionName() const
{
    return m_sessionName;
}

void OpenCodeSessionView::setProjectPath(const QString& path)
{
    m_fileTree->setRootPath(path);
}

void OpenCodeSessionView::setFileTreeVisible(bool visible)
{
    m_fileTree->setVisible(visible);
    emit fileTreeToggled(visible);
}

void OpenCodeSessionView::setReviewPanelVisible(bool visible)
{
    // Hide the entire right panel.
    for (int i = 0; i < m_mainSplitter->count(); ++i) {
        QWidget* w = m_mainSplitter->widget(i);
        if (w->objectName() == QStringLiteral("ReviewPanel")) {
            w->setVisible(visible);
            break;
        }
    }
    emit reviewPanelToggled(visible);
}

bool OpenCodeSessionView::isFileTreeVisible() const
{
    return m_fileTree->isVisible();
}

bool OpenCodeSessionView::isReviewPanelVisible() const
{
    for (int i = 0; i < m_mainSplitter->count(); ++i) {
        QWidget* w = m_mainSplitter->widget(i);
        if (w->objectName() == QStringLiteral("ReviewPanel")) {
            return w->isVisible();
        }
    }
    return false;
}

void OpenCodeSessionView::showJumpToLatest(bool show)
{
    if (show) {
        // Position the button at the bottom-center of the chat widget.
        QPoint btnPos(m_chatWidget->width() / 2 - 18,
                      m_chatWidget->height() - 50);
        m_jumpToLatestBtn->move(m_chatWidget->mapTo(this, btnPos));
        m_jumpToLatestBtn->raise();
    }
    m_jumpToLatestBtn->setVisible(show);
}

void OpenCodeSessionView::saveState(QVariantMap& state) const
{
    state[QStringLiteral("sessionName")] = m_sessionName;
    state[QStringLiteral("fileTreeVisible")] = isFileTreeVisible();
    state[QStringLiteral("reviewPanelVisible")] = isReviewPanelVisible();

    // Save splitter sizes.
    QVariantList mainSizes;
    for (int s : m_mainSplitter->sizes())
        mainSizes.append(s);
    state[QStringLiteral("mainSplitterSizes")] = mainSizes;

    QVariantList reviewSizes;
    for (int s : m_reviewSplitter->sizes())
        reviewSizes.append(s);
    state[QStringLiteral("reviewSplitterSizes")] = reviewSizes;

    // Save composer draft.
    QVariantMap draft;
    m_composer->saveDraft(draft);
    state[QStringLiteral("composerDraft")] = draft;
}

void OpenCodeSessionView::restoreState(const QVariantMap& state)
{
    if (state.contains(QStringLiteral("sessionName")))
        m_sessionName = state[QStringLiteral("sessionName")].toString();

    if (state.contains(QStringLiteral("fileTreeVisible")))
        setFileTreeVisible(state[QStringLiteral("fileTreeVisible")].toBool());

    if (state.contains(QStringLiteral("reviewPanelVisible")))
        setReviewPanelVisible(state[QStringLiteral("reviewPanelVisible")].toBool());

    if (state.contains(QStringLiteral("mainSplitterSizes"))) {
        QVariantList sizes = state[QStringLiteral("mainSplitterSizes")].toList();
        QList<int> intSizes;
        for (const auto& s : sizes)
            intSizes.append(s.toInt());
        if (!intSizes.isEmpty())
            m_mainSplitter->setSizes(intSizes);
    }

    if (state.contains(QStringLiteral("reviewSplitterSizes"))) {
        QVariantList sizes = state[QStringLiteral("reviewSplitterSizes")].toList();
        QList<int> intSizes;
        for (const auto& s : sizes)
            intSizes.append(s.toInt());
        if (!intSizes.isEmpty())
            m_reviewSplitter->setSizes(intSizes);
    }

    if (state.contains(QStringLiteral("composerDraft"))) {
        m_composer->restoreDraft(state[QStringLiteral("composerDraft")].toMap());
    }
}

// =============================================================================
// Slots
// =============================================================================

void OpenCodeSessionView::onUserScrolledUp()
{
    showJumpToLatest(true);
}

void OpenCodeSessionView::onJumpToLatestClicked()
{
    m_chatWidget->scrollToBottom();
    showJumpToLatest(false);
    emit jumpToLatestRequested();
}

void OpenCodeSessionView::onFileActivated(const QString& filePath)
{
    QFileInfo fi(filePath);
    if (fi.exists() && fi.isFile()) {
        QFile f(filePath);
        if (f.open(QFile::ReadOnly | QFile::Text)) {
            QString content = QString::fromUtf8(f.readAll());
            m_diffView->openFile(filePath, content);
            f.close();
        }
    }
}
