// =============================================================================
// opencode_session_view.h — Main session view (conversation + review panel)
//
// Replicates opencode desktop v2's session layout:
//   - QSplitter with conversation (left) + review panel (right)
//   - Conversation: OpenCodeChatWidget (top) + OpenCodeComposer (bottom)
//   - Review panel: OpenCodeDiffView (file diffs) + OpenCodeFileTree (sidebar)
//   - Tab-aware: stores/restores state per session
//   - Jump-to-latest button when scrolled up
//   - Timeline: scrollable session history
//
// opencode design: The session view is the main work area. It uses a
// horizontal split with the conversation on the left and supporting
// panels (file tree, review) on the right. The composer sits at the
// bottom of the conversation area.
// =============================================================================

#ifndef OPENCODE_SESSION_VIEW_H
#define OPENCODE_SESSION_VIEW_H

#include <QWidget>
#include <QSplitter>
#include <QVBoxLayout>
#include <QPushButton>
#include <QLabel>
#include <QTimer>

class OpenCodeChatWidget;
class OpenCodeComposer;
class OpenCodeDiffView;
class OpenCodeFileTree;
class OpenCodeToolBlock;
class MessageBlock;

// ---------------------------------------------------------------------------
// OpenCodeSessionView — Full session workspace
// ---------------------------------------------------------------------------
class OpenCodeSessionView : public QWidget
{
    Q_OBJECT

public:
    explicit OpenCodeSessionView(QWidget* parent = nullptr);

    // Access sub-widgets for programmatic control.
    OpenCodeChatWidget* chatWidget() const { return m_chatWidget; }
    OpenCodeComposer* composer() const { return m_composer; }
    OpenCodeDiffView* diffView() const { return m_diffView; }
    OpenCodeFileTree* fileTree() const { return m_fileTree; }

    // Session identity.
    void setSessionName(const QString& name);
    QString sessionName() const;

    // Set project root path (for file tree).
    void setProjectPath(const QString& path);

    // Toggle panels.
    void setFileTreeVisible(bool visible);
    void setReviewPanelVisible(bool visible);
    bool isFileTreeVisible() const;
    bool isReviewPanelVisible() const;

    // Save/restore session state for tab switching.
    void saveState(QVariantMap& state) const;
    void restoreState(const QVariantMap& state);

    // Jump-to-latest button (floating, shown when scrolled up).
    void showJumpToLatest(bool show);

signals:
    void fileTreeToggled(bool visible);
    void reviewPanelToggled(bool visible);
    void jumpToLatestRequested();

private slots:
    void onUserScrolledUp();
    void onJumpToLatestClicked();
    void onFileActivated(const QString& filePath);

private:
    void setupUi();

    QString m_sessionName;

    // Main widgets.
    OpenCodeChatWidget* m_chatWidget = nullptr;
    OpenCodeComposer* m_composer = nullptr;

    // Right panel widgets.
    OpenCodeDiffView* m_diffView = nullptr;
    OpenCodeFileTree* m_fileTree = nullptr;

    // Layout.
    QSplitter* m_mainSplitter = nullptr;     // conversation | review
    QSplitter* m_reviewSplitter = nullptr;   // file tree | diff view

    // Jump-to-latest button (floating).
    QPushButton* m_jumpToLatestBtn = nullptr;
};

#endif // OPENCODE_SESSION_VIEW_H
