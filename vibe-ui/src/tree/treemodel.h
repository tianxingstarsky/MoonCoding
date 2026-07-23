#pragma once

#include <QAbstractItemModel>
#include <QColor>
#include <QHash>
#include <QIcon>
#include <QJsonObject>
#include <QSet>
#include <memory>
#include <vector>

class TreeModel final : public QAbstractItemModel
{
    Q_OBJECT

public:
    enum Column {
        StatusColumn,
        TitleColumn,
        KindColumn,
        PriorityColumn,
        OwnerColumn,
        FilesColumn,
        ColumnCount,
    };

    enum Role {
        NodeIdRole = Qt::UserRole + 1,
        NodeObjectRole,
        StatusRole,
        OwnerRole,
    };

    explicit TreeModel(QObject *parent = nullptr);
    ~TreeModel() override;

    QModelIndex index(int row, int column, const QModelIndex &parent = {}) const override;
    QModelIndex parent(const QModelIndex &child) const override;
    int rowCount(const QModelIndex &parent = {}) const override;
    int columnCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
    QVariant headerData(
        int section,
        Qt::Orientation orientation,
        int role = Qt::DisplayRole) const override;
    Qt::ItemFlags flags(const QModelIndex &index) const override;
    QHash<int, QByteArray> roleNames() const override;

    void setTree(const QJsonObject &tree);
    QJsonObject tree() const;
    QJsonObject nodeObject(const QModelIndex &index) const;
    QString nodeId(const QModelIndex &index) const;
    quint64 version() const;
    QModelIndex indexForId(const QString &id) const;

private:
    struct Item {
        QJsonObject node;
        Item *parent = nullptr;
        std::vector<std::unique_ptr<Item>> children;
    };

    Item *itemFromIndex(const QModelIndex &index) const;
    QModelIndex indexForItem(const Item *item, int column = 0) const;
    bool ownsItem(const Item *item) const;
    static QString statusLabel(const QString &status);
    static QColor statusColor(const QString &status);
    static QString ownerLabel(const QJsonObject &node);
    static QIcon statusIcon(const QString &status);

    std::unique_ptr<Item> m_root;
    QJsonObject m_tree;
    QHash<QString, Item *> m_itemsById;
    // Rejects dangling QModelIndex::internalPointer after beginResetModel().
    QSet<Item *> m_liveItems;
};
