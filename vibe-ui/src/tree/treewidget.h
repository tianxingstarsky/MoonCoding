#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QWidget>

class QAction;
class QPlainTextEdit;
class QSplitter;
class QPoint;
class QToolBar;
class QTreeView;
class TreeModel;

class TreeWidget final : public QWidget
{
    Q_OBJECT

public:
    explicit TreeWidget(QWidget *parent = nullptr);

    TreeModel *model() const;

public slots:
    void setTree(const QJsonObject &tree);
    void setAgentBusy(bool busy);
    void setBackendReady(bool ready);
    void applyWidth(int width);

signals:
    void addRequested(const QJsonObject &node, quint64 expectedVersion);
    void updateRequested(
        const QString &nodeId,
        const QJsonObject &patch,
        quint64 expectedVersion);
    void deleteRequested(const QString &nodeId, quint64 expectedVersion);
    void reviewNodeRequested(const QString &nodeId);
    void reviewAllRequested();
    void refreshRequested();

private slots:
    void showContextMenu(const QPoint &position);
    void updateDetails();
    void addNode();
    void editSelectedNode();
    void deleteSelectedNode();
    void reviewSelectedNode();

private:
    QJsonObject selectedNode() const;
    void requestStatusChange(const QString &status);
    void updateActionStates();
    void applyColumnStrategy(bool compact);
    void applyTreeNow(const QJsonObject &tree);
    void scheduleDetailsUpdate();

    TreeModel *m_model;
    QTreeView *m_view;
    QPlainTextEdit *m_details;
    QToolBar *m_toolbar;
    QSplitter *m_splitter;
    QAction *m_addAction;
    QAction *m_editAction;
    QAction *m_reviewNodeAction;
    QAction *m_reviewAllAction;
    QAction *m_refreshAction;
    QJsonObject m_pendingTree;
    bool m_hasPendingTree = false;
    bool m_applyTreeScheduled = false;
    bool m_detailsUpdateScheduled = false;
    bool m_agentBusy = false;
    bool m_backendReady = false;
    bool m_compactLayout = false;
    bool m_didInitialExpand = false;
};
