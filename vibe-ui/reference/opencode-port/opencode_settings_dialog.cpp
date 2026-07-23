// =============================================================================
// opencode_settings_dialog.cpp — Settings dialog implementation
// =============================================================================

#include "opencode_settings_dialog.h"

#include <QFormLayout>
#include <QGroupBox>
#include <QHeaderView>
#include <QScrollBar>

OpenCodeSettingsDialog::OpenCodeSettingsDialog(QWidget* parent)
    : QDialog(parent)
{
    setObjectName(QStringLiteral("SettingsDialog"));
    setWindowTitle(tr("Settings"));
    setMinimumSize(620, 460);
    resize(680, 520);
    setModal(true);

    setupUi();
}

void OpenCodeSettingsDialog::setupUi()
{
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // Title bar.
    auto* titleBar = new QWidget(this);
    titleBar->setObjectName(QStringLiteral("DialogTitleBar"));
    titleBar->setFixedHeight(44);

    auto* titleLayout = new QHBoxLayout(titleBar);
    titleLayout->setContentsMargins(20, 0, 8, 0);

    auto* titleLabel = new QLabel(tr("Settings"), titleBar);
    titleLabel->setObjectName(QStringLiteral("DialogTitle"));
    QFont titleFont = titleLabel->font();
    titleFont.setBold(true);
    titleFont.setPixelSize(16);
    titleLabel->setFont(titleFont);
    titleLayout->addWidget(titleLabel);
    titleLayout->addStretch();

    auto* closeBtn = new QPushButton(QStringLiteral("✕"), titleBar);
    closeBtn->setObjectName(QStringLiteral("DialogCloseButton"));
    closeBtn->setFixedSize(28, 28);
    closeBtn->setFlat(true);
    closeBtn->setCursor(Qt::PointingHandCursor);
    connect(closeBtn, &QPushButton::clicked, this, &OpenCodeSettingsDialog::onCancel);
    titleLayout->addWidget(closeBtn);

    mainLayout->addWidget(titleBar);

    // Tab widget.
    m_tabWidget = new QTabWidget(this);
    m_tabWidget->setObjectName(QStringLiteral("SettingsTabWidget"));
    m_tabWidget->setTabPosition(QTabWidget::North);
    m_tabWidget->setDocumentMode(true);

    m_tabWidget->addTab(createGeneralTab(), tr("General"));
    m_tabWidget->addTab(createShortcutsTab(), tr("Shortcuts"));
    m_tabWidget->addTab(createProvidersTab(), tr("Providers"));
    m_tabWidget->addTab(createModelsTab(), tr("Models"));

    mainLayout->addWidget(m_tabWidget, 1);

    // Bottom buttons.
    auto* btnBar = new QWidget(this);
    btnBar->setObjectName(QStringLiteral("SettingsBtnBar"));
    btnBar->setFixedHeight(52);

    auto* btnLayout = new QHBoxLayout(btnBar);
    btnLayout->setContentsMargins(20, 8, 20, 8);
    btnLayout->addStretch();

    m_cancelBtn = new QPushButton(tr("Cancel"), btnBar);
    m_cancelBtn->setObjectName(QStringLiteral("SettingsCancelBtn"));
    m_cancelBtn->setFixedHeight(36);
    m_cancelBtn->setMinimumWidth(80);
    connect(m_cancelBtn, &QPushButton::clicked, this, &OpenCodeSettingsDialog::onCancel);
    btnLayout->addWidget(m_cancelBtn);

    m_applyBtn = new QPushButton(tr("Apply"), btnBar);
    m_applyBtn->setObjectName(QStringLiteral("SettingsApplyBtn"));
    m_applyBtn->setFixedHeight(36);
    m_applyBtn->setMinimumWidth(80);
    connect(m_applyBtn, &QPushButton::clicked, this, &OpenCodeSettingsDialog::onApply);
    btnLayout->addWidget(m_applyBtn);

    mainLayout->addWidget(btnBar);
}

QWidget* OpenCodeSettingsDialog::createGeneralTab()
{
    auto* page = new QWidget(this);
    page->setObjectName(QStringLiteral("SettingsGeneralTab"));

    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(16);

    // Layout mode.
    auto* layoutGroup = new QGroupBox(tr("Layout"), page);
    layoutGroup->setObjectName(QStringLiteral("SettingsGroup"));
    auto* layoutForm = new QFormLayout(layoutGroup);
    layoutForm->setSpacing(8);

    m_layoutCombo = new QComboBox(layoutGroup);
    m_layoutCombo->setObjectName(QStringLiteral("LayoutCombo"));
    m_layoutCombo->addItem(tr("v2 (Modern)"), QStringLiteral("v2"));
    m_layoutCombo->addItem(tr("Legacy"), QStringLiteral("legacy"));
    layoutForm->addRow(tr("Layout mode:"), m_layoutCombo);

    layout->addWidget(layoutGroup);

    // Theme.
    auto* themeGroup = new QGroupBox(tr("Appearance"), page);
    themeGroup->setObjectName(QStringLiteral("SettingsGroup"));
    auto* themeForm = new QFormLayout(themeGroup);
    themeForm->setSpacing(8);

    m_themeCombo = new QComboBox(themeGroup);
    m_themeCombo->setObjectName(QStringLiteral("ThemeCombo"));
    m_themeCombo->addItem(tr("Dark (Default)"), QStringLiteral("dark"));
    m_themeCombo->addItem(tr("Darker"), QStringLiteral("darker"));
    m_themeCombo->addItem(tr("High Contrast"), QStringLiteral("high-contrast"));
    themeForm->addRow(tr("Theme:"), m_themeCombo);

    layout->addWidget(themeGroup);

    // Language.
    auto* langGroup = new QGroupBox(tr("Language"), page);
    langGroup->setObjectName(QStringLiteral("SettingsGroup"));
    auto* langForm = new QFormLayout(langGroup);
    langForm->setSpacing(8);

    m_languageCombo = new QComboBox(langGroup);
    m_languageCombo->setObjectName(QStringLiteral("LanguageCombo"));
    m_languageCombo->addItem(tr("English"), QStringLiteral("en"));
    m_languageCombo->addItem(QString::fromUtf8("中文 (简体)"), QStringLiteral("zh-CN"));
    m_languageCombo->addItem(QString::fromUtf8("日本語"), QStringLiteral("ja"));
    langForm->addRow(tr("Language:"), m_languageCombo);

    layout->addWidget(langGroup);
    layout->addStretch();

    return page;
}

QWidget* OpenCodeSettingsDialog::createShortcutsTab()
{
    auto* page = new QWidget(this);
    page->setObjectName(QStringLiteral("SettingsShortcutsTab"));

    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(24, 20, 24, 20);

    auto* infoLabel = new QLabel(
        tr("Configure keyboard shortcuts. Double-click a shortcut to edit it."), page);
    infoLabel->setObjectName(QStringLiteral("SettingsInfoLabel"));
    infoLabel->setWordWrap(true);
    layout->addWidget(infoLabel);

    m_shortcutsTable = new QTableWidget(page);
    m_shortcutsTable->setObjectName(QStringLiteral("ShortcutsTable"));
    m_shortcutsTable->setColumnCount(2);
    m_shortcutsTable->setHorizontalHeaderLabels({tr("Action"), tr("Shortcut")});
    m_shortcutsTable->horizontalHeader()->setStretchLastSection(true);
    m_shortcutsTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_shortcutsTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_shortcutsTable->verticalHeader()->setVisible(false);
    m_shortcutsTable->setAlternatingRowColors(true);

    // Default shortcuts — matching opencode desktop v2.
    struct ShortcutEntry {
        QString action;
        QString shortcut;
    };
    QList<ShortcutEntry> defaults = {
        {tr("New Tab"),           QStringLiteral("Ctrl+N")},
        {tr("Open Tab"),          QStringLiteral("Ctrl+T")},
        {tr("Close Tab"),         QStringLiteral("Ctrl+W")},
        {tr("Switch Tab 1-9"),    QStringLiteral("Ctrl+1..9")},
        {tr("Command Palette"),   QStringLiteral("Ctrl+K")},
        {tr("Toggle Home Tab"),   QStringLiteral("Ctrl+H")},
        {tr("Settings"),          QStringLiteral("Ctrl+,")},
        {tr("Submit Message"),    QStringLiteral("Enter")},
        {tr("New Line"),          QStringLiteral("Shift+Enter")},
        {tr("Stop Generation"),   QStringLiteral("Esc")},
        {tr("Toggle File Tree"),  QStringLiteral("Ctrl+E")},
        {tr("Toggle Review"),     QStringLiteral("Ctrl+R")},
    };

    m_shortcutsTable->setRowCount(defaults.size());
    for (int i = 0; i < defaults.size(); ++i) {
        auto* actionItem = new QTableWidgetItem(defaults[i].action);
        actionItem->setFlags(actionItem->flags() & ~Qt::ItemIsEditable);
        m_shortcutsTable->setItem(i, 0, actionItem);

        auto* shortcutItem = new QTableWidgetItem(defaults[i].shortcut);
        shortcutItem->setFlags(shortcutItem->flags() & ~Qt::ItemIsEditable);
        shortcutItem->setTextAlignment(Qt::AlignCenter);
        m_shortcutsTable->setItem(i, 1, shortcutItem);
    }

    m_shortcutsTable->resizeColumnsToContents();
    layout->addWidget(m_shortcutsTable, 1);

    return page;
}

QWidget* OpenCodeSettingsDialog::createProvidersTab()
{
    auto* page = new QWidget(this);
    page->setObjectName(QStringLiteral("SettingsProvidersTab"));

    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(12);

    auto* infoLabel = new QLabel(
        tr("Manage your API providers. Add, configure, or remove provider connections."), page);
    infoLabel->setObjectName(QStringLiteral("SettingsInfoLabel"));
    infoLabel->setWordWrap(true);
    layout->addWidget(infoLabel);

    m_providerList = new QListWidget(page);
    m_providerList->setObjectName(QStringLiteral("ProviderList"));
    m_providerList->setMinimumHeight(160);
    connect(m_providerList, &QListWidget::itemDoubleClicked,
            this, &OpenCodeSettingsDialog::onProviderDoubleClicked);
    layout->addWidget(m_providerList, 1);

    auto* btnRow = new QHBoxLayout();
    btnRow->setSpacing(8);

    m_addProviderBtn = new QPushButton(tr("Add Provider"), page);
    m_addProviderBtn->setObjectName(QStringLiteral("AddProviderBtn"));
    m_addProviderBtn->setFixedHeight(36);
    m_addProviderBtn->setMinimumWidth(120);
    btnRow->addWidget(m_addProviderBtn);

    m_removeProviderBtn = new QPushButton(tr("Remove"), page);
    m_removeProviderBtn->setObjectName(QStringLiteral("RemoveProviderBtn"));
    m_removeProviderBtn->setFixedHeight(36);
    m_removeProviderBtn->setMinimumWidth(80);
    btnRow->addWidget(m_removeProviderBtn);

    btnRow->addStretch();
    layout->addLayout(btnRow);

    return page;
}

QWidget* OpenCodeSettingsDialog::createModelsTab()
{
    auto* page = new QWidget(this);
    page->setObjectName(QStringLiteral("SettingsModelsTab"));

    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(12);

    m_modelSearch = new QLineEdit(page);
    m_modelSearch->setObjectName(QStringLiteral("ModelSearchEdit"));
    m_modelSearch->setPlaceholderText(tr("Search models..."));
    m_modelSearch->setClearButtonEnabled(true);
    layout->addWidget(m_modelSearch);

    m_modelList = new QListWidget(page);
    m_modelList->setObjectName(QStringLiteral("ModelList"));
    m_modelList->setFrameShape(QFrame::NoFrame);
    layout->addWidget(m_modelList, 1);

    return page;
}

// =============================================================================
// Public API
// =============================================================================

QString OpenCodeSettingsDialog::layoutMode() const
{
    return m_layoutCombo->currentData().toString();
}

QString OpenCodeSettingsDialog::theme() const
{
    return m_themeCombo->currentData().toString();
}

QString OpenCodeSettingsDialog::language() const
{
    return m_languageCombo->currentData().toString();
}

void OpenCodeSettingsDialog::setLayoutMode(const QString& mode)
{
    int idx = m_layoutCombo->findData(mode);
    if (idx >= 0) m_layoutCombo->setCurrentIndex(idx);
}

void OpenCodeSettingsDialog::setTheme(const QString& theme)
{
    int idx = m_themeCombo->findData(theme);
    if (idx >= 0) m_themeCombo->setCurrentIndex(idx);
}

void OpenCodeSettingsDialog::setLanguage(const QString& lang)
{
    int idx = m_languageCombo->findData(lang);
    if (idx >= 0) m_languageCombo->setCurrentIndex(idx);
}

void OpenCodeSettingsDialog::addProvider(const QString& name, const QString& baseUrl,
                                           bool connected)
{
    QString status = connected ? tr("Connected") : tr("Disconnected");
    QString label = QStringLiteral("%1  [%2] — %3").arg(name, status, baseUrl);
    auto* item = new QListWidgetItem(label);
    item->setData(Qt::UserRole, name);
    m_providerList->addItem(item);
}

void OpenCodeSettingsDialog::clearProviders()
{
    m_providerList->clear();
}

void OpenCodeSettingsDialog::addModel(const QString& provider, const QString& model)
{
    auto* item = new QListWidgetItem(
        QStringLiteral("[%1] %2").arg(provider, model));
    item->setData(Qt::UserRole, provider);
    item->setData(Qt::UserRole + 1, model);
    m_modelList->addItem(item);
}

void OpenCodeSettingsDialog::clearModels()
{
    m_modelList->clear();
}

// =============================================================================
// Slots
// =============================================================================

void OpenCodeSettingsDialog::onApply()
{
    emit layoutModeChanged(layoutMode());
    emit themeChanged(theme());
    emit languageChanged(language());
    emit settingsApplied();
    accept();
}

void OpenCodeSettingsDialog::onCancel()
{
    reject();
}

void OpenCodeSettingsDialog::onProviderDoubleClicked(QListWidgetItem* item)
{
    QString name = item->data(Qt::UserRole).toString();
    emit providerConnectRequested(name);
}
