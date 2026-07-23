#include "nodedialog.h"

#include <QComboBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QJsonArray>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QSpinBox>
#include <QUuid>

NodeDialog::NodeDialog(
    const QJsonObject &existing,
    const QString &parentId,
    QWidget *parent)
    : QDialog(parent)
    , m_parentId(parentId)
{
    setWindowTitle(existing.isEmpty() ? tr("新增节点") : tr("编辑节点"));
    setMinimumWidth(480);

    auto *layout = new QFormLayout(this);

    m_titleEdit = new QLineEdit(this);
    m_titleEdit->setPlaceholderText(tr("节点标题"));
    layout->addRow(tr("标题"), m_titleEdit);

    m_kindCombo = new QComboBox(this);
    m_kindCombo->addItem(tr("功能"), QStringLiteral("feature"));
    m_kindCombo->addItem(tr("测试"), QStringLiteral("test"));
    m_kindCombo->addItem(tr("修复"), QStringLiteral("fix"));
    m_kindCombo->addItem(tr("重构"), QStringLiteral("refactor"));
    m_kindCombo->addItem(tr("文档"), QStringLiteral("docs"));
    m_kindCombo->addItem(tr("决策"), QStringLiteral("decision"));
    m_kindCombo->addItem(tr("部署"), QStringLiteral("deploy"));
    layout->addRow(tr("类型"), m_kindCombo);

    m_statusCombo = new QComboBox(this);
    m_statusCombo->addItem(tr("待处理"), QStringLiteral("pending"));
    m_statusCombo->addItem(tr("进行中"), QStringLiteral("in_progress"));
    m_statusCombo->addItem(tr("已完成"), QStringLiteral("completed"));
    m_statusCombo->addItem(tr("失败"), QStringLiteral("failed"));
    m_statusCombo->addItem(tr("需审查"), QStringLiteral("needs_review"));
    m_statusCombo->addItem(tr("已阻塞"), QStringLiteral("blocked"));
    m_statusCombo->addItem(tr("已拒绝"), QStringLiteral("rejected"));
    m_statusCombo->addItem(tr("已取消"), QStringLiteral("cancelled"));
    layout->addRow(tr("状态"), m_statusCombo);

    m_ownerCombo = new QComboBox(this);
    m_ownerCombo->addItem(tr("AI"), QStringLiteral("ai"));
    m_ownerCombo->addItem(tr("人"), QStringLiteral("human"));
    layout->addRow(tr("负责人"), m_ownerCombo);

    m_prioritySpin = new QSpinBox(this);
    m_prioritySpin->setRange(0, 10);
    m_prioritySpin->setValue(5);
    layout->addRow(tr("优先级"), m_prioritySpin);

    m_descriptionEdit = new QPlainTextEdit(this);
    m_descriptionEdit->setPlaceholderText(tr("描述或标注（可选）"));
    m_descriptionEdit->setMaximumHeight(120);
    layout->addRow(tr("描述"), m_descriptionEdit);

    m_filesEdit = new QLineEdit(this);
    m_filesEdit->setPlaceholderText(tr("关联文件，逗号分隔（可选）"));
    layout->addRow(tr("文件"), m_filesEdit);

    auto *buttons = new QDialogButtonBox(
        QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    layout->addRow(buttons);

    if (!existing.isEmpty()) {
        populate(existing);
    }
}

void NodeDialog::populate(const QJsonObject &existing)
{
    m_titleEdit->setText(existing.value(QStringLiteral("title")).toString());

    const QString kind = existing.value(QStringLiteral("kind")).toString();
    int kindIndex = m_kindCombo->findData(kind);
    if (kindIndex >= 0) {
        m_kindCombo->setCurrentIndex(kindIndex);
    }

    const QString status = existing.value(QStringLiteral("status")).toString();
    int statusIndex = m_statusCombo->findData(status);
    if (statusIndex >= 0) {
        m_statusCombo->setCurrentIndex(statusIndex);
    }

    const QString owner = existing.value(QStringLiteral("last_modified_by")).toString();
    int ownerIndex = m_ownerCombo->findData(owner);
    if (ownerIndex >= 0) {
        m_ownerCombo->setCurrentIndex(ownerIndex);
    }

    m_prioritySpin->setValue(existing.value(QStringLiteral("priority")).toInt(5));
    m_descriptionEdit->setPlainText(existing.value(QStringLiteral("description")).toString());

    QStringList files;
    for (const QJsonValue &file : existing.value(QStringLiteral("target_files")).toArray()) {
        files.append(file.toString());
    }
    m_filesEdit->setText(files.join(QStringLiteral(", ")));
}

QJsonObject NodeDialog::node() const
{
    const QStringList files = m_filesEdit->text().split(',', Qt::SkipEmptyParts);
    QJsonArray filesArray;
    for (const QString &file : files) {
        const QString trimmed = file.trimmed();
        if (!trimmed.isEmpty()) {
            filesArray.append(trimmed);
        }
    }

    QJsonObject node;
    node.insert(QStringLiteral("title"), m_titleEdit->text().trimmed());
    node.insert(QStringLiteral("kind"), m_kindCombo->currentData().toString());
    node.insert(QStringLiteral("status"), m_statusCombo->currentData().toString());
    node.insert(QStringLiteral("priority"), m_prioritySpin->value());
    node.insert(QStringLiteral("description"), m_descriptionEdit->toPlainText().trimmed());
    node.insert(QStringLiteral("target_files"), filesArray);
    node.insert(QStringLiteral("last_modified_by"), m_ownerCombo->currentData().toString());
    if (!m_parentId.isEmpty()) {
        node.insert(QStringLiteral("parent_id"), m_parentId);
    }
    return node;
}
