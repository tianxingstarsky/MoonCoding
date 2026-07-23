// =============================================================================
// opencode_composer.cpp — Composer input implementation
// =============================================================================

#include "opencode_composer.h"
#include "opencode_input_widget.h"
#include "opencode_dialog_overlay.h"

#include <QMenu>
#include <QAction>
#include <QFont>
#include <QApplication>

// =============================================================================
// AddMenuButton implementation
// =============================================================================

AddMenuButton::AddMenuButton(QWidget* parent)
    : QPushButton(parent)
{
    setObjectName(QStringLiteral("AddMenuButton"));
    setText(QStringLiteral("+"));
    setFixedSize(32, 32);
    setToolTip(tr("Add files, commands, or context"));
    setCursor(Qt::PointingHandCursor);

    m_menu = new QMenu(this);
    m_menu->setObjectName(QStringLiteral("AddMenu"));

    auto* addFilesAction = m_menu->addAction(tr("Add Files..."));
    connect(addFilesAction, &QAction::triggered, this, &AddMenuButton::addFilesRequested);

    auto* addCmdAction = m_menu->addAction(tr("Add Command..."));
    connect(addCmdAction, &QAction::triggered, this, &AddMenuButton::addCommandRequested);

    auto* addCtxAction = m_menu->addAction(tr("Add Context..."));
    connect(addCtxAction, &QAction::triggered, this, &AddMenuButton::addContextRequested);

    m_menu->addSeparator();

    auto* shellAction = m_menu->addAction(tr("Shell Mode"));
    shellAction->setCheckable(true);
    connect(shellAction, &QAction::toggled, this, &AddMenuButton::shellModeToggled);

    connect(this, &QPushButton::clicked, this, [this]() {
        QPoint pos = mapToGlobal(QPoint(0, height()));
        m_menu->popup(pos);
    });
}

// =============================================================================
// OpenCodeComposer implementation
// =============================================================================

OpenCodeComposer::OpenCodeComposer(QWidget* parent)
    : QWidget(parent)
{
    setupUi();
}

void OpenCodeComposer::setupUi()
{
    setObjectName(QStringLiteral("OpenCodeComposer"));

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // -----------------------------------------------------------------------
    // Context bar — model name + token count + step count
    // -----------------------------------------------------------------------
    m_contextBar = new QWidget(this);
    m_contextBar->setObjectName(QStringLiteral("ComposerContextBar"));
    m_contextBar->setFixedHeight(28);

    auto* ctxLayout = new QHBoxLayout(m_contextBar);
    ctxLayout->setContentsMargins(12, 2, 12, 2);
    ctxLayout->setSpacing(16);

    m_contextModelLabel = new QLabel(m_contextBar);
    m_contextModelLabel->setObjectName(QStringLiteral("ComposerModelLabel"));
    QFont ctxFont = m_contextModelLabel->font();
    ctxFont.setPixelSize(12);
    m_contextModelLabel->setFont(ctxFont);
    ctxLayout->addWidget(m_contextModelLabel);

    m_contextTokenLabel = new QLabel(m_contextBar);
    m_contextTokenLabel->setObjectName(QStringLiteral("ComposerTokenLabel"));
    m_contextTokenLabel->setFont(ctxFont);
    ctxLayout->addWidget(m_contextTokenLabel);

    m_contextStepLabel = new QLabel(m_contextBar);
    m_contextStepLabel->setObjectName(QStringLiteral("ComposerStepLabel"));
    m_contextStepLabel->setFont(ctxFont);
    ctxLayout->addWidget(m_contextStepLabel);

    ctxLayout->addStretch();
    mainLayout->addWidget(m_contextBar);

    // -----------------------------------------------------------------------
    // Toolbar — model selector + add menu + submit/stop
    // -----------------------------------------------------------------------
    auto* toolbar = new QWidget(this);
    toolbar->setObjectName(QStringLiteral("ComposerToolbar"));
    toolbar->setFixedHeight(40);

    auto* toolbarLayout = new QHBoxLayout(toolbar);
    toolbarLayout->setContentsMargins(8, 4, 8, 4);
    toolbarLayout->setSpacing(6);

    // Model selector button.
    m_modelBtn = new QPushButton(toolbar);
    m_modelBtn->setObjectName(QStringLiteral("ModelSelectButton"));
    m_modelBtn->setText(tr("Model"));
    m_modelBtn->setFixedHeight(32);
    m_modelBtn->setMinimumWidth(120);
    m_modelBtn->setCursor(Qt::PointingHandCursor);
    QFont modelBtnFont = m_modelBtn->font();
    modelBtnFont.setPixelSize(12);
    m_modelBtn->setFont(modelBtnFont);
    connect(m_modelBtn, &QPushButton::clicked, this, &OpenCodeComposer::onModelButtonClicked);
    toolbarLayout->addWidget(m_modelBtn);

    // Add menu button.
    m_addBtn = new AddMenuButton(toolbar);
    toolbarLayout->addWidget(m_addBtn);

    toolbarLayout->addStretch();

    // Submit button.
    m_submitBtn = new QPushButton(tr("Send"), toolbar);
    m_submitBtn->setObjectName(QStringLiteral("ComposerSubmitBtn"));
    m_submitBtn->setFixedSize(56, 32);
    m_submitBtn->setEnabled(false);
    m_submitBtn->setVisible(false);
    connect(m_submitBtn, &QPushButton::clicked, this, &OpenCodeComposer::onSubmitClicked);
    toolbarLayout->addWidget(m_submitBtn);

    // Stop button.
    m_stopBtn = new QPushButton(QStringLiteral("■"), toolbar);
    m_stopBtn->setObjectName(QStringLiteral("ComposerStopBtn"));
    m_stopBtn->setFixedSize(32, 32);
    m_stopBtn->setToolTip(tr("Stop generating (Esc)"));
    m_stopBtn->setVisible(false);
    connect(m_stopBtn, &QPushButton::clicked, this, &OpenCodeComposer::onStopClicked);
    toolbarLayout->addWidget(m_stopBtn);

    mainLayout->addWidget(toolbar);

    // -----------------------------------------------------------------------
    // Reuse existing OpenCodeInputWidget for the text area
    // -----------------------------------------------------------------------
    m_inputWidget = new OpenCodeInputWidget(this);
    m_inputWidget->setObjectName(QStringLiteral("ComposerInput"));
    mainLayout->addWidget(m_inputWidget);

    // Forward signals from input widget.
    connect(m_inputWidget, &OpenCodeInputWidget::messageSubmitted,
            this, &OpenCodeComposer::messageSubmitted);
    connect(m_inputWidget, &OpenCodeInputWidget::stopRequested,
            this, &OpenCodeComposer::stopRequested);
    connect(m_inputWidget, &OpenCodeInputWidget::textChanged,
            this, [this]() {
        bool hasText = !m_inputWidget->text().trimmed().isEmpty();
        m_submitBtn->setVisible(hasText && !m_agentBusy);
        m_submitBtn->setEnabled(hasText);
        emit textChanged();
    });

    // Forward add menu signals.
    connect(m_addBtn, &AddMenuButton::addFilesRequested,
            this, &OpenCodeComposer::addFilesRequested);
    connect(m_addBtn, &AddMenuButton::addCommandRequested,
            this, &OpenCodeComposer::addCommandRequested);
    connect(m_addBtn, &AddMenuButton::addContextRequested,
            this, &OpenCodeComposer::addContextRequested);

    updateContextLabel();
}

// =============================================================================
// Public API
// =============================================================================

void OpenCodeComposer::setText(const QString& text)
{
    m_inputWidget->setText(text);
}

QString OpenCodeComposer::text() const
{
    return m_inputWidget->text();
}

void OpenCodeComposer::clear()
{
    m_inputWidget->clear();
}

void OpenCodeComposer::focusInput()
{
    m_inputWidget->focusInput();
}

void OpenCodeComposer::setAgentBusy(bool busy)
{
    m_agentBusy = busy;
    m_stopBtn->setVisible(busy);
    m_submitBtn->setVisible(!busy && !m_inputWidget->text().trimmed().isEmpty());
    m_inputWidget->setAgentBusy(busy);
}

bool OpenCodeComposer::isAgentBusy() const
{
    return m_agentBusy;
}

void OpenCodeComposer::setModelInfo(const QString& provider, const QString& model)
{
    m_currentProvider = provider;
    m_currentModel = model;
    updateModelButtonLabel();
    m_inputWidget->setModelInfo(model);
}

void OpenCodeComposer::setTokenCount(int count)
{
    m_tokenCount = count;
    updateContextLabel();
    m_inputWidget->setTokenCount(count);
}

void OpenCodeComposer::setStepCount(int count)
{
    m_stepCount = count;
    updateContextLabel();
    m_inputWidget->setStepCount(count);
}

void OpenCodeComposer::addAvailableModel(const QString& provider, const QString& model,
                                           const QString& description)
{
    m_availableModels.append({provider, model, description});
}

void OpenCodeComposer::saveDraft(QVariantMap& draft) const
{
    draft[QStringLiteral("text")] = text();
    draft[QStringLiteral("model")] = m_currentModel;
    draft[QStringLiteral("provider")] = m_currentProvider;
}

void OpenCodeComposer::restoreDraft(const QVariantMap& draft)
{
    if (draft.contains(QStringLiteral("text")))
        setText(draft[QStringLiteral("text")].toString());
    if (draft.contains(QStringLiteral("provider")))
        m_currentProvider = draft[QStringLiteral("provider")].toString();
    if (draft.contains(QStringLiteral("model")))
        m_currentModel = draft[QStringLiteral("model")].toString();
    updateModelButtonLabel();
}

void OpenCodeComposer::setInputEnabled(bool enabled)
{
    m_inputWidget->setInputEnabled(enabled);
    m_modelBtn->setEnabled(enabled);
    m_addBtn->setEnabled(enabled);
}

// =============================================================================
// Private helpers
// =============================================================================

void OpenCodeComposer::updateModelButtonLabel()
{
    if (m_currentModel.isEmpty()) {
        m_modelBtn->setText(tr("Select Model"));
    } else {
        m_modelBtn->setText(m_currentModel);
    }
}

void OpenCodeComposer::updateContextLabel()
{
    QStringList parts;
    if (!m_currentModel.isEmpty()) {
        parts << m_currentModel;
    }
    if (m_tokenCount > 0) {
        if (m_tokenCount >= 1000)
            parts << tr("%1k tokens").arg(m_tokenCount / 1000.0, 0, 'f', 1);
        else
            parts << tr("%1 tokens").arg(m_tokenCount);
    }
    if (m_stepCount > 0) {
        parts << tr("Step %1").arg(m_stepCount);
    }
    m_contextModelLabel->setText(parts.join(QStringLiteral(" · ")));
}

// =============================================================================
// Slots
// =============================================================================

void OpenCodeComposer::onModelButtonClicked()
{
    if (m_modelPicker) {
        m_modelPicker->deleteLater();
    }
    m_modelPicker = new ModelPicker(window());

    // Rebuild model list.
    // Since ModelPicker takes (provider, models), we group by provider.
    QMap<QString, QStringList> byProvider;
    QMap<QString, QStringList> descriptionsByProvider;
    for (const auto& info : m_availableModels) {
        byProvider[info.provider].append(info.model);
        descriptionsByProvider[info.provider].append(info.description);
    }
    for (auto it = byProvider.begin(); it != byProvider.end(); ++it) {
        m_modelPicker->addProvider(it.key(), it.value(),
                                     descriptionsByProvider[it.key()]);
    }

    if (!m_currentProvider.isEmpty() && !m_currentModel.isEmpty()) {
        m_modelPicker->setCurrentModel(m_currentProvider, m_currentModel);
    }

    connect(m_modelPicker, &ModelPicker::modelSelected,
            this, &OpenCodeComposer::onModelSelected, Qt::UniqueConnection);

    m_modelPicker->exec();
}

void OpenCodeComposer::onModelSelected(const QString& provider, const QString& model)
{
    setModelInfo(provider, model);
    emit modelChanged(provider, model);
}

void OpenCodeComposer::onSubmitClicked()
{
    QString t = m_inputWidget->text().trimmed();
    if (!t.isEmpty() && !m_agentBusy) {
        emit messageSubmitted(t);
        clear();
    }
}

void OpenCodeComposer::onStopClicked()
{
    emit stopRequested();
}
