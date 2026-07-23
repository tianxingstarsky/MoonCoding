#include "tree/treemodel.h"
#include "input/inputwidget.h"

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTextEdit>
#include <QtTest>

class TreeModelTest final : public QObject
{
    Q_OBJECT

private slots:
    void buildsHierarchy();
    void exposesOwnershipAndStatus();
    void restoresIndexesByStableId();
    void rejectsCyclicPresentationData();
    void rejectsChildrenForNonzeroParentColumn();
    void preservesDraftUntilSubmissionIsAccepted();
};

namespace {
QJsonObject node(
    const QString &id,
    const QString &parentId,
    const QString &title,
    const QString &status = QStringLiteral("pending"))
{
    return QJsonObject{
        {QStringLiteral("id"), id},
        {QStringLiteral("parent_id"), parentId.isEmpty() ? QJsonValue() : QJsonValue(parentId)},
        {QStringLiteral("title"), title},
        {QStringLiteral("description"), QString()},
        {QStringLiteral("kind"), QStringLiteral("task")},
        {QStringLiteral("status"), status},
        {QStringLiteral("priority"), 50},
        {QStringLiteral("created_by"), QStringLiteral("ai")},
        {QStringLiteral("last_modified_by"), QStringLiteral("ai")},
        {QStringLiteral("human_locked_fields"), QJsonArray{}},
        {QStringLiteral("target_files"), QJsonArray{}},
        {QStringLiteral("revision"), 1},
    };
}
}

void TreeModelTest::buildsHierarchy()
{
    TreeModel model;
    model.setTree(QJsonObject{
        {QStringLiteral("version"), 2},
        {QStringLiteral("nodes"), QJsonArray{
            node(QStringLiteral("root"), {}, QStringLiteral("Project")),
            node(QStringLiteral("child"), QStringLiteral("root"), QStringLiteral("Feature")),
        }},
    });

    QCOMPARE(model.rowCount(), 1);
    const QModelIndex root = model.index(0, 0);
    QCOMPARE(model.data(model.index(0, TreeModel::TitleColumn)).toString(), QStringLiteral("Project"));
    QCOMPARE(model.rowCount(root), 1);
    const QModelIndex child = model.index(0, TreeModel::TitleColumn, root);
    QCOMPARE(model.data(child).toString(), QStringLiteral("Feature"));
    QCOMPARE(model.parent(child), root);
    QCOMPARE(model.version(), quint64(2));
}

void TreeModelTest::exposesOwnershipAndStatus()
{
    TreeModel model;
    QJsonObject humanNode = node(
        QStringLiteral("test"),
        {},
        QStringLiteral("Linux test"),
        QStringLiteral("failed"));
    humanNode.insert(QStringLiteral("last_modified_by"), QStringLiteral("human"));
    model.setTree(QJsonObject{
        {QStringLiteral("version"), 3},
        {QStringLiteral("nodes"), QJsonArray{humanNode}},
    });

    const QModelIndex status = model.index(0, TreeModel::StatusColumn);
    QCOMPARE(model.data(status, TreeModel::StatusRole).toString(), QStringLiteral("failed"));
    QCOMPARE(
        model.data(status, TreeModel::OwnerRole).toString(),
        QCoreApplication::translate("TreeModel", "人工"));
}

void TreeModelTest::restoresIndexesByStableId()
{
    TreeModel model;
    model.setTree(QJsonObject{
        {QStringLiteral("version"), 1},
        {QStringLiteral("nodes"), QJsonArray{
            node(QStringLiteral("root"), {}, QStringLiteral("Project")),
            node(QStringLiteral("child"), QStringLiteral("root"), QStringLiteral("Feature")),
        }},
    });

    const QModelIndex child = model.indexForId(QStringLiteral("child"));
    QVERIFY(child.isValid());
    QCOMPARE(model.nodeId(child), QStringLiteral("child"));
    QCOMPARE(model.nodeObject(child).value(QStringLiteral("title")).toString(), QStringLiteral("Feature"));
}

void TreeModelTest::rejectsCyclicPresentationData()
{
    TreeModel model;
    model.setTree(QJsonObject{
        {QStringLiteral("version"), 7},
        {QStringLiteral("nodes"), QJsonArray{
            node(QStringLiteral("a"), QStringLiteral("b"), QStringLiteral("A")),
            node(QStringLiteral("b"), QStringLiteral("a"), QStringLiteral("B")),
        }},
    });

    QCOMPARE(model.rowCount(), 2);
    QVERIFY(model.indexForId(QStringLiteral("a")).isValid());
    QVERIFY(model.indexForId(QStringLiteral("b")).isValid());
}

void TreeModelTest::rejectsChildrenForNonzeroParentColumn()
{
    TreeModel model;
    model.setTree(QJsonObject{
        {QStringLiteral("version"), 2},
        {QStringLiteral("nodes"), QJsonArray{
            node(QStringLiteral("root"), {}, QStringLiteral("Project")),
            node(QStringLiteral("child"), QStringLiteral("root"), QStringLiteral("Feature")),
        }},
    });

    // StatusColumn is 0; use TitleColumn as a non-zero parent column.
    const QModelIndex nonzeroParent = model.index(0, TreeModel::TitleColumn);
    QVERIFY(nonzeroParent.isValid());
    QVERIFY(nonzeroParent.column() != 0);
    QVERIFY(!model.index(0, TreeModel::TitleColumn, nonzeroParent).isValid());
}

void TreeModelTest::preservesDraftUntilSubmissionIsAccepted()
{
    InputWidget input;
    input.setBackendReady(true);
    auto *editor = input.findChild<QTextEdit *>(QStringLiteral("promptEditor"));
    QVERIFY(editor);
    editor->setPlainText(QStringLiteral("Keep this draft"));
    QSignalSpy submitted(&input, &InputWidget::messageSubmitted);

    QVERIFY(QMetaObject::invokeMethod(&input, "submit", Qt::DirectConnection));
    QCOMPARE(submitted.count(), 1);
    QCOMPARE(editor->toPlainText(), QStringLiteral("Keep this draft"));

    input.clearDraft();
    QVERIFY(editor->toPlainText().isEmpty());
}

QTEST_MAIN(TreeModelTest)
#include "treemodel_test.moc"
