#include "treewidget.h"

#include "nodedialog.h"
#include "treemodel.h"

#include <QHBoxLayout>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QMenu>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QSignalBlocker>
#include <QSplitter>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QTreeView>
#include <QVBoxLayout>

TreeWidget::TreeWidget(QWidget *parent)
    : QWidget(parent)
    , m_model(new TreeModel(this))
    , m_view(new QTreeView(this))
    , m_details(new QPlainTextEdit(this))
    , m_splitter(new QSplitter(Qt::Horizontal, this))
{
    auto *outer = new QVBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(0);

    auto *toolbar = new QToolBar(tr("项目树工具"), this);
    toolbar->setIconSize(QSize(22, 22));
    toolbar->setObjectName(QStringLiteral("treeToolbar"));
    m_addAction = toolbar->addAction(tr("新增"), this, &TreeWidget::addNode);
    m_editAction = toolbar->addAction(tr("编辑"), this, &TreeWidget::editSelectedNode);
    m_reviewNodeAction = toolbar->addAction(tr("审视节点"), this, &TreeWidget::reviewSelectedNode);
    m_reviewAllAction = toolbar->addAction(tr("审视全部"), this, [this] { emit reviewAllRequested(); });
    m_refreshAction = toolbar->addAction(tr("刷新"), this, [this] { emit refreshRequested(); });

    m_view->setModel(m_model);
    m_view->setObjectName(QStringLiteral("projectTree"));
    m_view->setExpandsOnDoubleClick(true);
    m_view->setContextMenuPolicy(Qt::CustomContextMenu);
    m_view->setSelectionMode(QAbstractItemView::SingleSelection);
    m_view->setAlternatingRowColors(true);
    m_view->setIndentation(22);
    m_view->setHeaderHidden(false);
    m_view->setFocusPolicy(Qt::StrongFocus);
    m_view->setUniformRowHeights(true);
    m_view->header()->setStretchLastSection(false);
    m_view->header()->setSectionResizeMode(TreeModel::TitleColumn, QHeaderView::Stretch);
    m_view->header()->setSectionResizeMode(TreeModel::StatusColumn, QHeaderView::ResizeToContents);
    m_view->header()->setSectionResizeMode(TreeModel::KindColumn, QHeaderView::ResizeToContents);
    m_view->header()->setSectionResizeMode(TreeModel::PriorityColumn, QHeaderView::ResizeToContents);
    m_view->header()->setSectionResizeMode(TreeModel::OwnerColumn, QHeaderView::ResizeToContents);
    m_view->header()->setSectionResizeMode(TreeModel::FilesColumn, QHeaderView::ResizeToContents);

    m_details->setObjectName(QStringLiteral("nodeDetails"));
    m_details->setReadOnly(true);
    m_details->setFrameShape(QFrame::NoFrame);
    m_details->setFocusPolicy(Qt::NoFocus);
    m_details->setPlaceholderText(tr("点选左侧节点查看详情"));

    m_splitter->addWidget(m_view);
    m_splitter->addWidget(m_details);
    m_splitter->setSizes({420, 260});

    outer->addWidget(toolbar);
    outer->addWidget(m_splitter, 1);

    connect(m_view, &QTreeView::customContextMenuRequested, this, &TreeWidget::showContextMenu);
    if (m_view->selectionModel()) {
        connect(m_view->selectionModel(), &QItemSelectionModel::currentChanged, this,
                [this](const QModelIndex &current, const QModelIndex &) {
                    Q_UNUSED(current);
                    scheduleDetailsUpdate();
                    updateActionStates();
                });
    }

    updateActionStates();
}

void TreeWidget::setTree(const QJsonObject &tree)
{
    // Coalesce rapid TreeUpdated events during agent streaming so we never
    // reset the model mid-click / mid-paint with a dangling QModelIndex.
    m_pendingTree = tree;
    m_hasPendingTree = true;
    // While the agent is busy, only stash — applying beginResetModel during
    // token/tool storms crashes linuxfb Qt.
    if (m_agentBusy) {
        return;
    }
    if (m_applyTreeScheduled) {
        return;
    }
    m_applyTreeScheduled = true;
    QTimer::singleShot(0, this, [this] {
        m_applyTreeScheduled = false;
        if (!m_hasPendingTree) {
            return;
        }
        m_hasPendingTree = false;
        applyTreeNow(m_pendingTree);
        m_pendingTree = QJsonObject{};
    });
}

void TreeWidget::applyTreeNow(const QJsonObject &tree)
{
    QString selectedId;
    m_view->setUpdatesEnabled(false);
    if (m_view && m_view->selectionModel()) {
        selectedId = m_model->nodeId(m_view->currentIndex());
        const QSignalBlocker blocker(m_view->selectionModel());
        m_view->selectionModel()->clear();
        m_view->setCurrentIndex({});
    }

    m_model->setTree(tree);

    if (!m_view) {
        return;
    }
    // expandToDepth on every stream update is expensive and crashy on linuxfb.
    if (!m_didInitialExpand) {
        m_view->expandToDepth(0);
        m_didInitialExpand = true;
    }
    if (!selectedId.isEmpty()) {
        const QModelIndex restored = m_model->indexForId(selectedId);
        if (restored.isValid()) {
            const QSignalBlocker blocker(m_view->selectionModel());
            m_view->setCurrentIndex(restored);
            m_view->scrollTo(restored);
        }
    }
    m_view->setUpdatesEnabled(true);
    scheduleDetailsUpdate();
    updateActionStates();
}

void TreeWidget::scheduleDetailsUpdate()
{
    if (m_detailsUpdateScheduled) {
        return;
    }
    m_detailsUpdateScheduled = true;
    QTimer::singleShot(0, this, [this] {
        m_detailsUpdateScheduled = false;
        updateDetails();
    });
}

void TreeWidget::setAgentBusy(bool busy)
{
    const bool wasBusy = m_agentBusy;
    m_agentBusy = busy;
    updateActionStates();
    if (wasBusy && !busy && m_hasPendingTree && !m_applyTreeScheduled) {
        m_applyTreeScheduled = true;
        QTimer::singleShot(0, this, [this] {
            m_applyTreeScheduled = false;
            if (!m_hasPendingTree) {
                return;
            }
            m_hasPendingTree = false;
            applyTreeNow(m_pendingTree);
            m_pendingTree = QJsonObject{};
        });
    }
}

void TreeWidget::setBackendReady(bool ready)
{
    m_backendReady = ready;
    updateActionStates();
}

void TreeWidget::updateActionStates()
{
    const bool idle = m_backendReady && !m_agentBusy;
    const bool hasSelection = !selectedNode().isEmpty();
    m_addAction->setEnabled(idle);
    m_editAction->setEnabled(idle && hasSelection);
    m_reviewNodeAction->setEnabled(idle && hasSelection);
    m_reviewAllAction->setEnabled(idle);
    m_refreshAction->setEnabled(m_backendReady && !m_agentBusy);
}

void TreeWidget::showContextMenu(const QPoint &position)
{
    const QModelIndex index = m_view->indexAt(position);
    if (index.isValid()) {
        m_view->setCurrentIndex(index);
    }

    QMenu menu(this);
    const bool idle = m_backendReady && !m_agentBusy;
    menu.addAction(tr("新增子节点"), this, &TreeWidget::addNode)->setEnabled(idle);
    menu.addAction(tr("编辑节点"), this, &TreeWidget::editSelectedNode)->setEnabled(
        index.isValid() && idle);

    QMenu *statusMenu = menu.addMenu(tr("设置状态"));
    const QList<QPair<QString, QString>> statuses{
        {tr("待处理"), QStringLiteral("pending")},
        {tr("进行中"), QStringLiteral("in_progress")},
        {tr("已完成"), QStringLiteral("completed")},
        {tr("失败"), QStringLiteral("failed")},
        {tr("需审查"), QStringLiteral("needs_review")},
        {tr("已阻塞"), QStringLiteral("blocked")},
        {tr("已拒绝"), QStringLiteral("rejected")},
        {tr("已取消"), QStringLiteral("cancelled")},
    };
    for (const auto &[label, value] : statuses) {
        statusMenu->addAction(label, this, [this, value] { requestStatusChange(value); });
    }
    statusMenu->setEnabled(index.isValid() && idle);

    menu.addSeparator();
    menu.addAction(tr("审视此节点"), this, &TreeWidget::reviewSelectedNode)->setEnabled(
        index.isValid() && idle);
    menu.addSeparator();
    menu.addAction(tr("删除分支"), this, &TreeWidget::deleteSelectedNode)->setEnabled(
        index.isValid() && idle);
    menu.exec(m_view->viewport()->mapToGlobal(position));
}

void TreeWidget::updateDetails()
{
    if (!m_details) {
        return;
    }
    const QJsonObject node = selectedNode();
    if (node.isEmpty()) {
        m_details->clear();
        return;
    }

    QStringList lines;
    lines << node.value(QStringLiteral("title")).toString();
    lines << QString();
    lines << tr("状态：%1")
                 .arg(node.value(QStringLiteral("status")).toString().replace(QLatin1Char('_'), QLatin1Char(' ')));
    lines << tr("类型：%1").arg(node.value(QStringLiteral("kind")).toString());
    lines << tr("优先级：%1").arg(node.value(QStringLiteral("priority")).toInt());

    const QString description = node.value(QStringLiteral("description")).toString().trimmed();
    if (!description.isEmpty()) {
        lines << QString();
        lines << description;
    }
    const QString humanNote = node.value(QStringLiteral("human_note")).toString().trimmed();
    if (!humanNote.isEmpty()) {
        lines << QString();
        lines << tr("人工指令：");
        lines << humanNote;
    }
    const QString aiNote = node.value(QStringLiteral("ai_note")).toString().trimmed();
    if (!aiNote.isEmpty()) {
        lines << QString();
        lines << tr("AI 标注：");
        lines << aiNote;
    }
    QStringList files;
    for (const QJsonValue &value : node.value(QStringLiteral("target_files")).toArray()) {
        const QString f = value.toString().trimmed();
        if (!f.isEmpty()) {
            files.append(f);
        }
    }
    if (!files.isEmpty()) {
        lines << QString();
        lines << tr("关联文件：");
        lines.append(files);
    }
    const QJsonArray evidence = node.value(QStringLiteral("evidence")).toArray();
    if (!evidence.isEmpty()) {
        lines << QString();
        lines << tr("验证证据：");
        int n = 0;
        for (const QJsonValue &value : evidence) {
            if (++n > 8) {
                lines << tr("…其余 %1 条已省略").arg(evidence.size() - 8);
                break;
            }
            const QJsonObject item = value.toObject();
            lines << QStringLiteral("- %1 %2")
                         .arg(item.value(QStringLiteral("success")).toBool() ? tr("[通过]") : tr("[失败]"),
                              item.value(QStringLiteral("summary")).toString());
            const QString cmd = item.value(QStringLiteral("command")).toString().trimmed();
            if (!cmd.isEmpty()) {
                lines << QStringLiteral("  %1").arg(cmd.left(160));
            }
        }
    }
    m_details->setPlainText(lines.join(QLatin1Char('\n')));
}

void TreeWidget::addNode()
{
    if (m_agentBusy) {
        return;
    }
    const QString parentId = m_model->nodeId(m_view->currentIndex());
    NodeDialog dialog({}, parentId, this);
    if (dialog.exec() == QDialog::Accepted) {
        emit addRequested(dialog.node(), m_model->version());
    }
}

void TreeWidget::editSelectedNode()
{
    if (m_agentBusy) {
        return;
    }
    const QJsonObject existing = selectedNode();
    if (existing.isEmpty()) {
        return;
    }
    const QString parentId = existing.value(QStringLiteral("parent_id")).toString();
    NodeDialog dialog(existing, parentId, this);
    if (dialog.exec() != QDialog::Accepted) {
        return;
    }
    const QJsonObject edited = dialog.node();
    QJsonObject patch;
    for (auto it = edited.begin(); it != edited.end(); ++it) {
        if (existing.value(it.key()) != it.value()) {
            patch.insert(it.key(), it.value());
        }
    }
    if (!patch.isEmpty()) {
        emit updateRequested(
            existing.value(QStringLiteral("id")).toString(),
            patch,
            m_model->version());
    }
}

void TreeWidget::deleteSelectedNode()
{
    const QJsonObject node = selectedNode();
    if (node.isEmpty() || m_agentBusy) {
        return;
    }
    if (QMessageBox::question(
            this,
            tr("删除分支"),
            tr("确定删除「%1」及其所有子节点？").arg(node.value(QStringLiteral("title")).toString()))
        == QMessageBox::Yes) {
        emit deleteRequested(
            node.value(QStringLiteral("id")).toString(),
            m_model->version());
    }
}

void TreeWidget::reviewSelectedNode()
{
    const QString id = m_model->nodeId(m_view->currentIndex());
    if (!id.isEmpty() && !m_agentBusy) {
        emit reviewNodeRequested(id);
    }
}

QJsonObject TreeWidget::selectedNode() const
{
    return m_model->nodeObject(m_view->currentIndex());
}

void TreeWidget::requestStatusChange(const QString &status)
{
    const QJsonObject node = selectedNode();
    if (node.isEmpty() || m_agentBusy) {
        return;
    }
    emit updateRequested(
        node.value(QStringLiteral("id")).toString(),
        QJsonObject{{QStringLiteral("status"), status}},
        m_model->version());
}

void TreeWidget::applyWidth(int width)
{
    const bool compact = width < 720;
    applyColumnStrategy(compact);
    if (!m_splitter) {
        return;
    }
    if (width < 520) {
        m_splitter->setOrientation(Qt::Vertical);
        m_splitter->setSizes({240, 180});
    } else {
        m_splitter->setOrientation(Qt::Horizontal);
        m_splitter->setSizes({qMax(220, width * 3 / 5), qMax(160, width * 2 / 5)});
    }
}

void TreeWidget::applyColumnStrategy(bool compact)
{
    if (compact == m_compactLayout) {
        return;
    }
    m_compactLayout = compact;
    m_view->setHeaderHidden(compact);
    m_view->setColumnHidden(TreeModel::KindColumn, compact);
    m_view->setColumnHidden(TreeModel::PriorityColumn, compact);
    m_view->setColumnHidden(TreeModel::OwnerColumn, compact);
    m_view->setColumnHidden(TreeModel::FilesColumn, compact);
    if (compact) {
        m_view->header()->setSectionResizeMode(TreeModel::TitleColumn, QHeaderView::Stretch);
        m_view->header()->setSectionResizeMode(TreeModel::StatusColumn, QHeaderView::ResizeToContents);
    } else {
        m_view->header()->setSectionResizeMode(TreeModel::TitleColumn, QHeaderView::Stretch);
        m_view->header()->setSectionResizeMode(TreeModel::StatusColumn, QHeaderView::ResizeToContents);
        m_view->header()->setSectionResizeMode(TreeModel::KindColumn, QHeaderView::ResizeToContents);
        m_view->header()->setSectionResizeMode(TreeModel::PriorityColumn, QHeaderView::ResizeToContents);
        m_view->header()->setSectionResizeMode(TreeModel::OwnerColumn, QHeaderView::ResizeToContents);
        m_view->header()->setSectionResizeMode(TreeModel::FilesColumn, QHeaderView::ResizeToContents);
    }
}
