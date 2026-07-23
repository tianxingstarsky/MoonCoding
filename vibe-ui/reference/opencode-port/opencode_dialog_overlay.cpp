// =============================================================================
// opencode_dialog_overlay.cpp — Modal overlay implementation
// =============================================================================

#include "opencode_dialog_overlay.h"

#include <QApplication>
#include <QScreen>
#include <QKeyEvent>
#include <QGraphicsDropShadowEffect>
#include <QScrollBar>

// =============================================================================
// OpenCodeDialog — Base dialog with opencode dark styling
// =============================================================================

OpenCodeDialog::OpenCodeDialog(const QString& title, QWidget* parent)
    : QDialog(parent, Qt::FramelessWindowHint | Qt::Dialog)
{
    setObjectName(QStringLiteral("OpenCodeDialog"));
    setModal(true);
    setAttribute(Qt::WA_DeleteOnClose, false);

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    setupTitleBar(title);

    m_content = new QWidget(this);
    m_content->setObjectName(QStringLiteral("DialogContent"));
    m_contentLayout = new QVBoxLayout(m_content);
    m_contentLayout->setContentsMargins(24, 16, 24, 24);
    m_contentLayout->setSpacing(12);
    mainLayout->addWidget(m_content, 1);

    setMinimumWidth(240);
}

void OpenCodeDialog::setupTitleBar(const QString& title)
{
    auto* titleBar = new QWidget(this);
    titleBar->setObjectName(QStringLiteral("DialogTitleBar"));
    titleBar->setFixedHeight(40);

    auto* titleLayout = new QHBoxLayout(titleBar);
    titleLayout->setContentsMargins(16, 0, 8, 0);
    titleLayout->setSpacing(8);

    auto* titleLabel = new QLabel(title, titleBar);
    titleLabel->setObjectName(QStringLiteral("DialogTitle"));
    QFont titleFont = titleLabel->font();
    titleFont.setBold(true);
    titleFont.setPixelSize(14);
    titleLabel->setFont(titleFont);
    titleLayout->addWidget(titleLabel);

    titleLayout->addStretch();

    auto* closeBtn = new QPushButton(QStringLiteral("✕"), titleBar);
    closeBtn->setObjectName(QStringLiteral("DialogCloseButton"));
    closeBtn->setFixedSize(28, 28);
    closeBtn->setFlat(true);
    closeBtn->setCursor(Qt::PointingHandCursor);
    connect(closeBtn, &QPushButton::clicked, this, &QDialog::reject);
    titleLayout->addWidget(closeBtn);

    qobject_cast<QVBoxLayout*>(layout())->addWidget(titleBar);
}

// =============================================================================
// PermissionPrompt
// =============================================================================

PermissionPrompt::PermissionPrompt(QWidget* parent)
    : OpenCodeDialog(tr("Permission Required"), parent)
{
    setObjectName(QStringLiteral("PermissionPrompt"));
    setFixedWidth(420);

    m_toolNameLabel = new QLabel(this);
    m_toolNameLabel->setObjectName(QStringLiteral("PermToolName"));
    QFont toolFont = m_toolNameLabel->font();
    toolFont.setBold(true);
    toolFont.setPixelSize(16);
    m_toolNameLabel->setFont(toolFont);
    contentLayout()->addWidget(m_toolNameLabel);

    m_commandLabel = new QLabel(this);
    m_commandLabel->setObjectName(QStringLiteral("PermCommand"));
    m_commandLabel->setWordWrap(true);
    QFont cmdFont = m_commandLabel->font();
    cmdFont.setFamily(QStringLiteral("Consolas"));
    cmdFont.setPixelSize(13);
    m_commandLabel->setFont(cmdFont);
    contentLayout()->addWidget(m_commandLabel);

    m_descLabel = new QLabel(this);
    m_descLabel->setObjectName(QStringLiteral("PermDescription"));
    m_descLabel->setWordWrap(true);
    contentLayout()->addWidget(m_descLabel);

    contentLayout()->addStretch();

    m_alwaysCheck = new QCheckBox(tr("Always allow this type of operation"), this);
    m_alwaysCheck->setObjectName(QStringLiteral("PermAlwaysCheck"));
    contentLayout()->addWidget(m_alwaysCheck);

    auto* btnLayout = new QHBoxLayout();
    btnLayout->setSpacing(8);

    auto* denyBtn = new QPushButton(tr("Deny"), this);
    denyBtn->setObjectName(QStringLiteral("PermDenyButton"));
    denyBtn->setFixedHeight(36);
    denyBtn->setMinimumWidth(80);
    connect(denyBtn, &QPushButton::clicked, this, [this]() {
        emit denied();
        reject();
    });
    btnLayout->addWidget(denyBtn);

    btnLayout->addStretch();

    auto* approveBtn = new QPushButton(tr("Approve"), this);
    approveBtn->setObjectName(QStringLiteral("PermApproveButton"));
    approveBtn->setFixedHeight(36);
    approveBtn->setMinimumWidth(80);
    connect(approveBtn, &QPushButton::clicked, this, [this]() {
        emit approved();
        accept();
    });
    btnLayout->addWidget(approveBtn);

    contentLayout()->addLayout(btnLayout);
}

void PermissionPrompt::setToolName(const QString& name)
{
    m_toolNameLabel->setText(name);
}

void PermissionPrompt::setToolCommand(const QString& command)
{
    m_commandLabel->setText(command);
}

void PermissionPrompt::setToolDescription(const QString& desc)
{
    m_descLabel->setText(desc);
}

void PermissionPrompt::setAlwaysAllow(bool always)
{
    m_alwaysCheck->setChecked(always);
}

bool PermissionPrompt::alwaysAllow() const
{
    return m_alwaysCheck->isChecked();
}

// =============================================================================
// CommandPalette
// =============================================================================

CommandPalette::CommandPalette(QWidget* parent)
    : OpenCodeDialog(tr("Command Palette"), parent)
{
    setObjectName(QStringLiteral("CommandPalette"));
    setFixedSize(520, 360);
    setupUi();
}

void CommandPalette::setupUi()
{
    m_searchEdit = new QLineEdit(this);
    m_searchEdit->setObjectName(QStringLiteral("CommandPaletteSearch"));
    m_searchEdit->setPlaceholderText(tr("Type a command..."));
    m_searchEdit->setClearButtonEnabled(true);
    QFont searchFont = m_searchEdit->font();
    searchFont.setPixelSize(16);
    m_searchEdit->setFont(searchFont);
    contentLayout()->addWidget(m_searchEdit);

    m_commandList = new QListWidget(this);
    m_commandList->setObjectName(QStringLiteral("CommandPaletteList"));
    m_commandList->setFrameShape(QFrame::NoFrame);
    contentLayout()->addWidget(m_commandList, 1);

    connect(m_searchEdit, &QLineEdit::textChanged, this, &CommandPalette::onFilterChanged);
    connect(m_commandList, &QListWidget::itemActivated, this, &CommandPalette::onItemActivated);

    m_searchEdit->setFocus();
}

void CommandPalette::addCommand(const QString& name, const QString& description,
                                  const QString& shortcut)
{
    QString display = name;
    if (!shortcut.isEmpty())
        display += QStringLiteral("  (") + shortcut + QStringLiteral(")");
    if (!description.isEmpty())
        display += QStringLiteral(" — ") + description;

    m_allCommands.append(name);
    auto* item = new QListWidgetItem(display);
    item->setData(Qt::UserRole, name);
    m_commandList->addItem(item);
}

void CommandPalette::clearCommands()
{
    m_commandList->clear();
    m_allCommands.clear();
}

void CommandPalette::onFilterChanged(const QString& text)
{
    for (int i = 0; i < m_commandList->count(); ++i) {
        auto* item = m_commandList->item(i);
        bool visible = text.isEmpty()
            || item->text().contains(text, Qt::CaseInsensitive);
        item->setHidden(!visible);
    }
}

void CommandPalette::onItemActivated(QListWidgetItem* item)
{
    QString name = item->data(Qt::UserRole).toString();
    emit commandSelected(name);
    accept();
}

// =============================================================================
// ModelPicker
// =============================================================================

ModelPicker::ModelPicker(QWidget* parent)
    : OpenCodeDialog(tr("Select Model"), parent)
{
    setObjectName(QStringLiteral("ModelPicker"));
    setFixedSize(460, 380);

    m_searchEdit = new QLineEdit(this);
    m_searchEdit->setObjectName(QStringLiteral("ModelPickerSearch"));
    m_searchEdit->setPlaceholderText(tr("Search models..."));
    m_searchEdit->setClearButtonEnabled(true);
    QFont searchFont = m_searchEdit->font();
    searchFont.setPixelSize(15);
    m_searchEdit->setFont(searchFont);
    contentLayout()->addWidget(m_searchEdit);

    m_modelList = new QListWidget(this);
    m_modelList->setObjectName(QStringLiteral("ModelPickerList"));
    m_modelList->setFrameShape(QFrame::NoFrame);
    contentLayout()->addWidget(m_modelList, 1);

    connect(m_searchEdit, &QLineEdit::textChanged, this, &ModelPicker::onFilterChanged);
    connect(m_modelList, &QListWidget::itemActivated, this, &ModelPicker::onModelActivated);

    m_searchEdit->setFocus();
}

void ModelPicker::addProvider(const QString& providerName,
                                const QStringList& models,
                                const QStringList& descriptions)
{
    // Add a provider section header.
    auto* headerItem = new QListWidgetItem(
        QStringLiteral("── ") + providerName + QStringLiteral(" ──"));
    headerItem->setFlags(headerItem->flags() & ~Qt::ItemIsSelectable);
    QFont headerFont = headerItem->font();
    headerFont.setBold(true);
    headerFont.setPixelSize(12);
    headerItem->setFont(headerFont);
    headerItem->setForeground(QColor(QStringLiteral("#8b949e")));
    m_modelList->addItem(headerItem);

    for (int i = 0; i < models.size(); ++i) {
        QString label = models[i];
        QString desc = i < descriptions.size() ? descriptions[i] : QString();

        auto* item = new QListWidgetItem(label);
        if (!desc.isEmpty())
            item->setToolTip(desc);
        item->setData(Qt::UserRole, providerName);
        item->setData(Qt::UserRole + 1, models[i]);
        m_modelList->addItem(item);

        ModelEntry entry;
        entry.provider = providerName;
        entry.model = models[i];
        entry.description = desc;
        m_entries.append(entry);
    }
}

void ModelPicker::setCurrentModel(const QString& provider, const QString& model)
{
    for (int i = 0; i < m_modelList->count(); ++i) {
        auto* item = m_modelList->item(i);
        if (item->data(Qt::UserRole).toString() == provider
            && item->data(Qt::UserRole + 1).toString() == model) {
            m_modelList->setCurrentItem(item);
            break;
        }
    }
}

void ModelPicker::onFilterChanged(const QString& text)
{
    for (int i = 0; i < m_modelList->count(); ++i) {
        auto* item = m_modelList->item(i);
        if (item->flags() & Qt::ItemIsSelectable) {
            bool visible = text.isEmpty()
                || item->text().contains(text, Qt::CaseInsensitive);
            item->setHidden(!visible);
        }
    }
}

void ModelPicker::onModelActivated(QListWidgetItem* item)
{
    if (!(item->flags() & Qt::ItemIsSelectable)) return;
    QString provider = item->data(Qt::UserRole).toString();
    QString model = item->data(Qt::UserRole + 1).toString();
    emit modelSelected(provider, model);
    accept();
}

// =============================================================================
// ProviderConnectDialog
// =============================================================================

ProviderConnectDialog::ProviderConnectDialog(QWidget* parent)
    : OpenCodeDialog(tr("Connect Provider"), parent)
{
    setObjectName(QStringLiteral("ProviderConnectDialog"));
    setFixedWidth(400);

    m_providerLabel = new QLabel(this);
    m_providerLabel->setObjectName(QStringLiteral("ProvConnectName"));
    QFont provFont = m_providerLabel->font();
    provFont.setBold(true);
    provFont.setPixelSize(16);
    m_providerLabel->setFont(provFont);
    contentLayout()->addWidget(m_providerLabel);

    // API key.
    auto* keyLabel = new QLabel(tr("API Key:"), this);
    contentLayout()->addWidget(keyLabel);

    m_apiKeyEdit = new QLineEdit(this);
    m_apiKeyEdit->setObjectName(QStringLiteral("ProvApiKey"));
    m_apiKeyEdit->setEchoMode(QLineEdit::Password);
    m_apiKeyEdit->setPlaceholderText(tr("sk-..."));
    contentLayout()->addWidget(m_apiKeyEdit);

    // Base URL.
    auto* urlLabel = new QLabel(tr("Base URL:"), this);
    contentLayout()->addWidget(urlLabel);

    m_baseUrlEdit = new QLineEdit(this);
    m_baseUrlEdit->setObjectName(QStringLiteral("ProvBaseUrl"));
    m_baseUrlEdit->setPlaceholderText(tr("https://api.openai.com/v1"));
    contentLayout()->addWidget(m_baseUrlEdit);

    // API type.
    auto* typeLabel = new QLabel(tr("API Type:"), this);
    contentLayout()->addWidget(typeLabel);

    m_apiTypeCombo = new QComboBox(this);
    m_apiTypeCombo->setObjectName(QStringLiteral("ProvApiType"));
    m_apiTypeCombo->addItem(tr("OpenAI Compatible"));
    m_apiTypeCombo->addItem(tr("Anthropic"));
    m_apiTypeCombo->addItem(tr("Google AI"));
    contentLayout()->addWidget(m_apiTypeCombo);

    contentLayout()->addStretch();

    // Buttons.
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();

    auto* cancelBtn = new QPushButton(tr("Cancel"), this);
    cancelBtn->setObjectName(QStringLiteral("ProvCancelBtn"));
    cancelBtn->setFixedHeight(36);
    cancelBtn->setMinimumWidth(80);
    connect(cancelBtn, &QPushButton::clicked, this, &QDialog::reject);
    btnLayout->addWidget(cancelBtn);

    auto* connectBtn = new QPushButton(tr("Connect"), this);
    connectBtn->setObjectName(QStringLiteral("ProvConnectBtn"));
    connectBtn->setFixedHeight(36);
    connectBtn->setMinimumWidth(80);
    connect(connectBtn, &QPushButton::clicked, this, [this]() {
        emit connectRequested(
            m_providerLabel->text(),
            m_apiKeyEdit->text().trimmed(),
            m_baseUrlEdit->text().trimmed());
        accept();
    });
    btnLayout->addWidget(connectBtn);

    contentLayout()->addLayout(btnLayout);
}

void ProviderConnectDialog::setProviderName(const QString& name)
{
    m_providerLabel->setText(name);
}

QString ProviderConnectDialog::apiKey() const
{
    return m_apiKeyEdit->text().trimmed();
}

QString ProviderConnectDialog::baseUrl() const
{
    return m_baseUrlEdit->text().trimmed();
}

QString ProviderConnectDialog::apiType() const
{
    return m_apiTypeCombo->currentText();
}
