#pragma once

#include <QHash>
#include <QJsonObject>
#include <QList>
#include <QPointer>
#include <QSet>
#include <QWidget>

class QJsonValue;
class QScrollArea;

class AppSchemaRenderer final : public QWidget
{
    Q_OBJECT

public:
    explicit AppSchemaRenderer(QWidget *parent = nullptr);

    bool setSchema(const QJsonObject &schema, QString *error = nullptr);
    void applyPatch(const QJsonObject &patch);
    void clear();

signals:
    void uiEvent(const QJsonObject &event);

private:
    static constexpr int MaxNodes = 128;
    static constexpr int MaxDepth = 8;
    static constexpr int MaxTextLength = 16384;

    bool validateSchema(const QJsonObject &schema, QJsonObject *root,
                        QString *error) const;
    bool validateNode(const QJsonObject &node, int depth, int &nodeCount,
                      QSet<QString> &ids, QString *error) const;
    QWidget *buildNode(const QJsonObject &node, QWidget *parent);
    QWidget *buildContainer(const QJsonObject &node, QWidget *parent,
                            const QString &type);
    QWidget *buildUnsupported(const QJsonObject &node, QWidget *parent,
                              const QString &type);
    void registerControl(const QJsonObject &node, QWidget *widget);
    void addToFocusChain(QWidget *widget);
    void applySemanticSize(QWidget *widget, const QString &size,
                           bool interactive) const;
    void applySinglePatch(const QJsonObject &patch);
    void patchWidget(QWidget *widget, const QJsonObject &changes);
    void emitEvent(const QString &id, const QString &eventName,
                   const QJsonValue &value);
    static QString displayValue(const QJsonValue &value);

    QScrollArea *m_scrollArea;
    QHash<QString, QPointer<QWidget>> m_controls;
    QList<QPointer<QWidget>> m_focusChain;
};
