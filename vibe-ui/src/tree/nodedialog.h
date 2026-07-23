#pragma once

#include <QDialog>
#include <QJsonObject>
#include <QString>

class QComboBox;
class QLineEdit;
class QPlainTextEdit;
class QSpinBox;

class NodeDialog final : public QDialog
{
    Q_OBJECT

public:
    explicit NodeDialog(
        const QJsonObject &existing,
        const QString &parentId,
        QWidget *parent = nullptr);

    QJsonObject node() const;

private:
    void populate(const QJsonObject &existing);

    QLineEdit *m_titleEdit;
    QComboBox *m_kindCombo;
    QComboBox *m_statusCombo;
    QComboBox *m_ownerCombo;
    QSpinBox *m_prioritySpin;
    QPlainTextEdit *m_descriptionEdit;
    QLineEdit *m_filesEdit;
    QString m_parentId;
};
