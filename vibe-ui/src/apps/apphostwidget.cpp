#include "apphostwidget.h"

#include "appschemarenderer.h"
#include "opencode_antialias.h"

#include <QAbstractAnimation>
#include <QEasingCurve>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QPointer>
#include <QPropertyAnimation>
#include <QResizeEvent>
#include <QSignalBlocker>
#include <QSizePolicy>
#include <QStyle>
#include <QStringList>
#include <QToolButton>
#include <QVBoxLayout>

AppHostWidget::AppHostWidget(QWidget *parent)
    : QWidget(parent)
    , m_toolbar(new QWidget(this))
    , m_titleLabel(new QLabel(this))
    , m_statusLabel(new QLabel(this))
    , m_backButton(new QToolButton(this))
    , m_startButton(new QToolButton(this))
    , m_stopButton(new QToolButton(this))
    , m_restartButton(new QToolButton(this))
    , m_logsButton(new QToolButton(this))
    , m_errorBanner(new QLabel(this))
    , m_renderer(new AppSchemaRenderer(this))
    , m_logDrawer(new QPlainTextEdit(this))
    , m_logAnimation(
          new QPropertyAnimation(m_logDrawer, "maximumHeight", this))
{
    setObjectName(QStringLiteral("appHost"));

    auto *rootLayout = new QVBoxLayout(this);
    rootLayout->setContentsMargins(0, 0, 0, 0);
    rootLayout->setSpacing(0);

    m_toolbar->setObjectName(QStringLiteral("appHostToolbar"));
    m_toolbar->setMinimumHeight(52);
    auto *toolbarLayout = new QHBoxLayout(m_toolbar);
    toolbarLayout->setContentsMargins(6, 4, 6, 4);
    toolbarLayout->setSpacing(4);

    const auto prepareButton = [](QToolButton *button,
                                  const QString &objectName) {
        button->setObjectName(objectName);
        button->setAutoRaise(true);
        button->setFocusPolicy(Qt::StrongFocus);
        button->setMinimumSize(44, 44);
        button->setSizePolicy(QSizePolicy::Minimum, QSizePolicy::Fixed);
    };

    prepareButton(m_backButton, QStringLiteral("appHostBack"));
    m_backButton->setText(QStringLiteral("<-"));
    m_backButton->setToolTip(tr("返回应用列表"));
    m_backButton->setAccessibleName(tr("返回应用列表"));

    m_titleLabel->setObjectName(QStringLiteral("appHostTitle"));
    m_titleLabel->setTextFormat(Qt::PlainText);
    m_titleLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    m_titleLabel->setMinimumWidth(80);

    m_statusLabel->setObjectName(QStringLiteral("appHostStatus"));
    m_statusLabel->setTextFormat(Qt::PlainText);
    m_statusLabel->setAlignment(Qt::AlignCenter);
    m_statusLabel->setSizePolicy(QSizePolicy::Minimum, QSizePolicy::Preferred);

    prepareButton(m_startButton, QStringLiteral("appHostStart"));
    m_startButton->setText(QStringLiteral("▶"));
    m_startButton->setToolTip(tr("启动应用"));
    m_startButton->setAccessibleName(tr("启动应用"));

    prepareButton(m_stopButton, QStringLiteral("appHostStop"));
    m_stopButton->setText(QStringLiteral("■"));
    m_stopButton->setToolTip(tr("停止应用"));
    m_stopButton->setAccessibleName(tr("停止应用"));

    prepareButton(m_restartButton, QStringLiteral("appHostRestart"));
    m_restartButton->setText(QStringLiteral("↻"));
    m_restartButton->setToolTip(tr("重新启动"));
    m_restartButton->setAccessibleName(tr("重新启动"));

    prepareButton(m_logsButton, QStringLiteral("appHostLogs"));
    m_logsButton->setText(tr("日志"));
    m_logsButton->setToolTip(tr("显示日志"));
    m_logsButton->setAccessibleName(tr("显示日志"));
    m_logsButton->setCheckable(true);

    toolbarLayout->addWidget(m_backButton);
    toolbarLayout->addWidget(m_titleLabel, 1);
    toolbarLayout->addWidget(m_statusLabel);
    toolbarLayout->addWidget(m_startButton);
    toolbarLayout->addWidget(m_stopButton);
    toolbarLayout->addWidget(m_restartButton);
    toolbarLayout->addWidget(m_logsButton);

    m_errorBanner->setObjectName(QStringLiteral("appHostError"));
    m_errorBanner->setTextFormat(Qt::PlainText);
    m_errorBanner->setWordWrap(true);
    m_errorBanner->setContentsMargins(12, 8, 12, 8);
    m_errorBanner->hide();

    m_renderer->setObjectName(QStringLiteral("appSchemaSurface"));

    m_logDrawer->setObjectName(QStringLiteral("appLogDrawer"));
    m_logDrawer->setReadOnly(true);
    m_logDrawer->setLineWrapMode(QPlainTextEdit::NoWrap);
    m_logDrawer->setPlaceholderText(tr("暂无运行日志"));
    m_logDrawer->setMaximumBlockCount(2000);
    m_logDrawer->setMinimumHeight(0);
    m_logDrawer->setMaximumHeight(0);
    m_logDrawer->setFont(opencode::monospaceFont(12));
    m_logDrawer->hide();

    m_logAnimation->setDuration(160);
    m_logAnimation->setEasingCurve(QEasingCurve::OutCubic);

    rootLayout->addWidget(m_toolbar);
    rootLayout->addWidget(m_errorBanner);
    rootLayout->addWidget(m_renderer, 1);
    rootLayout->addWidget(m_logDrawer);

    connect(m_backButton, &QToolButton::clicked, this,
            [this] { emit backRequested(); });
    connect(m_startButton, &QToolButton::clicked, this,
            [this] { emit startRequested(); });
    connect(m_stopButton, &QToolButton::clicked, this,
            [this] { emit stopRequested(); });
    connect(m_restartButton, &QToolButton::clicked, this,
            [this] { emit restartRequested(); });
    connect(m_logsButton, &QToolButton::toggled, this,
            [this](bool checked) { setLogsExpanded(checked); });
    connect(m_renderer, &AppSchemaRenderer::uiEvent, this,
            [this](const QJsonObject &event) { emit uiEvent(event); });

    const QPointer<QPlainTextEdit> guardedDrawer(m_logDrawer);
    connect(m_logAnimation, &QPropertyAnimation::finished, this,
            [this, guardedDrawer] {
                if (!guardedDrawer) {
                    return;
                }
                if (m_logsExpanded) {
                    guardedDrawer->show();
                    guardedDrawer->setMaximumHeight(expandedLogHeight());
                } else {
                    guardedDrawer->hide();
                }
            });

    setApp(QString(), QString());
    updateRuntimeUi();
}

void AppHostWidget::setApp(const QString &title, const QString &name)
{
    m_appName = name;
    const QString displayTitle = title.isEmpty()
        ? (name.isEmpty() ? tr("应用") : name)
        : title;
    m_titleLabel->setText(displayTitle);
    m_titleLabel->setToolTip(name.isEmpty() ? displayTitle : name);
    m_titleLabel->setAccessibleName(
        tr("应用标题：%1").arg(displayTitle));
    updateRuntimeUi();
}

void AppHostWidget::setRuntimeState(RuntimeState state)
{
    m_runtimeState = state;
    updateRuntimeUi();
}

bool AppHostWidget::setSchema(const QJsonObject &schema, QString *error)
{
    QString validationError;
    const bool accepted = m_renderer->setSchema(schema, &validationError);
    if (accepted) {
        m_schemaError.clear();
    } else {
        m_schemaError = tr(
            "应用界面不符合规范。宿主仍可使用，请查看日志或重新启动。");
    }
    updateErrorBanner();

    if (error) {
        *error = validationError;
    }
    return accepted;
}

void AppHostWidget::applyPatch(const QJsonObject &patch)
{
    m_renderer->applyPatch(patch);
}

void AppHostWidget::appendLog(const QString &text)
{
    m_logDrawer->appendPlainText(text);
}

void AppHostWidget::clearLogs()
{
    m_logDrawer->clear();
}

void AppHostWidget::revealLogs()
{
    if (!m_logsExpanded) {
        setLogsExpanded(true);
    }
    m_logsButton->setChecked(true);
}

void AppHostWidget::resizeEvent(QResizeEvent *event)
{
    QWidget::resizeEvent(event);
    if (m_logsExpanded
        && m_logAnimation->state() == QAbstractAnimation::Stopped) {
        m_logDrawer->setMaximumHeight(expandedLogHeight());
    }
}

void AppHostWidget::setLogsExpanded(bool expanded)
{
    if (m_logsExpanded == expanded
        && m_logAnimation->state() == QAbstractAnimation::Stopped) {
        return;
    }

    m_logsExpanded = expanded;
    if (m_logsButton->isChecked() != expanded) {
        const QSignalBlocker blocker(m_logsButton);
        m_logsButton->setChecked(expanded);
    }
    m_logsButton->setToolTip(expanded ? tr("隐藏日志") : tr("显示日志"));
    m_logsButton->setAccessibleName(
        expanded ? tr("隐藏日志") : tr("显示日志"));

    m_logAnimation->stop();
    int currentHeight = m_logDrawer->maximumHeight();
    if (currentHeight < 0 || currentHeight >= QWIDGETSIZE_MAX) {
        currentHeight = m_logDrawer->height();
    }

    if (expanded) {
        m_logDrawer->show();
        currentHeight = qMax(0, currentHeight);
        m_logDrawer->setMaximumHeight(currentHeight);
        m_logAnimation->setStartValue(currentHeight);
        m_logAnimation->setEndValue(expandedLogHeight());
    } else {
        currentHeight = qMax(0, m_logDrawer->height());
        m_logDrawer->setMaximumHeight(currentHeight);
        m_logAnimation->setStartValue(currentHeight);
        m_logAnimation->setEndValue(0);
    }
    m_logAnimation->start();
}

void AppHostWidget::updateRuntimeUi()
{
    QString statusText;
    QString stateName;

    switch (m_runtimeState) {
    case RuntimeState::Idle:
        statusText = tr("未运行");
        stateName = QStringLiteral("idle");
        break;
    case RuntimeState::Starting:
        statusText = tr("正在启动…");
        stateName = QStringLiteral("starting");
        break;
    case RuntimeState::Running:
        statusText = tr("● 运行中");
        stateName = QStringLiteral("running");
        break;
    case RuntimeState::Stopping:
        statusText = tr("正在停止…");
        stateName = QStringLiteral("stopping");
        break;
    case RuntimeState::Error:
        statusText = tr("运行异常");
        stateName = QStringLiteral("error");
        break;
    }

    m_statusLabel->setText(statusText);
    m_statusLabel->setAccessibleName(tr("应用状态：%1").arg(statusText));
    m_statusLabel->setProperty("runtimeState", stateName);
    m_statusLabel->style()->unpolish(m_statusLabel);
    m_statusLabel->style()->polish(m_statusLabel);

    const bool hasApp = !m_appName.isEmpty();
    const bool showStart = m_runtimeState == RuntimeState::Idle
        || m_runtimeState == RuntimeState::Error;
    const bool showStop = !showStart;

    m_startButton->setVisible(showStart);
    m_startButton->setEnabled(hasApp && showStart);
    m_stopButton->setVisible(showStop);
    m_stopButton->setEnabled(
        hasApp && (m_runtimeState == RuntimeState::Starting
                   || m_runtimeState == RuntimeState::Running));
    m_restartButton->setEnabled(
        hasApp && (m_runtimeState == RuntimeState::Running
                   || m_runtimeState == RuntimeState::Error));

    updateErrorBanner();
}

void AppHostWidget::updateErrorBanner()
{
    QStringList messages;
    if (m_runtimeState == RuntimeState::Error) {
        messages.append(tr(
            "应用意外停止。你可以重新启动，或展开日志查看原因。"));
    }
    if (!m_schemaError.isEmpty()) {
        messages.append(m_schemaError);
    }

    m_errorBanner->setText(messages.join(QLatin1Char('\n')));
    m_errorBanner->setVisible(!messages.isEmpty());
}

int AppHostWidget::expandedLogHeight() const
{
    return qBound(120, height() / 3, 240);
}
