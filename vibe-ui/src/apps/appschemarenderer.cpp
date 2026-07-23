#include "appschemarenderer.h"

#include <QBoxLayout>
#include <QCheckBox>
#include <QCoreApplication>
#include <QFont>
#include <QFrame>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QLabel>
#include <QLocale>
#include <QProgressBar>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSizePolicy>
#include <QSlider>
#include <QStringList>
#include <QVBoxLayout>

#include <cmath>
#include <limits>
#include <utility>

namespace {
constexpr char ControlTypeProperty[] = "_mooncodingControlType";
constexpr char EventValueProperty[] = "_mooncodingEventValue";

bool failValidation(QString *error, const QString &message)
{
    if (error) {
        *error = message;
    }
    return false;
}

bool isScalar(const QJsonValue &value)
{
    return !value.isArray() && !value.isObject();
}

bool jsonInteger(const QJsonValue &value, int *result)
{
    if (!value.isDouble()) {
        return false;
    }

    const double number = value.toDouble();
    if (!std::isfinite(number) || std::floor(number) != number
        || number < std::numeric_limits<int>::min()
        || number > std::numeric_limits<int>::max()) {
        return false;
    }

    if (result) {
        *result = static_cast<int>(number);
    }
    return true;
}

int patchedInteger(const QJsonValue &value, int fallback)
{
    int result = fallback;
    return jsonInteger(value, &result) ? result : fallback;
}

/// LLM-authored schemas often use `kind` instead of `type`. Normalize once.
/// Also strip absolute layout keys — models keep emitting x/y/width/height and
/// that must not brick the whole app UI (calculator footgun).
QJsonObject normalizeControlNode(QJsonObject node)
{
    if (!node.contains(QStringLiteral("type"))
        && node.contains(QStringLiteral("kind"))
        && node.value(QStringLiteral("kind")).isString()) {
        node.insert(QStringLiteral("type"), node.value(QStringLiteral("kind")));
    }

    if (node.value(QStringLiteral("type")).toString() == QStringLiteral("value")
        && !node.contains(QStringLiteral("value"))
        && node.contains(QStringLiteral("text"))) {
        node.insert(QStringLiteral("value"), node.value(QStringLiteral("text")));
    }

    static const QStringList coordinateKeys = {
        QStringLiteral("x"),
        QStringLiteral("y"),
        QStringLiteral("width"),
        QStringLiteral("height"),
        QStringLiteral("geometry"),
        QStringLiteral("position")
    };
    for (const QString &key : coordinateKeys) {
        node.remove(key);
    }

    const QJsonValue childrenValue = node.value(QStringLiteral("children"));
    if (childrenValue.isArray()) {
        QJsonArray normalized;
        for (const QJsonValue &child : childrenValue.toArray()) {
            if (child.isObject()) {
                normalized.append(normalizeControlNode(child.toObject()));
            } else {
                normalized.append(child);
            }
        }
        node.insert(QStringLiteral("children"), normalized);
    }
    return node;
}

QJsonObject normalizeSchemaDocument(QJsonObject schema)
{
    if (schema.contains(QStringLiteral("root"))
        && schema.value(QStringLiteral("root")).isObject()) {
        schema.insert(
            QStringLiteral("root"),
            normalizeControlNode(schema.value(QStringLiteral("root")).toObject()));
        return schema;
    }
    return normalizeControlNode(std::move(schema));
}
} // namespace

AppSchemaRenderer::AppSchemaRenderer(QWidget *parent)
    : QWidget(parent)
    , m_scrollArea(new QScrollArea(this))
{
    setObjectName(QStringLiteral("appSchemaSurface"));
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    m_scrollArea->setFrameShape(QFrame::NoFrame);
    m_scrollArea->setWidgetResizable(true);
    m_scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_scrollArea->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    layout->addWidget(m_scrollArea);
}

bool AppSchemaRenderer::setSchema(const QJsonObject &schema, QString *error)
{
    const QJsonObject normalized = normalizeSchemaDocument(schema);
    QJsonObject root;
    if (!validateSchema(normalized, &root, error)) {
        return false;
    }

    m_controls.clear();
    m_focusChain.clear();

    QWidget *surface = buildNode(root, nullptr);
    if (!surface) {
        return failValidation(error, tr("The interface could not be created."));
    }

    QWidget *oldSurface = m_scrollArea->takeWidget();
    m_scrollArea->setWidget(surface);
    delete oldSurface;

    QPointer<QWidget> previous;
    for (const QPointer<QWidget> &current : std::as_const(m_focusChain)) {
        if (!current) {
            continue;
        }
        if (previous) {
            QWidget::setTabOrder(previous.data(), current.data());
        }
        previous = current;
    }

    if (error) {
        error->clear();
    }
    return true;
}

void AppSchemaRenderer::applyPatch(const QJsonObject &patch)
{
    const QJsonValue patchesValue = patch.value(QStringLiteral("patches"));
    if (patchesValue.isArray()) {
        const QJsonArray patches = patchesValue.toArray();
        for (const QJsonValue &value : patches) {
            if (value.isObject()) {
                applySinglePatch(value.toObject());
            }
        }
        return;
    }

    const auto applyObjectMap = [this](const QJsonObject &object) {
        for (auto it = object.constBegin(); it != object.constEnd(); ++it) {
            QJsonObject single;
            if (it.value().isObject()) {
                single = it.value().toObject();
            } else {
                single.insert(QStringLiteral("value"), it.value());
            }
            single.insert(QStringLiteral("id"), it.key());
            applySinglePatch(single);
        }
    };

    const QJsonValue controlsValue = patch.value(QStringLiteral("controls"));
    if (controlsValue.isObject()) {
        applyObjectMap(controlsValue.toObject());
        return;
    }

    const QJsonValue valuesValue = patch.value(QStringLiteral("values"));
    if (valuesValue.isObject()) {
        applyObjectMap(valuesValue.toObject());
        return;
    }

    if (patch.value(QStringLiteral("id")).isString()) {
        applySinglePatch(patch);
        return;
    }

    // Also accept a compact map of control id -> changes.
    for (auto it = patch.constBegin(); it != patch.constEnd(); ++it) {
        if (!m_controls.contains(it.key())) {
            continue;
        }
        QJsonObject single;
        if (it.value().isObject()) {
            single = it.value().toObject();
        } else {
            single.insert(QStringLiteral("value"), it.value());
        }
        single.insert(QStringLiteral("id"), it.key());
        applySinglePatch(single);
    }
}

void AppSchemaRenderer::clear()
{
    m_controls.clear();
    m_focusChain.clear();
    QWidget *surface = m_scrollArea->takeWidget();
    delete surface;
}

bool AppSchemaRenderer::validateSchema(const QJsonObject &schema,
                                       QJsonObject *root,
                                       QString *error) const
{
    const QJsonValue version = schema.value(QStringLiteral("version"));
    if (!version.isUndefined()) {
        if (!version.isDouble() || version.toDouble() != 1.0) {
            return failValidation(error, tr("UI schema version 1 is required."));
        }
    }

    QJsonObject rootNode;
    if (schema.contains(QStringLiteral("root"))) {
        const QJsonValue rootValue = schema.value(QStringLiteral("root"));
        if (!rootValue.isObject()) {
            return failValidation(error, tr("The schema root must be an object."));
        }
        rootNode = rootValue.toObject();
    } else {
        rootNode = schema;
    }

    if (rootNode.value(QStringLiteral("type")).toString()
        != QStringLiteral("screen")) {
        const QString found = rootNode.value(QStringLiteral("type")).toString();
        return failValidation(
            error,
            found.isEmpty()
                ? tr("根控件必须是 screen（当前缺少 type；不要用 kind）。")
                : tr("根控件必须是 screen（当前 type=%1）。").arg(found));
    }

    int nodeCount = 0;
    QSet<QString> ids;
    if (!validateNode(rootNode, 1, nodeCount, ids, error)) {
        return false;
    }

    if (root) {
        *root = rootNode;
    }
    return true;
}

bool AppSchemaRenderer::validateNode(const QJsonObject &node,
                                     int depth,
                                     int &nodeCount,
                                     QSet<QString> &ids,
                                     QString *error) const
{
    if (depth > MaxDepth) {
        return failValidation(
            error, tr("The interface exceeds the maximum depth of %1.")
                       .arg(MaxDepth));
    }
    ++nodeCount;
    if (nodeCount > MaxNodes) {
        return failValidation(
            error, tr("The interface exceeds the maximum of %1 controls.")
                       .arg(MaxNodes));
    }

    const QJsonValue typeValue = node.value(QStringLiteral("type"));
    if (!typeValue.isString() || typeValue.toString().trimmed().isEmpty()) {
        return failValidation(error, tr("Every control needs a type."));
    }
    const QString type = typeValue.toString();
    if (type.size() > 64) {
        return failValidation(error, tr("A control type name is too long."));
    }

    const QJsonValue idValue = node.value(QStringLiteral("id"));
    QString id;
    if (!idValue.isUndefined()) {
        if (!idValue.isString()) {
            return failValidation(error, tr("Control ids must be strings."));
        }
        id = idValue.toString();
        if (id.trimmed().isEmpty() || id.size() > 128) {
            return failValidation(
                error, tr("Control ids must contain 1 to 128 characters."));
        }
        if (ids.contains(id)) {
            return failValidation(
                error, tr("Control id \"%1\" is used more than once.").arg(id));
        }
        ids.insert(id);
    }

    const bool interactive = type == QStringLiteral("button")
        || type == QStringLiteral("toggle")
        || type == QStringLiteral("slider");
    if (interactive && id.isEmpty()) {
        return failValidation(
            error, tr("Interactive control \"%1\" needs an id.").arg(type));
    }

    const QJsonValue sizeValue = node.value(QStringLiteral("size"));
    if (!sizeValue.isUndefined()) {
        if (!sizeValue.isString()) {
            return failValidation(error, tr("Control size must be sm, md, or lg."));
        }
        const QString size = sizeValue.toString();
        if (size != QStringLiteral("sm")
            && size != QStringLiteral("md")
            && size != QStringLiteral("lg")) {
            return failValidation(error, tr("Control size must be sm, md, or lg."));
        }
    }

    static const QStringList coordinateKeys = {
        QStringLiteral("x"),
        QStringLiteral("y"),
        QStringLiteral("width"),
        QStringLiteral("height"),
        QStringLiteral("geometry"),
        QStringLiteral("position")
    };
    for (const QString &key : coordinateKeys) {
        if (node.contains(key)) {
            // Should already be stripped in normalizeControlNode; keep as safety net
            // with a precise message for diagnostics.
            const QString where = id.isEmpty() ? type : id;
            return failValidation(
                error,
                tr("Arbitrary control coordinates are not supported (field \"%1\" on \"%2\").")
                    .arg(key, where));
        }
    }

    static const QStringList textKeys = {
        QStringLiteral("text"),
        QStringLiteral("label"),
        QStringLiteral("title"),
        QStringLiteral("subtitle")
    };
    for (const QString &key : textKeys) {
        const QJsonValue value = node.value(key);
        if (value.isUndefined()) {
            continue;
        }
        if (!value.isString()) {
            return failValidation(
                error, tr("The \"%1\" field must be text.").arg(key));
        }
        if (value.toString().size() > MaxTextLength) {
            return failValidation(
                error, tr("Text in the interface is too long."));
        }
    }

    const QJsonValue enabled = node.value(QStringLiteral("enabled"));
    if (!enabled.isUndefined() && !enabled.isBool()) {
        return failValidation(error, tr("The enabled field must be true or false."));
    }
    const QJsonValue visible = node.value(QStringLiteral("visible"));
    if (!visible.isUndefined() && !visible.isBool()) {
        return failValidation(error, tr("The visible field must be true or false."));
    }
    const QJsonValue wrap = node.value(QStringLiteral("wrap"));
    if (!wrap.isUndefined() && !wrap.isBool()) {
        return failValidation(error, tr("The wrap field must be true or false."));
    }

    if (type == QStringLiteral("toggle")) {
        const QJsonValue checked = node.contains(QStringLiteral("value"))
            ? node.value(QStringLiteral("value"))
            : node.value(QStringLiteral("checked"));
        if (!checked.isUndefined() && !checked.isBool()) {
            return failValidation(
                error, tr("A toggle value must be true or false."));
        }
    }

    if (type == QStringLiteral("slider")
        || type == QStringLiteral("progress")) {
        int minimum = 0;
        int maximum = 100;
        int value = minimum;

        if (node.contains(QStringLiteral("min"))
            && !jsonInteger(node.value(QStringLiteral("min")), &minimum)) {
            return failValidation(error, tr("The minimum must be a whole number."));
        }
        if (node.contains(QStringLiteral("max"))
            && !jsonInteger(node.value(QStringLiteral("max")), &maximum)) {
            return failValidation(error, tr("The maximum must be a whole number."));
        }
        if (minimum >= maximum) {
            return failValidation(
                error, tr("The maximum must be greater than the minimum."));
        }
        if (node.contains(QStringLiteral("value"))) {
            if (!jsonInteger(node.value(QStringLiteral("value")), &value)) {
                return failValidation(error, tr("The value must be a whole number."));
            }
            if (value < minimum || value > maximum) {
                return failValidation(
                    error, tr("A control value is outside its allowed range."));
            }
        }
    }

    if ((type == QStringLiteral("button")
         || type == QStringLiteral("value"))
        && node.contains(QStringLiteral("value"))) {
        const QJsonValue value = node.value(QStringLiteral("value"));
        if (!isScalar(value)) {
            return failValidation(
                error, tr("Control values must be text, numbers, booleans, or null."));
        }
        if (value.isString() && value.toString().size() > MaxTextLength) {
            return failValidation(error, tr("Text in the interface is too long."));
        }
    }

    if (type == QStringLiteral("grid")
        && node.contains(QStringLiteral("columns"))) {
        int columns = 0;
        if (!jsonInteger(node.value(QStringLiteral("columns")), &columns)
            || columns < 1 || columns > 4) {
            return failValidation(
                error, tr("A grid must have between 1 and 4 columns."));
        }
    }

    const QJsonValue childrenValue = node.value(QStringLiteral("children"));
    if (childrenValue.isUndefined()) {
        return true;
    }
    if (!childrenValue.isArray()) {
        return failValidation(error, tr("Control children must be an array."));
    }

    const QJsonArray children = childrenValue.toArray();
    for (const QJsonValue &childValue : children) {
        if (!childValue.isObject()) {
            return failValidation(error, tr("Every child control must be an object."));
        }
        if (!validateNode(childValue.toObject(), depth + 1,
                          nodeCount, ids, error)) {
            return false;
        }
    }
    return true;
}

QWidget *AppSchemaRenderer::buildNode(const QJsonObject &node, QWidget *parent)
{
    const QString type = node.value(QStringLiteral("type")).toString();
    const QString size = node.value(QStringLiteral("size"))
                             .toString(QStringLiteral("md"));

    if (type == QStringLiteral("screen")
        || type == QStringLiteral("column")
        || type == QStringLiteral("row")
        || type == QStringLiteral("grid")) {
        return buildContainer(node, parent, type);
    }

    if (type == QStringLiteral("label")) {
        auto *label = new QLabel(node.value(QStringLiteral("text")).toString(),
                                 parent);
        label->setTextFormat(Qt::PlainText);
        label->setWordWrap(node.value(QStringLiteral("wrap")).toBool(true));
        label->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Minimum);
        applySemanticSize(label, size, false);
        registerControl(node, label);
        return label;
    }

    if (type == QStringLiteral("button")) {
        QString text = node.value(QStringLiteral("text")).toString();
        if (text.isEmpty()) {
            text = node.value(QStringLiteral("label")).toString();
        }
        if (text.isEmpty()) {
            text = tr("Action");
        }
        auto *button = new QPushButton(text, parent);
        button->setAutoDefault(false);
        button->setFocusPolicy(Qt::StrongFocus);
        applySemanticSize(button, size, true);
        registerControl(node, button);
        addToFocusChain(button);

        const QString id = node.value(QStringLiteral("id")).toString();
        const QJsonValue eventValue = node.contains(QStringLiteral("value"))
            ? node.value(QStringLiteral("value"))
            : QJsonValue(true);
        button->setProperty(EventValueProperty, eventValue.toVariant());
        const QPointer<QPushButton> guardedButton(button);
        connect(button, &QPushButton::clicked, this,
                [this, id, guardedButton] {
                    if (!guardedButton) {
                        return;
                    }
                    emitEvent(
                        id,
                        QStringLiteral("click"),
                        QJsonValue::fromVariant(
                            guardedButton->property(EventValueProperty)));
                });
        return button;
    }

    if (type == QStringLiteral("toggle")) {
        QString text = node.value(QStringLiteral("text")).toString();
        if (text.isEmpty()) {
            text = tr("Toggle");
        }
        auto *toggle = new QCheckBox(text, parent);
        const bool checked = node.contains(QStringLiteral("value"))
            ? node.value(QStringLiteral("value")).toBool()
            : node.value(QStringLiteral("checked")).toBool(false);
        toggle->setChecked(checked);
        toggle->setFocusPolicy(Qt::StrongFocus);
        applySemanticSize(toggle, size, true);
        registerControl(node, toggle);
        addToFocusChain(toggle);

        const QString id = node.value(QStringLiteral("id")).toString();
        connect(toggle, &QCheckBox::clicked, this,
                [this, id](bool value) {
                    emitEvent(id, QStringLiteral("change"), QJsonValue(value));
                });
        return toggle;
    }

    if (type == QStringLiteral("slider")) {
        auto *slider = new QSlider(Qt::Horizontal, parent);
        slider->setRange(node.value(QStringLiteral("min")).toInt(0),
                         node.value(QStringLiteral("max")).toInt(100));
        slider->setValue(node.value(QStringLiteral("value"))
                             .toInt(slider->minimum()));
        slider->setTracking(true);
        slider->setFocusPolicy(Qt::StrongFocus);
        const QString accessibleName =
            node.value(QStringLiteral("label")).toString();
        slider->setAccessibleName(accessibleName.isEmpty()
                                      ? tr("Slider")
                                      : accessibleName);
        applySemanticSize(slider, size, true);
        registerControl(node, slider);
        addToFocusChain(slider);

        const QString id = node.value(QStringLiteral("id")).toString();
        connect(slider, &QSlider::valueChanged, this,
                [this, id](int value) {
                    emitEvent(id, QStringLiteral("change"), QJsonValue(value));
                });
        return slider;
    }

    if (type == QStringLiteral("progress")) {
        auto *progress = new QProgressBar(parent);
        progress->setRange(node.value(QStringLiteral("min")).toInt(0),
                           node.value(QStringLiteral("max")).toInt(100));
        progress->setValue(node.value(QStringLiteral("value"))
                               .toInt(progress->minimum()));
        if (node.contains(QStringLiteral("text"))) {
            progress->setFormat(node.value(QStringLiteral("text")).toString());
        }
        const QString accessibleName =
            node.value(QStringLiteral("label")).toString();
        progress->setAccessibleName(accessibleName.isEmpty()
                                        ? tr("Progress")
                                        : accessibleName);
        applySemanticSize(progress, size, false);
        registerControl(node, progress);
        return progress;
    }

    if (type == QStringLiteral("value")) {
        auto *card = new QFrame(parent);
        card->setFrameShape(QFrame::StyledPanel);
        card->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Minimum);
        auto *layout = new QHBoxLayout(card);
        layout->setContentsMargins(12, 8, 12, 8);
        layout->setSpacing(12);

        const QString captionText =
            node.value(QStringLiteral("label")).toString();
        if (!captionText.isEmpty()) {
            auto *caption = new QLabel(captionText, card);
            caption->setTextFormat(Qt::PlainText);
            caption->setWordWrap(true);
            layout->addWidget(caption);
        }

        auto *valueLabel =
            new QLabel(displayValue(
                           node.contains(QStringLiteral("value"))
                               ? node.value(QStringLiteral("value"))
                               : node.value(QStringLiteral("text"))),
                       card);
        valueLabel->setObjectName(QStringLiteral("appSchemaValueText"));
        valueLabel->setTextFormat(Qt::PlainText);
        valueLabel->setWordWrap(true);
        valueLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
        layout->addWidget(valueLabel, 1);

        applySemanticSize(valueLabel, size, false);
        card->setMinimumHeight(44);
        registerControl(node, card);
        return card;
    }

    return buildUnsupported(node, parent, type);
}

QWidget *AppSchemaRenderer::buildContainer(const QJsonObject &node,
                                            QWidget *parent,
                                            const QString &type)
{
    auto *container = new QWidget(parent);
    container->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Minimum);

    const QString size = node.value(QStringLiteral("size"))
                             .toString(QStringLiteral("md"));
    const int spacing = size == QStringLiteral("sm")
        ? 8
        : (size == QStringLiteral("lg") ? 16 : 12);
    const int margin = type == QStringLiteral("screen") ? spacing : 0;

    QBoxLayout *boxLayout = nullptr;
    QGridLayout *gridLayout = nullptr;
    if (type == QStringLiteral("row")) {
        boxLayout = new QHBoxLayout(container);
    } else if (type == QStringLiteral("grid")) {
        gridLayout = new QGridLayout(container);
    } else {
        boxLayout = new QVBoxLayout(container);
    }

    if (boxLayout) {
        boxLayout->setContentsMargins(margin, margin, margin, margin);
        boxLayout->setSpacing(spacing);
    } else {
        gridLayout->setContentsMargins(margin, margin, margin, margin);
        gridLayout->setHorizontalSpacing(spacing);
        gridLayout->setVerticalSpacing(spacing);
    }

    if (type == QStringLiteral("screen") && boxLayout) {
        const QString titleText =
            node.value(QStringLiteral("title")).toString();
        if (!titleText.isEmpty()) {
            auto *title = new QLabel(titleText, container);
            title->setTextFormat(Qt::PlainText);
            title->setWordWrap(true);
            QFont titleFont = title->font();
            if (titleFont.pointSizeF() > 0.0) {
                titleFont.setPointSizeF(titleFont.pointSizeF() + 4.0);
            }
            titleFont.setBold(true);
            title->setFont(titleFont);
            boxLayout->addWidget(title);
        }

        const QString subtitleText =
            node.value(QStringLiteral("subtitle")).toString();
        if (!subtitleText.isEmpty()) {
            auto *subtitle = new QLabel(subtitleText, container);
            subtitle->setTextFormat(Qt::PlainText);
            subtitle->setWordWrap(true);
            boxLayout->addWidget(subtitle);
        }
    }

    const QJsonArray children =
        node.value(QStringLiteral("children")).toArray();
    const int columns = node.value(QStringLiteral("columns")).toInt(2);
    int childIndex = 0;
    for (const QJsonValue &childValue : children) {
        QWidget *child = buildNode(childValue.toObject(), container);
        if (!child) {
            continue;
        }
        if (gridLayout) {
            gridLayout->addWidget(child, childIndex / columns,
                                  childIndex % columns);
        } else {
            boxLayout->addWidget(child);
        }
        ++childIndex;
    }

    if (gridLayout) {
        for (int column = 0; column < columns; ++column) {
            gridLayout->setColumnStretch(column, 1);
        }
    } else {
        boxLayout->addStretch(1);
    }

    container->setProperty("semanticSize", size);
    registerControl(node, container);
    return container;
}

QWidget *AppSchemaRenderer::buildUnsupported(const QJsonObject &node,
                                              QWidget *parent,
                                              const QString &type)
{
    auto *card = new QFrame(parent);
    card->setObjectName(QStringLiteral("appSchemaUnsupported"));
    card->setFrameShape(QFrame::StyledPanel);

    auto *layout = new QVBoxLayout(card);
    layout->setContentsMargins(12, 10, 12, 10);
    auto *label = new QLabel(
        tr("组件不受支持：%1").arg(type), card);
    label->setTextFormat(Qt::PlainText);
    label->setWordWrap(true);
    layout->addWidget(label);

    registerControl(node, card);
    card->setVisible(true);
    return card;
}

void AppSchemaRenderer::registerControl(const QJsonObject &node,
                                        QWidget *widget)
{
    if (!widget) {
        return;
    }

    widget->setProperty(
        ControlTypeProperty,
        node.value(QStringLiteral("type")).toString());
    widget->setEnabled(node.value(QStringLiteral("enabled")).toBool(true));
    widget->setVisible(node.value(QStringLiteral("visible")).toBool(true));

    const QString accessibleName =
        node.value(QStringLiteral("label")).toString(
            node.value(QStringLiteral("text")).toString());
    if (!accessibleName.isEmpty() && widget->accessibleName().isEmpty()) {
        widget->setAccessibleName(accessibleName);
    }

    const QString id = node.value(QStringLiteral("id")).toString();
    if (!id.isEmpty()) {
        m_controls.insert(id, widget);
    }
}

void AppSchemaRenderer::addToFocusChain(QWidget *widget)
{
    if (widget) {
        m_focusChain.append(widget);
    }
}

void AppSchemaRenderer::applySemanticSize(QWidget *widget,
                                          const QString &size,
                                          bool interactive) const
{
    if (!widget) {
        return;
    }

    widget->setProperty("semanticSize", size);

    QFont font = widget->font();
    if (font.pointSizeF() > 0.0) {
        const qreal delta = size == QStringLiteral("sm")
            ? 0.0
            : (size == QStringLiteral("lg") ? 3.0 : 1.0);
        font.setPointSizeF(font.pointSizeF() + delta);
        widget->setFont(font);
    }

    if (interactive) {
        const int extent = size == QStringLiteral("lg") ? 56 : 44;
        widget->setMinimumSize(44, extent);
    } else if (qobject_cast<QProgressBar *>(widget)) {
        widget->setMinimumHeight(size == QStringLiteral("lg") ? 32 : 24);
    }
}

void AppSchemaRenderer::applySinglePatch(const QJsonObject &patch)
{
    const QString id = patch.value(QStringLiteral("id")).toString();
    if (id.isEmpty()) {
        return;
    }

    const QPointer<QWidget> widget = m_controls.value(id);
    if (!widget) {
        return;
    }

    const QJsonValue changesValue = patch.value(QStringLiteral("changes"));
    patchWidget(widget, changesValue.isObject()
                            ? changesValue.toObject()
                            : patch);
}

void AppSchemaRenderer::patchWidget(QWidget *widget,
                                    const QJsonObject &changes)
{
    if (!widget) {
        return;
    }

    const QSignalBlocker blocker(widget);
    if (changes.value(QStringLiteral("enabled")).isBool()) {
        widget->setEnabled(
            changes.value(QStringLiteral("enabled")).toBool());
    }
    if (changes.value(QStringLiteral("visible")).isBool()) {
        widget->setVisible(
            changes.value(QStringLiteral("visible")).toBool());
    }

    const QString type = widget->property(ControlTypeProperty).toString();
    const QJsonValue text = changes.value(QStringLiteral("text"));
    const QJsonValue value = changes.value(QStringLiteral("value"));

    if (type == QStringLiteral("label")) {
        if (auto *label = qobject_cast<QLabel *>(widget)) {
            if (text.isString()) {
                label->setText(text.toString().left(MaxTextLength));
            } else if (isScalar(value) && !value.isUndefined()) {
                label->setText(displayValue(value));
            }
        }
        return;
    }

    if (type == QStringLiteral("button")) {
        if (auto *button = qobject_cast<QPushButton *>(widget)) {
            if (text.isString()) {
                button->setText(text.toString().left(MaxTextLength));
            }
            if (!value.isUndefined() && isScalar(value)) {
                button->setProperty(EventValueProperty, value.toVariant());
            }
        }
        return;
    }

    if (type == QStringLiteral("toggle")) {
        if (auto *toggle = qobject_cast<QCheckBox *>(widget)) {
            if (text.isString()) {
                toggle->setText(text.toString().left(MaxTextLength));
            }
            const QJsonValue checked =
                changes.value(QStringLiteral("checked")).isBool()
                ? changes.value(QStringLiteral("checked"))
                : value;
            if (checked.isBool()) {
                toggle->setChecked(checked.toBool());
            }
        }
        return;
    }

    if (type == QStringLiteral("slider")) {
        if (auto *slider = qobject_cast<QSlider *>(widget)) {
            const int minimum = patchedInteger(
                changes.value(QStringLiteral("min")), slider->minimum());
            const int maximum = patchedInteger(
                changes.value(QStringLiteral("max")), slider->maximum());
            if (minimum < maximum) {
                slider->setRange(minimum, maximum);
            }
            if (value.isDouble()) {
                slider->setValue(
                    patchedInteger(value, slider->value()));
            }
        }
        return;
    }

    if (type == QStringLiteral("progress")) {
        if (auto *progress = qobject_cast<QProgressBar *>(widget)) {
            const int minimum = patchedInteger(
                changes.value(QStringLiteral("min")), progress->minimum());
            const int maximum = patchedInteger(
                changes.value(QStringLiteral("max")), progress->maximum());
            if (minimum < maximum) {
                progress->setRange(minimum, maximum);
            }
            if (value.isDouble()) {
                progress->setValue(
                    patchedInteger(value, progress->value()));
            }
            if (text.isString()) {
                progress->setFormat(text.toString().left(MaxTextLength));
            }
        }
        return;
    }

    if (type == QStringLiteral("value")) {
        auto *valueLabel = widget->findChild<QLabel *>(
            QStringLiteral("appSchemaValueText"), Qt::FindDirectChildrenOnly);
        if (valueLabel) {
            if (!value.isUndefined() && isScalar(value)) {
                valueLabel->setText(displayValue(value));
            } else if (text.isString()) {
                valueLabel->setText(text.toString().left(MaxTextLength));
            }
        }
    }
}

void AppSchemaRenderer::emitEvent(const QString &id,
                                  const QString &eventName,
                                  const QJsonValue &value)
{
    QJsonObject event;
    event.insert(QStringLiteral("type"), QStringLiteral("ui.event"));
    event.insert(QStringLiteral("id"), id);
    event.insert(QStringLiteral("event"), eventName);
    event.insert(QStringLiteral("value"),
                 value.isUndefined() ? QJsonValue(QJsonValue::Null) : value);
    emit uiEvent(event);
}

QString AppSchemaRenderer::displayValue(const QJsonValue &value)
{
    if (value.isString()) {
        return value.toString().left(MaxTextLength);
    }
    if (value.isBool()) {
        return value.toBool()
            ? QCoreApplication::translate("AppSchemaRenderer", "On")
            : QCoreApplication::translate("AppSchemaRenderer", "Off");
    }
    if (value.isDouble()) {
        return QLocale().toString(value.toDouble(), 'g', 12);
    }
    if (value.isNull()) {
        return QStringLiteral("—");
    }
    return QString();
}
