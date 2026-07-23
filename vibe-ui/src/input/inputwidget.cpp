#include "inputwidget.h"



#include <QEvent>

#include <QFileDialog>

#include <QFontMetrics>

#include <QHBoxLayout>

#include <QKeyEvent>

#include <QLabel>

#include <QPushButton>

#include <QSizePolicy>

#include <QStyle>

#include <QTextEdit>

#include <QToolButton>

#include <QVBoxLayout>



namespace {

constexpr int kMinEditorLines = 1;

constexpr int kMaxEditorLines = 5;

}



InputWidget::InputWidget(QWidget *parent)

    : QWidget(parent)

    , m_editor(new QTextEdit(this))

    , m_attachButton(new QToolButton(this))

    , m_keyboardButton(new QToolButton(this))

    , m_sendButton(new QPushButton(tr("发送"), this))

    , m_footer(new QLabel(this))

    , m_contextBar(new QWidget(this))

    , m_contextModelBtn(new QPushButton(this))

    , m_contextInfo(new QLabel(this))

{

    setObjectName(QStringLiteral("composer"));

    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Maximum);



    m_contextBar->setObjectName(QStringLiteral("contextBar"));

    m_contextBar->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);

    auto *contextLayout = new QHBoxLayout(m_contextBar);

    contextLayout->setContentsMargins(0, 0, 0, 0);

    contextLayout->setSpacing(8);

    m_contextModelBtn->setObjectName(QStringLiteral("contextModel"));

    m_contextModelBtn->setFlat(true);

    m_contextModelBtn->setCursor(Qt::PointingHandCursor);

    m_contextModelBtn->setToolTip(tr("打开设置"));

    m_contextInfo->setObjectName(QStringLiteral("contextInfo"));

    contextLayout->addWidget(m_contextModelBtn);

    contextLayout->addStretch(1);

    contextLayout->addWidget(m_contextInfo);

    connect(m_contextModelBtn, &QPushButton::clicked, this, &InputWidget::settingsRequested);



    m_editor->setObjectName(QStringLiteral("promptEditor"));

    m_editor->setPlaceholderText(

        tr("描述下一步改动、修正某个树节点，或发起一次严格审视…"));

    m_editor->setAcceptRichText(false);

    m_editor->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);

    m_editor->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);

    m_editor->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

    m_editor->setTabChangesFocus(false);

    m_editor->installEventFilter(this);



    m_attachButton->setObjectName(QStringLiteral("attachButton"));

    m_attachButton->setText(QStringLiteral("+"));

    m_attachButton->setToolTip(tr("附加上下文文件"));

    m_attachButton->setFixedSize(40, 40);



    m_keyboardButton->setObjectName(QStringLiteral("attachButton"));

    m_keyboardButton->setText(tr("键"));

    m_keyboardButton->setToolTip(tr("打开/关闭软键盘（点输入框也会自动弹出）"));

    m_keyboardButton->setCheckable(true);

    m_keyboardButton->setFixedSize(40, 40);



    m_sendButton->setObjectName(QStringLiteral("sendButton"));

    m_sendButton->setDefault(true);

    m_sendButton->setFixedWidth(72);

    m_sendButton->setMinimumHeight(40);

    m_footer->setObjectName(QStringLiteral("composerFooter"));



    auto *editorRow = new QHBoxLayout;

    editorRow->setContentsMargins(0, 0, 0, 0);

    editorRow->setSpacing(6);

    editorRow->addWidget(m_attachButton, 0, Qt::AlignBottom);

    editorRow->addWidget(m_keyboardButton, 0, Qt::AlignBottom);

    editorRow->addWidget(m_editor, 1);

    editorRow->addWidget(m_sendButton, 0, Qt::AlignBottom);



    auto *layout = new QVBoxLayout(this);

    layout->setContentsMargins(10, 6, 10, 6);

    layout->setSpacing(4);

    layout->addWidget(m_contextBar);

    layout->addLayout(editorRow);

    layout->addWidget(m_footer);



    connect(m_sendButton, &QPushButton::clicked, this, [this] {

        if (m_busy) {

            emit interruptRequested();

        } else {

            submit();

        }

    });

    connect(m_attachButton, &QToolButton::clicked, this, &InputWidget::attachFiles);

    connect(m_keyboardButton, &QToolButton::clicked, this, [this] {

        m_editor->setFocus();

        emit softKeyboardToggleRequested();

    });

    connect(m_editor, &QTextEdit::textChanged, this, &InputWidget::updateFooter);

    connect(m_editor, &QTextEdit::textChanged, this, &InputWidget::adjustEditorHeight);



    adjustEditorHeight();

    setBackendReady(false);

}



bool InputWidget::eventFilter(QObject *watched, QEvent *event)

{

    if (watched == m_editor && event->type() == QEvent::KeyPress) {

        auto *keyEvent = static_cast<QKeyEvent *>(event);

        if ((keyEvent->key() == Qt::Key_Return || keyEvent->key() == Qt::Key_Enter)

            && keyEvent->modifiers().testFlag(Qt::ControlModifier)) {

            if (!m_busy) {

                submit();

            }

            return true;

        }

    }

    return QWidget::eventFilter(watched, event);

}



void InputWidget::adjustEditorHeight()

{

    const QFontMetrics fm(m_editor->font());

    const int lineH = qMax(18, fm.lineSpacing());

    const int pad = 16;

    const int minH = lineH * kMinEditorLines + pad;

    const int maxH = lineH * kMaxEditorLines + pad;

    m_editor->document()->setTextWidth(m_editor->viewport()->width());

    const int docH = int(m_editor->document()->size().height()) + pad;

    const int h = qBound(minH, docH, maxH);

    if (m_editor->height() != h) {

        m_editor->setFixedHeight(h);

    }

    m_sendButton->setFixedHeight(qMax(40, qMin(h, 52)));

}



void InputWidget::setKeyboardButtonChecked(bool checked)

{

    m_keyboardButton->setChecked(checked);

}



void InputWidget::setAgentBusy(bool busy)

{

    m_busy = busy;

    m_editor->setEnabled(m_ready && !busy);

    m_attachButton->setEnabled(m_ready && !busy);

    m_keyboardButton->setEnabled(true);

    m_sendButton->setEnabled(m_ready);

    m_sendButton->setText(busy ? tr("停止") : tr("发送"));

    m_sendButton->setProperty("stop", busy);

    m_sendButton->style()->unpolish(m_sendButton);

    m_sendButton->style()->polish(m_sendButton);

    updateFooter();

}



void InputWidget::setBackendReady(bool ready)

{

    m_ready = ready;

    setAgentBusy(m_busy);

}



void InputWidget::clearDraft()

{

    m_editor->clear();

    m_attachedFiles.clear();

    updateFooter();

    adjustEditorHeight();

}



void InputWidget::focusEditor()

{

    m_editor->setFocus();

}



void InputWidget::submit()

{

    if (!m_ready || m_busy) {

        return;

    }

    QString message = m_editor->toPlainText().trimmed();

    if (message.isEmpty()) {

        return;

    }

    if (!m_attachedFiles.isEmpty()) {

        message += tr("\n\n显式上下文文件：\n- %1")

                       .arg(m_attachedFiles.join(QStringLiteral("\n- ")));

    }

    emit messageSubmitted(message);

}



void InputWidget::attachFiles()

{

    const QStringList files = QFileDialog::getOpenFileNames(

        this,

        tr("附加上下文文件"));

    for (const QString &file : files) {

        if (!m_attachedFiles.contains(file)) {

            m_attachedFiles.append(file);

        }

    }

    updateFooter();

}



void InputWidget::setContextModel(const QString &modelName)

{

    m_contextModelBtn->setText(modelName.isEmpty() ? tr("模型") : modelName);

}



void InputWidget::setContextTokens(quint64 tokensIn, quint64 tokensOut)

{

    m_contextTokensIn = tokensIn;

    m_contextTokensOut = tokensOut;

    refreshContextInfo();

}



void InputWidget::setContextSteps(quint64 steps)

{

    m_contextSteps = steps;

    refreshContextInfo();

}



void InputWidget::refreshContextInfo()

{

    QStringList parts;

    if (m_contextSteps > 0)

        parts.append(tr("%1 步").arg(m_contextSteps));

    if (m_contextTokensIn + m_contextTokensOut > 0) {

        parts.append(tr("入 %1 · 出 %2").arg(m_contextTokensIn).arg(m_contextTokensOut));

    }

    m_contextInfo->setText(parts.join(QStringLiteral(" · ")));

}



void InputWidget::updateFooter()

{

    if (m_busy) {

        m_footer->setText(tr("Agent 工作中 · 按停止可中断"));

        return;

    }

    if (!m_ready) {

        m_footer->setText(tr("后端不可用 · 草稿已保留"));

        return;

    }

    const int characters = m_editor->toPlainText().size();

    const QString files = m_attachedFiles.isEmpty()

        ? QString()

        : tr(" · %1 个文件").arg(m_attachedFiles.size());

    m_footer->setText(tr("Ctrl+Enter 发送 · %1 字%2").arg(characters).arg(files));

}


