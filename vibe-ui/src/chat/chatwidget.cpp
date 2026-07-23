#include "chatwidget.h"
#include "opencode_antialias.h"

#include <QApplication>
#include <QEvent>
#include <QFrame>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QLabel>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QRegularExpression>
#include <QResizeEvent>
#include <QScrollArea>
#include <QScrollBar>
#include <QSizePolicy>
#include <QStyle>
#include <QTextBrowser>
#include <QTextOption>
#include <QTimer>
#include <QVBoxLayout>

namespace {

QString cssFontStack(const QStringList &families)
{
    QStringList quoted;
    quoted.reserve(families.size());
    for (const QString &family : families) {
        quoted.append(QStringLiteral("'%1'").arg(family));
    }
    return quoted.join(QStringLiteral(", "));
}

// Long unbroken tokens must never force the chat column wider than the viewport.
class WrappingPlainTextEdit final : public QPlainTextEdit
{
public:
    explicit WrappingPlainTextEdit(QWidget *parent = nullptr)
        : QPlainTextEdit(parent)
    {
        setMinimumWidth(0);
        setLineWrapMode(QPlainTextEdit::WidgetWidth);
        setWordWrapMode(QTextOption::WrapAtWordBoundaryOrAnywhere);
        setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    }

    QSize minimumSizeHint() const override { return QSize(0, QPlainTextEdit::minimumSizeHint().height()); }
    QSize sizeHint() const override
    {
        return QSize(0, QPlainTextEdit::sizeHint().height());
    }
};

class FullHeightTextBrowser final : public QTextBrowser
{
public:
    explicit FullHeightTextBrowser(QWidget *parent = nullptr)
        : QTextBrowser(parent)
    {
        setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setFrameShape(QFrame::NoFrame);
        setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Fixed);
        setMinimumWidth(0);
        QTextOption opt = document()->defaultTextOption();
        opt.setWrapMode(QTextOption::WrapAtWordBoundaryOrAnywhere);
        document()->setDefaultTextOption(opt);
    }

    QSize minimumSizeHint() const override { return QSize(0, 20); }
    QSize sizeHint() const override { return QSize(0, height() > 0 ? height() : 20); }

    void refreshLayout() { recalcHeight(); }

protected:
    void resizeEvent(QResizeEvent *event) override
    {
        QTextBrowser::resizeEvent(event);
        recalcHeight();
    }

private:
    void recalcHeight()
    {
        if (!document() || !viewport())
            return;
        // Prefer viewport width; fall back so first paint (width 0) still wraps to parent.
        int w = viewport()->width();
        if (w < 8)
            w = width() - contentsMargins().left() - contentsMargins().right();
        if (w < 8 && parentWidget())
            w = parentWidget()->width() - 16;
        if (w < 8)
            return;
        document()->setTextWidth(w);
        const int h = qMax(20, static_cast<int>(document()->size().height()) + 8);
        if (height() != h)
            setFixedHeight(h);
    }
};

void configureWrappingPlainText(QPlainTextEdit *edit)
{
    if (!edit)
        return;
    edit->setMinimumWidth(0);
    edit->setLineWrapMode(QPlainTextEdit::WidgetWidth);
    edit->setWordWrapMode(QTextOption::WrapAtWordBoundaryOrAnywhere);
    edit->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    // Ignored: longest-line sizeHint must not widen the chat past the viewport.
    edit->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
}

void configureWrappingLabel(QLabel *label)
{
    if (!label)
        return;
    label->setWordWrap(true);
    label->setMinimumWidth(0);
    // Ignored horizontal sizeHint so long paths cannot force the chat wider than the viewport.
    label->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
}

void applyMessageBodyStyle(QTextBrowser *body)
{
    if (!body)
        return;
    QFont f = QApplication::font();
    f.setStyleStrategy(QFont::PreferAntialias);
    f.setFamilies(opencode::uiFontFamilies());
    body->setFont(f);
    const int sz = qBound(9, f.pointSize() > 0 ? f.pointSize() : 13, 28);
    const QString families = cssFontStack(opencode::uiFontFamilies());
    const QString mono = cssFontStack(opencode::monoFontFamilies());
    // pre-wrap + break-word: long paths/tokens stay inside the chat column.
    body->document()->setDefaultStyleSheet(QStringLiteral(
        "body { font-family: %1; font-size: %3px; }"
        "p, li, td, th, span, div { }"
        "pre, code { font-family: %2; font-size: %4px; white-space: pre-wrap; }")
                                               .arg(families, mono)
                                               .arg(sz)
                                               .arg(qMax(9, sz - 1)));
    QTextOption opt = body->document()->defaultTextOption();
    opt.setWrapMode(QTextOption::WrapAtWordBoundaryOrAnywhere);
    body->document()->setDefaultTextOption(opt);
}

QString describeToolCall(const QString &name, const QString &inputJson)
{
    if (inputJson.isEmpty())
        return name;

    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(inputJson.toUtf8(), &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject())
        return name;

    const QJsonObject args = doc.object();

    if (name == QStringLiteral("read")) {
        const QString path = args.value(QStringLiteral("filePath")).toString(
            args.value(QStringLiteral("path")).toString());
        const int offset = args.value(QStringLiteral("offset")).toInt(1);
        const int limit = args.value(QStringLiteral("limit")).toInt(2000);
        if (path.isEmpty())
            return QStringLiteral("read");
        if (args.contains(QStringLiteral("offset")) || args.contains(QStringLiteral("limit"))) {
            const int end = offset + qMax(0, limit - 1);
            return QStringLiteral("read %1 · L%2-%3").arg(path).arg(offset).arg(end);
        }
        return QStringLiteral("read %1").arg(path);
    }
    if (name == QStringLiteral("bash") || name == QStringLiteral("verify_command")) {
        const QString cmd = args.value(QStringLiteral("command")).toString();
        const QJsonArray argv = args.value(QStringLiteral("args")).toArray();
        QStringList parts;
        parts.append(cmd);
        for (const QJsonValue &v : argv)
            parts.append(v.toString());
        const QString joined = parts.join(QLatin1Char(' '));
        return joined.size() > 80 ? joined.left(80) + QStringLiteral("...") : joined;
    }
    if (name == QStringLiteral("grep")) {
        return QStringLiteral("grep \"%1\" in %2")
            .arg(args.value(QStringLiteral("pattern")).toString(),
                 args.value(QStringLiteral("path")).toString(QStringLiteral(".")));
    }
    if (name == QStringLiteral("glob")) {
        return QStringLiteral("glob %1").arg(args.value(QStringLiteral("pattern")).toString());
    }
    if (name == QStringLiteral("vibe")) {
        const QString action = args.value(QStringLiteral("action")).toString();
        const QString path = args.value(QStringLiteral("path")).toString();
        const QJsonObject nested = args.value(QStringLiteral("args")).toObject();
        const int seq = nested.value(QStringLiteral("seq")).toInt(
            args.value(QStringLiteral("seq")).toInt(0));
        static const QHash<QString, QString> labels = {
            {QStringLiteral("overview"), QStringLiteral("区块总览")},
            {QStringLiteral("read"), QStringLiteral("读区块")},
            {QStringLiteral("peek"), QStringLiteral("窥区块")},
            {QStringLiteral("replace"), QStringLiteral("替换区块")},
            {QStringLiteral("insert"), QStringLiteral("插入区块")},
            {QStringLiteral("drop"), QStringLiteral("删除区块")},
            {QStringLiteral("new"), QStringLiteral("新建区块集")},
            {QStringLiteral("split"), QStringLiteral("拆分为区块")},
            {QStringLiteral("assemble"), QStringLiteral("程序映射投影")},
            {QStringLiteral("verify"), QStringLiteral("校验区块")},
        };
        const QString label = labels.value(action, QStringLiteral("区块·%1").arg(action));
        QString base = path.isEmpty() ? label : QStringLiteral("%1 %2").arg(label, path);
        if (seq > 0
            && (action == QStringLiteral("read")
                || action == QStringLiteral("peek")
                || action == QStringLiteral("replace")
                || action == QStringLiteral("drop"))) {
            base += QStringLiteral(" · seq=%1").arg(seq);
        }
        return base;
    }
    if (name == QStringLiteral("memory")) {
        const QString act = args.value(QStringLiteral("action")).toString();
        const QString q = args.value(QStringLiteral("query")).toString();
        return act == QStringLiteral("search")
            ? QStringLiteral("memory search: %1").arg(q)
            : QStringLiteral("memory %1").arg(act);
    }
    if (name == QStringLiteral("tree")) {
        const QString act = args.value(QStringLiteral("action")).toString();
        const QString title = args.value(QStringLiteral("title")).toString();
        return act.isEmpty() ? name
                             : QStringLiteral("tree %1 %2").arg(act, title).trimmed();
    }
    if (name == QStringLiteral("apps")) {
        return QStringLiteral("apps %1 %2")
            .arg(args.value(QStringLiteral("action")).toString(),
                 args.value(QStringLiteral("title")).toString())
            .trimmed();
    }
    return name;
}

/// Prefer showing file/block line spans in the tool header when present in output.
QString extractLineSpanHint(const QString &name, const QString &inputJson, const QString &output)
{
    QRegularExpression fileLines(
        QStringLiteral(R"(file:\s*(\S+)\s+lines\s+(\d+)-(\d+))"));
    QRegularExpression vibeHeader(
        QStringLiteral(R"(\[(\d+)\]\s+rev=\d+\s+lines\s+(\d+)-(\d+))"));
    QRegularExpression vibePeek(
        QStringLiteral(R"(\[(\d+)\]\s+(.+?)\s+\(lines\s+(\d+)-(\d+)\))"));
    QRegularExpression locked(
        QStringLiteral(R"(locked lines\s+(\d+)-(\d+)\s+\(seq=(\d+))"));
    QRegularExpression jsonLines(
        QStringLiteral(R"("lines"\s*:\s*\{\s*"start"\s*:\s*(\d+)\s*,\s*"end"\s*:\s*(\d+))"));
    QRegularExpression overviewLine(
        QStringLiteral(R"(^\s*\[\s*\d+\]\s+.+?\s+lines\s+(\d+)-(\d+)\s*$)"));
    overviewLine.setPatternOptions(QRegularExpression::MultilineOption);

    auto m = fileLines.match(output);
    if (m.hasMatch()) {
        return QStringLiteral("%1 · L%2-%3")
            .arg(m.captured(1), m.captured(2), m.captured(3));
    }
    m = locked.match(output);
    if (m.hasMatch()) {
        return QStringLiteral("锁定 L%1-%2 · seq=%3")
            .arg(m.captured(1), m.captured(2), m.captured(3));
    }
    m = jsonLines.match(output);
    if (m.hasMatch() && m.captured(1) != QStringLiteral("0")) {
        return QStringLiteral("锁定 L%1-%2").arg(m.captured(1), m.captured(2));
    }
    m = vibeHeader.match(output);
    if (m.hasMatch()) {
        return QStringLiteral("seq=%1 · L%2-%3")
            .arg(m.captured(1), m.captured(2), m.captured(3));
    }
    m = vibePeek.match(output);
    if (m.hasMatch()) {
        return QStringLiteral("seq=%1 · L%2-%3 · %4")
            .arg(m.captured(1), m.captured(3), m.captured(4), m.captured(2).trimmed());
    }

    if (name == QStringLiteral("vibe")) {
        QJsonParseError err;
        const QJsonDocument doc = QJsonDocument::fromJson(inputJson.toUtf8(), &err);
        if (err.error == QJsonParseError::NoError && doc.isObject()) {
            const QString action = doc.object().value(QStringLiteral("action")).toString();
            if (action == QStringLiteral("overview")) {
                // Show first few block line ranges as a compact hint.
                QStringList spans;
                QRegularExpressionMatchIterator it = overviewLine.globalMatch(output);
                while (it.hasNext() && spans.size() < 4) {
                    const QRegularExpressionMatch om = it.next();
                    spans.append(QStringLiteral("L%1-%2").arg(om.captured(1), om.captured(2)));
                }
                if (!spans.isEmpty())
                    return spans.join(QStringLiteral(" "));
            }
        }
    }
    return QString();
}

bool shouldAutoExpandTool(const QString &name, const QString &inputJson)
{
    // Only expand block-protocol views — never dump whole source via plain read.
    if (name != QStringLiteral("vibe"))
        return false;
    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(inputJson.toUtf8(), &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject())
        return false;
    const QString action = doc.object().value(QStringLiteral("action")).toString();
    return action == QStringLiteral("read")
        || action == QStringLiteral("overview")
        || action == QStringLiteral("peek")
        || action == QStringLiteral("replace")
        || action == QStringLiteral("insert")
        || action == QStringLiteral("drop");
}

} // namespace

ChatWidget::ChatWidget(QWidget *parent)
    : QWidget(parent)
    , m_scrollArea(new QScrollArea(this))
    , m_messageContainer(new QWidget(this))
    , m_messageLayout(new QVBoxLayout(m_messageContainer))
    , m_thinkingFlushTimer(new QTimer(this))
    , m_answerFlushTimer(new QTimer(this))
    , m_scrollTimer(new QTimer(this))
{
    auto *outer = new QVBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(0);

    m_scrollArea->setObjectName(QStringLiteral("ChatScrollArea"));
    m_scrollArea->setFrameShape(QFrame::NoFrame);
    m_scrollArea->setWidgetResizable(true);
    m_scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_scrollArea->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    m_scrollArea->viewport()->setAutoFillBackground(true);

    m_messageContainer->setObjectName(QStringLiteral("chatMessages"));
    m_messageContainer->setAutoFillBackground(true);
    m_messageContainer->setMinimumWidth(0);
    m_messageContainer->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    m_messageLayout->setContentsMargins(8, 8, 8, 8);
    m_messageLayout->setSpacing(2);
    m_messageLayout->addStretch(1);

    m_scrollArea->setWidget(m_messageContainer);
    outer->addWidget(m_scrollArea, 1);
    constrainChatColumn();

    // Viewport resize is not always a ChatWidget resize (side panel / splitter).
    class ViewportResizeFilter final : public QObject
    {
    public:
        explicit ViewportResizeFilter(ChatWidget *host)
            : QObject(host)
            , m_host(host)
        {
        }
        bool eventFilter(QObject *watched, QEvent *event) override
        {
            Q_UNUSED(watched);
            if (event->type() == QEvent::Resize && m_host)
                m_host->constrainChatColumn();
            return QObject::eventFilter(watched, event);
        }

    private:
        ChatWidget *m_host = nullptr;
    };
    m_scrollArea->viewport()->installEventFilter(new ViewportResizeFilter(this));

    m_thinkingFlushTimer->setSingleShot(true);
    // Coalesce token paints — per-delta QTextDocument layout segfaults on linuxfb.
    m_answerFlushTimer->setSingleShot(true);
    m_scrollTimer->setSingleShot(true);
    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    // Board: fewer main-thread setText/layout passes during streaming.
    m_thinkingFlushTimer->setInterval(board ? 220 : 80);
    m_answerFlushTimer->setInterval(board ? 300 : 120);
    m_scrollTimer->setInterval(board ? 180 : 50);
    connect(m_thinkingFlushTimer, &QTimer::timeout, this, &ChatWidget::flushThinkingBody);
    connect(m_answerFlushTimer, &QTimer::timeout, this, &ChatWidget::flushAnswerBody);
    connect(m_scrollTimer, &QTimer::timeout, this, &ChatWidget::flushScrollToBottom);
}

void ChatWidget::scrollToBottom()
{
    m_scrollPending = true;
    if (!m_scrollTimer->isActive()) {
        m_scrollTimer->start();
    }
}

void ChatWidget::flushScrollToBottom()
{
    if (!m_scrollPending || !m_scrollArea) {
        return;
    }
    m_scrollPending = false;
    // Avoid layout storms on linuxfb while a card is actively streaming.
    if (!m_streamingMessage.row && m_messageLayout) {
        m_messageLayout->activate();
    }
    if (!m_streamingMessage.row && m_messageContainer) {
        m_messageContainer->updateGeometry();
    }
    if (auto *bar = m_scrollArea->verticalScrollBar()) {
        bar->setValue(bar->maximum());
    }
}

void ChatWidget::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    QTimer::singleShot(0, this, [this] {
        constrainChatColumn();
        if (!m_streamingMessage.row) {
            reflowMessageBodies();
        }
    });
}

void ChatWidget::constrainChatColumn()
{
    if (!m_scrollArea || !m_messageContainer)
        return;
    const int w = m_scrollArea->viewport()->width();
    if (w < 32)
        return;
    m_messageContainer->setMaximumWidth(w);
    if (m_messageContainer->width() != w)
        m_messageContainer->resize(w, m_messageContainer->height());
}

QString ChatWidget::clippedPlain(const QString &text, int maxChars)
{
    if (text.size() <= maxChars) {
        return text;
    }
    return QStringLiteral("…\n") + text.right(maxChars);
}

void ChatWidget::reflowMessageBodies()
{
    constrainChatColumn();
}

void ChatWidget::refreshFonts()
{
    // Labels pick up application font via stylesheets; nothing heavy to refresh.
    constrainChatColumn();
}

void ChatWidget::setStreamingAccent(bool active)
{
    if (!m_streamingMessage.accent)
        return;
    if (active) {
        m_streamingMessage.accent->setObjectName(QStringLiteral("msgAccentAiStream"));
    } else {
        m_streamingMessage.accent->setObjectName(QStringLiteral("msgAccentAi"));
    }
    m_streamingMessage.accent->style()->unpolish(m_streamingMessage.accent);
    m_streamingMessage.accent->style()->polish(m_streamingMessage.accent);
    m_streamingMessage.accent->update();
}

void ChatWidget::sealActiveThinking()
{
    m_thinkingFlushTimer->stop();
    if (!m_activeThinking.widget || m_activeThinking.sealed)
        return;

    flushThinkingBody();
    m_activeThinking.sealed = true;
    if (m_activeThinking.body)
        m_activeThinking.body->hide();
    if (m_activeThinking.toggle) {
        m_activeThinking.toggle->setChecked(false);
        m_activeThinking.toggle->setText(QStringLiteral(">"));
        m_activeThinking.toggle->setEnabled(!m_activeThinking.text.isEmpty());
    }
    if (m_activeThinking.summary) {
        const int chars = m_activeThinking.text.size();
        m_activeThinking.summary->setText(
            tr("思考 #%1 · 已完成 · %2 字").arg(m_activeThinking.index).arg(chars));
    }
    m_activeThinking = {};
}

void ChatWidget::openThinkingSegment()
{
    if (!m_streamingMessage.timeline)
        return;

    sealActiveAnswer();
    sealActiveThinking();
    ++m_thinkingCount;

    auto *block = new QWidget();
    block->setObjectName(QStringLiteral("thinkingBlock"));
    auto *layout = new QVBoxLayout(block);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    auto *header = new QWidget(block);
    header->setObjectName(QStringLiteral("thinkingHeader"));
    header->setMinimumHeight(24);
    auto *headerLayout = new QHBoxLayout(header);
    headerLayout->setContentsMargins(4, 0, 4, 0);
    headerLayout->setSpacing(4);

    auto *toggle = new QPushButton(QStringLiteral("v"), header);
    toggle->setObjectName(QStringLiteral("toolToggle"));
    toggle->setFixedSize(18, 18);
    toggle->setCheckable(true);
    toggle->setChecked(true);

    auto *summary = new QLabel(tr("思考 #%1 · 进行中...").arg(m_thinkingCount), header);
    summary->setObjectName(QStringLiteral("thinkingSummary"));
    configureWrappingLabel(summary);

    headerLayout->addWidget(toggle);
    headerLayout->addWidget(summary, 1);

    auto *body = new QLabel(block);
    body->setObjectName(QStringLiteral("thinkingBody"));
    body->setWordWrap(true);
    body->setTextInteractionFlags(Qt::TextSelectableByMouse);
    body->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    body->setMinimumWidth(0);
    body->setMaximumHeight(160);
    body->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    configureWrappingLabel(body);

    layout->addWidget(header);
    layout->addWidget(body);

    connect(toggle, &QPushButton::clicked, this, [toggle, body](bool checked) {
        toggle->setText(checked ? QStringLiteral("v") : QStringLiteral(">"));
        body->setVisible(checked);
    });

    m_streamingMessage.timeline->addWidget(block);
    m_activeThinking = {block, toggle, summary, body, QString(), m_thinkingCount, false};
}

void ChatWidget::flushThinkingBody()
{
    if (!m_activeThinking.body || m_activeThinking.sealed)
        return;
    const QString &text = m_activeThinking.text;
    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    const int kMaxChars = board ? 600 : 1200;
    m_activeThinking.body->setText(clippedPlain(text, kMaxChars));
    if (m_activeThinking.summary) {
        m_activeThinking.summary->setText(
            tr("思考 #%1 · 进行中 · %2 字")
                .arg(m_activeThinking.index)
                .arg(text.size()));
    }
    scrollToBottom();
}

void ChatWidget::sealActiveAnswer()
{
    if (m_answerFlushTimer) {
        m_answerFlushTimer->stop();
    }
    if (!m_activeAnswer.widget || m_activeAnswer.sealed)
        return;
    // Show full text before sealing (streaming may have shown a truncated tail).
    if (m_activeAnswer.body && !m_activeAnswer.text.isEmpty()) {
        m_activeAnswer.body->setText(m_activeAnswer.text);
    }
    m_activeAnswer.sealed = true;
    m_activeAnswer = {};
    m_streamingText.clear();
}

void ChatWidget::openAnswerSegment()
{
    if (!m_streamingMessage.timeline)
        return;

    sealActiveThinking();
    sealActiveAnswer();
    ++m_answerCount;

    auto *block = new QWidget();
    block->setObjectName(QStringLiteral("answerBlock"));
    block->setMinimumWidth(0);
    block->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    auto *layout = new QVBoxLayout(block);
    layout->setContentsMargins(0, 2, 0, 2);
    layout->setSpacing(2);

    // QLabel only: QTextBrowser/QPlainTextEdit document layout segfaults on linuxfb
    // under token streaming, and PlainTextEdit often paints "blank" at width 0.
    auto *body = new QLabel(block);
    body->setObjectName(QStringLiteral("answerBody"));
    body->setWordWrap(true);
    body->setTextInteractionFlags(Qt::TextSelectableByMouse);
    body->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    body->setMinimumWidth(0);
    body->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    configureWrappingLabel(body);
    layout->addWidget(body);
    m_streamingMessage.timeline->addWidget(block);
    m_activeAnswer = {block, body, QString(), m_answerCount, false};
    m_streamingText.clear();
}

void ChatWidget::scheduleAnswerFlush()
{
    if (!m_answerFlushTimer) {
        return;
    }
    if (!m_answerFlushTimer->isActive()) {
        m_answerFlushTimer->start();
    }
}

void ChatWidget::flushAnswerBody()
{
    if (!m_activeAnswer.body || m_activeAnswer.sealed)
        return;
    if (m_activeAnswer.text.isEmpty()) {
        m_activeAnswer.body->clear();
        return;
    }
    // Keep UI responsive: while streaming, show a bounded tail; full text on seal.
    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    const int kStreamMaxChars = board ? 2200 : 6000;
    const QString &full = m_activeAnswer.text;
    if (!m_activeAnswer.sealed && full.size() > kStreamMaxChars) {
        m_activeAnswer.body->setText(QStringLiteral("…\n") + full.right(kStreamMaxChars));
    } else {
        m_activeAnswer.body->setText(full);
    }
    scrollToBottom();
}

ChatWidget::MessageWidgets ChatWidget::createMessage(const QString &role, bool user)
{
    auto *row = new QWidget(m_messageContainer);
    row->setObjectName(QStringLiteral("messageRow"));
    row->setMinimumWidth(0);
    row->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    auto *rowLayout = new QHBoxLayout(row);
    rowLayout->setContentsMargins(0, 4, 0, 4);
    rowLayout->setSpacing(8);

    auto *accent = new QWidget(row);
    accent->setObjectName(user ? QStringLiteral("msgAccentUser") : QStringLiteral("msgAccentAi"));
    accent->setFixedWidth(3);
    accent->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);

    auto *content = new QWidget(row);
    content->setMinimumWidth(0);
    content->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    auto *contentLayout = new QVBoxLayout(content);
    contentLayout->setContentsMargins(8, 2, 12, 2);
    contentLayout->setSpacing(2);

    auto *header = new QHBoxLayout;
    header->setSpacing(8);
    auto *roleLabel = new QLabel(role, content);
    roleLabel->setObjectName(QStringLiteral("messageRole"));
    auto *meta = new QLabel(QString(), content);
    meta->setObjectName(QStringLiteral("messageMeta"));
    configureWrappingLabel(meta);
    header->addWidget(roleLabel);
    header->addStretch(1);
    header->addWidget(meta, 1);
    contentLayout->addLayout(header);

    // Chronological timeline: thinking → tools → formal answers, in order.
    auto *timeline = new QVBoxLayout;
    timeline->setContentsMargins(0, 2, 0, 0);
    timeline->setSpacing(2);
    contentLayout->addLayout(timeline);

    auto *body = new QLabel(content);
    body->setObjectName(QStringLiteral("messageBody"));
    body->setWordWrap(true);
    body->setTextInteractionFlags(Qt::TextSelectableByMouse);
    body->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    body->setMinimumWidth(0);
    body->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    configureWrappingLabel(body);
    contentLayout->addWidget(body);
    // Assistant formal text lives in timeline answer segments; keep footer body for user/history.
    if (!user) {
        body->hide();
    }

    rowLayout->addWidget(accent);
    rowLayout->addWidget(content, 1);

    m_messageLayout->insertWidget(m_messageLayout->count() - 1, row);
    return {row, accent, content, roleLabel, meta, timeline, body, user};
}

ChatWidget::ToolBlock ChatWidget::createToolBlock(
    const QString &iconChar, const QString &name,
    const QString &detail, const QString &output,
    bool ok, bool autoExpand)
{
    auto *block = new QWidget();
    block->setObjectName(QStringLiteral("toolBlock"));
    block->setMinimumWidth(0);
    block->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    auto *layout = new QVBoxLayout(block);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    auto *header = new QWidget(block);
    header->setObjectName(QStringLiteral("toolHeader"));
    header->setMinimumHeight(24);
    auto *headerLayout = new QHBoxLayout(header);
    headerLayout->setContentsMargins(4, 0, 4, 0);
    headerLayout->setSpacing(4);

    auto *toggle = new QPushButton(QStringLiteral(">"), header);
    toggle->setObjectName(QStringLiteral("toolToggle"));
    toggle->setFixedSize(18, 18);
    toggle->setCheckable(true);
    toggle->setEnabled(!output.isEmpty());

    auto *icon = new QLabel(iconChar, header);
    icon->setObjectName(ok ? QStringLiteral("toolOk") : QStringLiteral("toolFail"));
    icon->setFixedWidth(22);

    auto *nameLabel = new QLabel(name, header);
    nameLabel->setObjectName(QStringLiteral("toolName"));
    configureWrappingLabel(nameLabel);

    auto *detailLabel = new QLabel(detail, header);
    detailLabel->setObjectName(QStringLiteral("toolDetail"));
    detailLabel->setTextFormat(Qt::PlainText);
    configureWrappingLabel(detailLabel);

    headerLayout->addWidget(toggle);
    headerLayout->addWidget(icon);
    headerLayout->addWidget(nameLabel);
    headerLayout->addWidget(detailLabel, 1);

    // Never feed multi-MB tool dumps into QTextDocument on the board.
    auto *outputWidget = new QLabel(block);
    outputWidget->setObjectName(QStringLiteral("toolOutput"));
    outputWidget->setWordWrap(true);
    outputWidget->setTextInteractionFlags(Qt::TextSelectableByMouse);
    outputWidget->setTextFormat(Qt::PlainText);
    outputWidget->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    outputWidget->setMinimumWidth(0);
    outputWidget->setMaximumHeight(autoExpand ? 240 : 120);
    outputWidget->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    outputWidget->setText(clippedPlain(output, 2500));
    configureWrappingLabel(outputWidget);
    const bool expand = autoExpand && !output.isEmpty();
    outputWidget->setVisible(expand);
    if (expand) {
        toggle->setChecked(true);
        toggle->setText(QStringLiteral("v"));
    } else {
        outputWidget->hide();
    }

    layout->addWidget(header);
    layout->addWidget(outputWidget);

    connect(toggle, &QPushButton::clicked, this, [toggle, outputWidget](bool checked) {
        toggle->setText(checked ? QStringLiteral("v") : QStringLiteral(">"));
        outputWidget->setVisible(checked);
    });

    if (m_streamingMessage.timeline) {
        m_streamingMessage.timeline->addWidget(block);
    }

    return {block, icon, nameLabel, detailLabel, toggle, outputWidget, expand};
}

void ChatWidget::appendUserMessage(const QString &text)
{
    MessageWidgets msg = createMessage(tr("你"), true);
    msg.body->setText(text);
    scrollToBottom();
}

void ChatWidget::beginAssistantMessage()
{
    // One user turn = one assistant card. Each Thinking/Answer round is a timeline segment.
    if (m_streamingMessage.row) {
        sealActiveAnswer();
        m_streamingMessage.meta->setText(tr("思考中..."));
        setStreamingAccent(true);
        openThinkingSegment();
        scrollToBottom();
        return;
    }

    m_streamingText.clear();
    m_pendingToolNames.clear();
    m_pendingToolInputs.clear();
    m_pendingToolBlocks.clear();
    m_toolStep = 0;
    m_thinkingCount = 0;
    m_answerCount = 0;
    m_activeThinking = {};
    m_activeAnswer = {};
    m_streamingMessage = createMessage(tr("MoonCoding"), false);
    m_streamingMessage.meta->setText(tr("思考中..."));
    setStreamingAccent(true);
    openThinkingSegment();
    scrollToBottom();
}

void ChatWidget::appendThinkingDelta(const QString &delta)
{
    if (!m_streamingMessage.row)
        beginAssistantMessage();
    sealActiveAnswer();
    if (!m_activeThinking.widget || m_activeThinking.sealed)
        openThinkingSegment();

    m_activeThinking.text += delta;
    m_streamingMessage.meta->setText(tr("思考中..."));
    setStreamingAccent(true);
    if (!m_thinkingFlushTimer->isActive())
        m_thinkingFlushTimer->start();
}

void ChatWidget::appendAssistantDelta(const QString &delta)
{
    if (!m_streamingMessage.row)
        beginAssistantMessage();
    sealActiveThinking();
    if (!m_activeAnswer.widget || m_activeAnswer.sealed)
        openAnswerSegment();

    m_activeAnswer.text += delta;
    m_streamingText = m_activeAnswer.text;
    m_streamingMessage.meta->setText(tr("输出中"));
    scheduleAnswerFlush();
}

void ChatWidget::finishAssistantMessage(
    const QString &content, quint64 tokensIn, quint64 tokensOut)
{
    if (!m_streamingMessage.row)
        m_streamingMessage = createMessage(tr("MoonCoding"), false);

    sealActiveThinking();
    const QString finalContent = content.isEmpty()
        ? (m_activeAnswer.widget ? m_activeAnswer.text : m_streamingText)
        : content;

    if (!finalContent.isEmpty()) {
        if (!m_activeAnswer.widget || m_activeAnswer.sealed)
            openAnswerSegment();
        m_activeAnswer.text = finalContent;
        m_streamingText = finalContent;
        sealActiveAnswer();
    }

    if (tokensIn + tokensOut > 0) {
        m_streamingMessage.meta->setText(
            tr("%1 / %2 tokens").arg(tokensIn).arg(tokensOut));
    } else {
        m_streamingMessage.meta->setText(tr("准备调用工具"));
    }

    setStreamingAccent(false);
    scrollToBottom();
}

void ChatWidget::showToolStart(const QString &id, const QString &name, const QString &input)
{
    m_pendingToolNames.insert(id, name);
    m_pendingToolInputs.insert(id, input);
    m_toolStep++;

    const QString cmdPreview = describeToolCall(name, input);
    if (!m_streamingMessage.row)
        beginAssistantMessage();
    sealActiveThinking();
    sealActiveAnswer();

    m_streamingMessage.meta->setText(tr("步骤 %1 · %2").arg(m_toolStep).arg(cmdPreview));

    if (m_pendingToolBlocks.contains(id))
        return;

    ToolBlock block = createToolBlock(
        QStringLiteral("..."),
        name,
        cmdPreview,
        QString(),
        true,
        false);
    m_pendingToolBlocks.insert(id, block);
    scrollToBottom();
}

void ChatWidget::showToolResult(
    const QString &id, const QString &name, const QString &output,
    int exitCode, quint64 durationMs)
{
    const QString displayName = name.isEmpty()
        ? m_pendingToolNames.value(id, QStringLiteral("?")) : name;
    const QString inputJson = m_pendingToolInputs.value(id);
    m_pendingToolNames.remove(id);
    m_pendingToolInputs.remove(id);

    const bool ok = exitCode == 0;
    const QString duration = durationMs < 1000
        ? tr("%1 ms").arg(durationMs)
        : tr("%1 s").arg(durationMs / 1000.0, 0, 'f', 1);
    const QString cmdDesc = describeToolCall(displayName, inputJson);
    const QString spanHint = extractLineSpanHint(displayName, inputJson, output);
    const bool autoExpand = shouldAutoExpandTool(displayName, inputJson);

    QString outputPreview;
    if (!spanHint.isEmpty()) {
        outputPreview = spanHint;
    } else if (!output.isEmpty()) {
        const int cut = output.indexOf('\n');
        outputPreview = output.left(cut < 0 ? qMin(120, output.size()) : qMin(cut, 120))
                            .trimmed();
    }

    const QString detail = ok
        ? (outputPreview.isEmpty()
               ? tr("%1 · %2").arg(duration, cmdDesc)
               : tr("%1 · %2 · %3").arg(duration, cmdDesc, outputPreview))
        : tr("退出 %1 · %2 · %3").arg(exitCode).arg(duration, cmdDesc);

    if (!m_streamingMessage.row)
        beginAssistantMessage();
    sealActiveThinking();
    sealActiveAnswer();

    if (m_pendingToolBlocks.contains(id)) {
        ToolBlock block = m_pendingToolBlocks.take(id);
        if (block.icon) {
            block.icon->setText(ok ? QStringLiteral("OK") : QStringLiteral("X"));
            block.icon->setObjectName(ok ? QStringLiteral("toolOk") : QStringLiteral("toolFail"));
            block.icon->style()->unpolish(block.icon);
            block.icon->style()->polish(block.icon);
        }
        if (block.name)
            block.name->setText(displayName);
        if (block.detail)
            block.detail->setText(detail);
        if (block.output) {
            block.output->setText(clippedPlain(output, 2500));
            block.output->setMaximumHeight(autoExpand ? 240 : 120);
            const bool expand = autoExpand && !output.isEmpty();
            block.output->setVisible(expand);
            if (block.toggle) {
                block.toggle->setEnabled(!output.isEmpty());
                block.toggle->setChecked(expand);
                block.toggle->setText(expand ? QStringLiteral("v") : QStringLiteral(">"));
            }
        } else if (block.toggle) {
            block.toggle->setEnabled(!output.isEmpty());
            block.toggle->setChecked(false);
            block.toggle->setText(QStringLiteral(">"));
        }
    } else {
        createToolBlock(
            ok ? QStringLiteral("OK") : QStringLiteral("X"),
            displayName,
            detail,
            output,
            ok,
            autoExpand);
    }

    m_streamingMessage.meta->setText(tr("步骤 %1 · %2").arg(m_toolStep).arg(cmdDesc));
    scrollToBottom();
}

void ChatWidget::showError(const QString &message)
{
    finalizeCard();
    MessageWidgets err = createMessage(tr("错误"), false);
    err.accent->setObjectName(QStringLiteral("msgAccentError"));
    err.accent->style()->unpolish(err.accent);
    err.accent->style()->polish(err.accent);
    err.accent->update();

    QString text = message.trimmed();
    const QString lower = text.toLower();
    if (text.isEmpty()) {
        text = tr("未知错误（无详细信息）。请检查网络后重试。");
    } else if (lower.contains(QStringLiteral("dns error"))
               || lower.contains(QStringLiteral("name resolution"))
               || lower.contains(QStringLiteral("nodename nor servname"))
               || lower.contains(QStringLiteral("temporary failure in name resolution"))) {
        text = tr("网络 DNS 解析失败，无法连接 API。\n"
                  "请点顶栏网络状态长按「一键恢复」，或打开 WiFi 页点「一键恢复网络」。\n\n原始错误：%1")
                   .arg(message);
    } else if (lower.contains(QStringLiteral("connection refused"))
               || lower.contains(QStringLiteral("network is unreachable"))
               || lower.contains(QStringLiteral("timed out"))
               || lower.contains(QStringLiteral("timeout"))
               || lower.contains(QStringLiteral("connect error"))
               || lower.contains(QStringLiteral("error sending request"))) {
        text = tr("无法连接 API 服务器（网络中断或超时）。\n"
                  "请长按顶栏网络状态，或到 WiFi 页点「一键恢复网络」。\n\n原始错误：%1")
                   .arg(message);
    } else if (lower.contains(QStringLiteral("certificate"))
               || lower.contains(QStringLiteral("notvalidyet"))
               || lower.contains(QStringLiteral("not valid yet"))
               || lower.contains(QStringLiteral("invalidcertificate"))) {
        text = tr("系统时间异常导致 HTTPS 证书校验失败。\n"
                  "设备无电池时钟，断网后容易回到 1970 年。\n"
                  "请恢复网络后重试（开机脚本会自动校时）。\n\n原始错误：%1")
                   .arg(message);
    }

    err.body->show();
    err.body->setText(text);
    err.meta->setText(QString());
    scrollToBottom();
}

void ChatWidget::showInterrupted(const QString &reason)
{
    finalizeCard();
    MessageWidgets msg = createMessage(tr("已中断"), false);
    msg.accent->setObjectName(QStringLiteral("msgAccentError"));
    msg.accent->style()->unpolish(msg.accent);
    msg.accent->style()->polish(msg.accent);
    msg.accent->update();
    msg.body->show();
    msg.body->setText(reason);
    msg.meta->setText(QString());
    scrollToBottom();
}

void ChatWidget::agentDone(quint64 tokensIn, quint64 tokensOut, quint64 steps)
{
    if (!m_streamingMessage.row) return;

    sealActiveThinking();
    sealActiveAnswer();
    m_streamingMessage.meta->setText(
        tr("%1 / %2 tokens · %3 步").arg(tokensIn).arg(tokensOut).arg(steps));
    finalizeCard();
}

void ChatWidget::finalizeCard()
{
    if (!m_streamingMessage.row) return;
    sealActiveThinking();
    sealActiveAnswer();
    if (m_streamingMessage.accent) {
        m_streamingMessage.accent->setObjectName(QStringLiteral("msgAccentAi"));
        m_streamingMessage.accent->style()->unpolish(m_streamingMessage.accent);
        m_streamingMessage.accent->style()->polish(m_streamingMessage.accent);
        m_streamingMessage.accent->update();
    }
    m_streamingMessage = {};
    m_streamingText.clear();
    m_pendingToolNames.clear();
    m_pendingToolInputs.clear();
    m_pendingToolBlocks.clear();
    m_toolStep = 0;
    m_thinkingCount = 0;
    m_answerCount = 0;
}

void ChatWidget::setMessages(const QJsonArray &messages)
{
    clear();
    for (const QJsonValue &value : messages) {
        const QJsonObject msg = value.toObject();
        const QString role = msg.value(QStringLiteral("role")).toString();
        const QString content = msg.value(QStringLiteral("content")).toString();
        if (role == QStringLiteral("user")) {
            appendUserMessage(content);
        } else if (role == QStringLiteral("assistant")) {
            MessageWidgets restored = createMessage(tr("MoonCoding"), false);
            if (restored.body) {
                restored.body->show();
                restored.body->setText(content);
            }
            restored.meta->setText(tr("历史"));
        }
    }
    scrollToBottom();
}

void ChatWidget::clear()
{
    m_thinkingFlushTimer->stop();
    if (m_answerFlushTimer) {
        m_answerFlushTimer->stop();
    }
    while (m_messageLayout->count() > 1) {
        QLayoutItem *item = m_messageLayout->takeAt(0);
        if (item) {
            delete item->widget();
            delete item;
        }
    }
    m_streamingMessage = {};
    m_activeThinking = {};
    m_activeAnswer = {};
    m_streamingText.clear();
    m_pendingToolNames.clear();
    m_pendingToolInputs.clear();
    m_pendingToolBlocks.clear();
    m_toolStep = 0;
    m_thinkingCount = 0;
    m_answerCount = 0;
}
