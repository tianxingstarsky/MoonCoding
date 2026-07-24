#include "treemodel.h"

#include <QJsonArray>
#include <QPainter>
#include <QPixmap>
#include <QSet>
#include <QStringList>
#include <map>

TreeModel::TreeModel(QObject *parent)
    : QAbstractItemModel(parent)
    , m_root(std::make_unique<Item>())
{
    m_liveItems.insert(m_root.get());
}

TreeModel::~TreeModel() = default;

QModelIndex TreeModel::index(int row, int column, const QModelIndex &parentIndex) const
{
    if (row < 0 || column < 0 || column >= ColumnCount
        || (parentIndex.isValid() && parentIndex.column() != 0)) {
        return {};
    }
    Item *parentItem = itemFromIndex(parentIndex);
    if (!parentItem || row >= static_cast<int>(parentItem->children.size())) {
        return {};
    }
    return createIndex(row, column, parentItem->children.at(row).get());
}

QModelIndex TreeModel::parent(const QModelIndex &child) const
{
    if (!child.isValid()) {
        return {};
    }
    Item *childItem = itemFromIndex(child);
    if (!childItem || !childItem->parent || childItem->parent == m_root.get()) {
        return {};
    }
    return indexForItem(childItem->parent);
}

int TreeModel::rowCount(const QModelIndex &parentIndex) const
{
    if (parentIndex.column() > 0) {
        return 0;
    }
    const Item *item = itemFromIndex(parentIndex);
    return item ? static_cast<int>(item->children.size()) : 0;
}

int TreeModel::columnCount(const QModelIndex &) const
{
    return ColumnCount;
}

QVariant TreeModel::data(const QModelIndex &modelIndex, int role) const
{
    if (!modelIndex.isValid()) {
        return {};
    }
    const Item *item = itemFromIndex(modelIndex);
    if (!item) {
        return {};
    }
    const QJsonObject &node = item->node;
    const QString status = node.value(QStringLiteral("status")).toString(QStringLiteral("pending"));

    if (role == NodeIdRole) {
        return node.value(QStringLiteral("id")).toString();
    }
    if (role == NodeObjectRole) {
        return QVariant::fromValue(node);
    }
    if (role == StatusRole) {
        return status;
    }
    if (role == OwnerRole) {
        return ownerLabel(node);
    }
    if (role == Qt::DecorationRole && modelIndex.column() == StatusColumn) {
        return statusIcon(status);
    }
    if (role == Qt::ToolTipRole) {
        QStringList details;
        const QString description = node.value(QStringLiteral("description")).toString();
        const QString humanNote = node.value(QStringLiteral("human_note")).toString();
        const QString aiNote = node.value(QStringLiteral("ai_note")).toString();
        if (!description.isEmpty()) {
            details << description;
        }
        if (!humanNote.isEmpty()) {
            details << tr("人工: %1").arg(humanNote);
        }
        if (!aiNote.isEmpty()) {
            details << tr("AI: %1").arg(aiNote);
        }
        details << tr("修订 %1").arg(node.value(QStringLiteral("revision")).toInteger());
        return details.join(QStringLiteral("\n\n"));
    }
    if (role == Qt::ForegroundRole && modelIndex.column() == StatusColumn) {
        return statusColor(status);
    }
    if (role != Qt::DisplayRole) {
        return {};
    }

    switch (modelIndex.column()) {
    case StatusColumn:
        return statusLabel(status);
    case TitleColumn:
        return node.value(QStringLiteral("title")).toString();
    case KindColumn:
        return node.value(QStringLiteral("kind")).toString().replace('_', ' ');
    case PriorityColumn:
        return node.value(QStringLiteral("priority")).toInt();
    case OwnerColumn:
        return ownerLabel(node);
    case FilesColumn:
        return node.value(QStringLiteral("target_files")).toArray().size();
    default:
        return {};
    }
}

QVariant TreeModel::headerData(int section, Qt::Orientation orientation, int role) const
{
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole) {
        return {};
    }
    switch (section) {
    case StatusColumn:
        return tr("状态");
    case TitleColumn:
        return tr("工作项");
    case KindColumn:
        return tr("类型");
    case PriorityColumn:
        return tr("优先级");
    case OwnerColumn:
        return tr("负责人");
    case FilesColumn:
        return tr("文件");
    default:
        return {};
    }
}

Qt::ItemFlags TreeModel::flags(const QModelIndex &modelIndex) const
{
    return modelIndex.isValid()
        ? Qt::ItemIsEnabled | Qt::ItemIsSelectable
        : Qt::NoItemFlags;
}

QHash<int, QByteArray> TreeModel::roleNames() const
{
    auto roles = QAbstractItemModel::roleNames();
    roles.insert(NodeIdRole, "nodeId");
    roles.insert(NodeObjectRole, "node");
    roles.insert(StatusRole, "status");
    roles.insert(OwnerRole, "owner");
    return roles;
}

void TreeModel::setTree(const QJsonObject &tree)
{
    beginResetModel();
    m_tree = tree;
    m_root = std::make_unique<Item>();
    m_itemsById.clear();
    m_liveItems.clear();
    m_liveItems.insert(m_root.get());

    std::map<QString, std::unique_ptr<Item>> pool;
    QStringList insertionOrder;
    QHash<QString, QString> parentIds;
    const QJsonArray nodes = tree.value(QStringLiteral("nodes")).toArray();
    for (const QJsonValue &value : nodes) {
        if (!value.isObject()) {
            continue;
        }
        const QJsonObject node = value.toObject();
        const QString id = node.value(QStringLiteral("id")).toString();
        if (id.isEmpty() || pool.find(id) != pool.end()) {
            continue;
        }
        auto item = std::make_unique<Item>();
        item->node = node;
        m_itemsById.insert(id, item.get());
        m_liveItems.insert(item.get());
        parentIds.insert(id, node.value(QStringLiteral("parent_id")).toString());
        pool.emplace(id, std::move(item));
        insertionOrder.append(id);
    }

    for (const QString &id : insertionOrder) {
        auto found = pool.find(id);
        if (found == pool.end()) {
            continue;
        }
        std::unique_ptr<Item> item = std::move(found->second);
        pool.erase(found);
        const QString parentId = item->node.value(QStringLiteral("parent_id")).toString();
        bool cyclic = false;
        QSet<QString> ancestors{id};
        QString ancestorId = parentId;
        while (!ancestorId.isEmpty()) {
            if (ancestors.contains(ancestorId)) {
                cyclic = true;
                break;
            }
            ancestors.insert(ancestorId);
            ancestorId = parentIds.value(ancestorId);
        }
        Item *parentItem = (parentId.isEmpty() || cyclic)
            ? m_root.get()
            : m_itemsById.value(parentId, nullptr);
        if (!parentItem || parentItem == item.get() || !ownsItem(parentItem)) {
            parentItem = m_root.get();
        }
        item->parent = parentItem;
        parentItem->children.push_back(std::move(item));
    }
    endResetModel();
}

QJsonObject TreeModel::tree() const
{
    return m_tree;
}

QJsonObject TreeModel::nodeObject(const QModelIndex &modelIndex) const
{
    const Item *item = itemFromIndex(modelIndex);
    return item ? item->node : QJsonObject{};
}

QString TreeModel::nodeId(const QModelIndex &modelIndex) const
{
    return nodeObject(modelIndex).value(QStringLiteral("id")).toString();
}

quint64 TreeModel::version() const
{
    return static_cast<quint64>(m_tree.value(QStringLiteral("version")).toInteger());
}

QModelIndex TreeModel::indexForId(const QString &id) const
{
    return indexForItem(m_itemsById.value(id, nullptr));
}

bool TreeModel::ownsItem(const Item *item) const
{
    return item && m_liveItems.contains(const_cast<Item *>(item));
}

TreeModel::Item *TreeModel::itemFromIndex(const QModelIndex &modelIndex) const
{
    if (!modelIndex.isValid()) {
        return m_root.get();
    }
    auto *item = static_cast<Item *>(modelIndex.internalPointer());
    // Stale indexes after model reset must not be dereferenced.
    return ownsItem(item) ? item : nullptr;
}

QModelIndex TreeModel::indexForItem(const Item *item, int column) const
{
    if (!ownsItem(item) || !item->parent || item == m_root.get() || !ownsItem(item->parent)) {
        return {};
    }
    const auto &siblings = item->parent->children;
    for (int row = 0; row < static_cast<int>(siblings.size()); ++row) {
        if (siblings.at(row).get() == item) {
            return createIndex(row, column, const_cast<Item *>(item));
        }
    }
    return {};
}

QString TreeModel::statusLabel(const QString &status)
{
    static const QHash<QString, QString> labels{
        {QStringLiteral("pending"), tr("待处理")},
        {QStringLiteral("in_progress"), tr("进行中")},
        {QStringLiteral("completed"), tr("已完成")},
        {QStringLiteral("failed"), tr("失败")},
        {QStringLiteral("needs_review"), tr("需审查")},
        {QStringLiteral("blocked"), tr("已阻塞")},
        {QStringLiteral("rejected"), tr("已拒绝")},
        {QStringLiteral("cancelled"), tr("已取消")},
    };
    return labels.value(status, status);
}

QColor TreeModel::statusColor(const QString &status)
{
    static const QHash<QString, QColor> colors{
        {QStringLiteral("pending"), QColor(QStringLiteral("#7b8498"))},
        {QStringLiteral("in_progress"), QColor(QStringLiteral("#5b9cff"))},
        {QStringLiteral("completed"), QColor(QStringLiteral("#45c486"))},
        {QStringLiteral("failed"), QColor(QStringLiteral("#f06473"))},
        {QStringLiteral("needs_review"), QColor(QStringLiteral("#e7b75f"))},
        {QStringLiteral("blocked"), QColor(QStringLiteral("#e88955"))},
        {QStringLiteral("rejected"), QColor(QStringLiteral("#c574d8"))},
        {QStringLiteral("cancelled"), QColor(QStringLiteral("#5f6675"))},
    };
    return colors.value(status, QColor(QStringLiteral("#7b8498")));
}

QString TreeModel::ownerLabel(const QJsonObject &node)
{
    const QString actor = node.value(QStringLiteral("last_modified_by")).toString();
    if (actor == QStringLiteral("human")) {
        return tr("人工");
    }
    if (actor == QStringLiteral("ai")) {
        return tr("AI");
    }
    return tr("系统");
}

QIcon TreeModel::statusIcon(const QString &status)
{
    static QHash<QString, QIcon> cache;
    const auto it = cache.constFind(status);
    if (it != cache.cend()) {
        return it.value();
    }
    QPixmap pixmap(16, 16);
    pixmap.fill(Qt::transparent);
    {
        QPainter painter(&pixmap);
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setPen(Qt::NoPen);
        painter.setBrush(statusColor(status));
        painter.drawEllipse(QRectF(3, 3, 10, 10));
    }
    const QIcon icon(pixmap);
    cache.insert(status, icon);
    return icon;
}
