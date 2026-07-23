#include "mainwindow.h"

#include "apps/apprunner.h"
#include "apps/appswidget.h"
#include "chat/chatwidget.h"
#include "input/boardime.h"
#include "input/inputwidget.h"
#include "input/softkeyboard.h"
#include "input/touchscroll.h"
#include "languagemanager.h"
#include "opencode_antialias.h"
#include "rustbridge.h"
#include "settings/modelfetcher.h"
#include "settings/wifipanel.h"
#include "settings/boardnetrecover.h"
#include "tree/treewidget.h"

#include <QApplication>
#include <QCloseEvent>
#include <QComboBox>
#include <QCoreApplication>
#include <QEvent>
#include <QEventLoop>
#include <QMouseEvent>
#include <QCryptographicHash>
#include <QDateTime>
#include <QDir>
#include <QDoubleSpinBox>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QFontComboBox>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMessageBox>
#include <QPushButton>
#include <QProcess>
#include <QResizeEvent>
#include <QScrollArea>
#include <QSettings>
#include <QSpinBox>
#include <QSplitter>
#include <QStackedWidget>
#include <QStandardItemModel>
#include <QStandardPaths>
#include <QStatusBar>
#include <QStyle>
#include <QToolButton>
#include <QTimer>
#include <QUuid>
#include <QVBoxLayout>

static int configuredUiFontSize()
{
    int def = 13;
    if (qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb")) {
        def = 16; // DSI board: default larger for readability
    }
    return qBound(9, QSettings().value(QStringLiteral("appearance/fontSize"), def).toInt(), 28);
}

static void applyAppFont()
{
    opencode::installFontSubstitutions();

    QSettings settings;
#ifdef Q_OS_WIN
    const QString defaultFamily = QStringLiteral("Microsoft YaHei UI");
#else
    const QString defaultFamily = QStringLiteral("Noto Sans CJK SC");
#endif
    const QString family = settings.value(
        QStringLiteral("appearance/fontFamily"),
        defaultFamily).toString();
    const int size = configuredUiFontSize();
    QFont font;
    QStringList families = opencode::uiFontFamilies();
    if (!family.isEmpty() && families.value(0) != family) {
        families.prepend(family);
    }
    font.setFamilies(families);
    font.setPointSize(size);
    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    if (board) {
        // Static face + no AA: biggest single-thread font win on A7/linuxfb.
        font.setWeight(QFont::Normal);
        font.setStyleStrategy(QFont::NoAntialias);
        font.setHintingPreference(QFont::PreferNoHinting);
    } else {
        font.setWeight(QFont::DemiBold);
        font.setStyleStrategy(QFont::PreferAntialias);
        font.setHintingPreference(QFont::PreferFullHinting);
    }
    font.setStyleHint(QFont::SansSerif);
    qApp->setFont(font);
}

namespace {
QString workspaceKey(const QString &workspace)
{
    QString normalized = QDir::fromNativeSeparators(QDir::cleanPath(workspace));
#ifdef Q_OS_WIN
    normalized = normalized.toCaseFolded();
#endif
    return QString::fromLatin1(
        QCryptographicHash::hash(normalized.toUtf8(), QCryptographicHash::Sha256).toHex().left(16));
}

constexpr int kWideLayoutThreshold = 1024;
constexpr int kTreeSidePanelWidth = 360;
}

// ═══════════════════════════════════════════════════════════════
//  Constructor
// ═══════════════════════════════════════════════════════════════

MainWindow::MainWindow(const QString &workspace, QWidget *parent)
    : QMainWindow(parent)
    , m_workspace(workspace)
    , m_bridge(new RustBridge(this))
    , m_chat(new ChatWidget(this))
    , m_tree(new TreeWidget(this))
    , m_input(new InputWidget(this))
    , m_pages(new QStackedWidget(this))
    , m_chatPage(new QWidget(this))
    , m_treePage(new QWidget(this))
    , m_treeSideHost(new QWidget(this))
    , m_chatSplitter(new QSplitter(Qt::Horizontal, m_chatPage))
    , m_historyPanel(new QWidget(this))
    , m_historyList(new QListWidget(this))
    , m_historySearch(new QLineEdit(this))
    , m_projectButton(new QToolButton(this))
    , m_chatNav(new QPushButton(tr("对话"), this))
    , m_treeNav(new QPushButton(tr("项目树"), this))
    , m_appsNav(new QPushButton(tr("预览"), this))
    , m_historyNav(new QPushButton(tr("历史"), this))
    , m_activeNodeLabel(new QLabel(tr("暂无活跃节点"), this))
    , m_connectionLabel(new QLabel(tr("后端加载中"), this))
    , m_tokenLabel(new QLabel(tr("0 tokens"), this))
    , m_networkLabel(new QLabel(tr("网络…"), this))
    , m_flashBanner(new QLabel(this))
    , m_flashTimer(new QTimer(this))
    , m_networkTimer(new QTimer(this))
    , m_mainHeader(new QWidget(this))
    , m_subHeader(new QWidget(this))
    , m_subHeaderTitle(new QLabel(this))
{
    setWindowTitle(tr("MoonCoding"));
    setMinimumSize(360, 640);
    resize(1280, 720);

    auto *central = new QWidget(this);
    auto *rootLayout = new QVBoxLayout(central);
    rootLayout->setContentsMargins(0, 0, 0, 0);
    rootLayout->setSpacing(0);

    // ── Main header: top row (brand/project/utils) + nav row (touch-friendly)
    // Two rows fit 720px portrait boards without horizontal overflow.
    m_mainHeader->setObjectName(QStringLiteral("appHeader"));
    auto *headerRoot = new QVBoxLayout(m_mainHeader);
    headerRoot->setContentsMargins(8, 6, 8, 6);
    headerRoot->setSpacing(6);

    auto *topRow = new QHBoxLayout;
    topRow->setSpacing(6);
    auto *brand = new QLabel(QStringLiteral("MoonCoding"), m_mainHeader);
    brand->setObjectName(QStringLiteral("brand"));
    m_projectButton->setObjectName(QStringLiteral("projectButton"));
    if (m_workspace.isEmpty()) {
        m_projectButton->setText(tr("无项目"));
        m_projectButton->setToolTip(tr("还没有项目、请创建新项目"));
    } else {
        m_projectButton->setText(QFileInfo(m_workspace).fileName());
        m_projectButton->setToolTip(m_workspace);
    }
    m_chatNav->setObjectName(QStringLiteral("navButton"));
    m_chatNav->setCheckable(true);
    m_chatNav->setChecked(true);
    m_treeNav->setObjectName(QStringLiteral("navButton"));
    m_treeNav->setCheckable(true);
    m_historyNav->setObjectName(QStringLiteral("navButton"));
    m_historyNav->setCheckable(true);
    m_appsNav->setObjectName(QStringLiteral("navButton"));
    m_appsNav->setCheckable(true);
    auto *themeButton = new QToolButton(m_mainHeader);
    themeButton->setObjectName(QStringLiteral("navButton"));
    themeButton->setText(tr("主题"));
    themeButton->setToolTip(tr("切换浅色/深色主题"));
    auto *settingsButton = new QToolButton(m_mainHeader);
    settingsButton->setObjectName(QStringLiteral("navButton"));
    settingsButton->setText(tr("设置"));
    m_networkLabel->setObjectName(QStringLiteral("networkStatus"));
    m_networkLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    m_networkLabel->setMinimumWidth(72);
    m_networkLabel->setToolTip(tr("点一下：WiFi 设置 · 长按：一键恢复网络"));
    topRow->addWidget(brand);
    topRow->addWidget(m_projectButton, 1);
    topRow->addWidget(m_networkLabel);
    topRow->addWidget(themeButton);
    topRow->addWidget(settingsButton);

    auto *navRow = new QHBoxLayout;
    navRow->setSpacing(6);
    navRow->addWidget(m_chatNav, 1);
    navRow->addWidget(m_treeNav, 1);
    navRow->addWidget(m_appsNav, 1);
    navRow->addWidget(m_historyNav, 1);

    headerRoot->addLayout(topRow);
    headerRoot->addLayout(navRow);

    // ── Sub header (sub-pages: back + title) ──
    m_subHeader->setObjectName(QStringLiteral("appHeader"));
    auto *subLayout = new QHBoxLayout(m_subHeader);
    subLayout->setContentsMargins(8, 4, 14, 4);
    auto *backBtn = new QPushButton(tr("← 返回"), m_subHeader);
    backBtn->setObjectName(QStringLiteral("backButton"));
    backBtn->setMaximumWidth(100);
    m_subHeaderTitle->setObjectName(QStringLiteral("brand"));
    subLayout->addWidget(backBtn);
    subLayout->addWidget(m_subHeaderTitle, 1);
    m_subHeader->hide();
    connect(backBtn, &QPushButton::clicked, this, [this] {
        if (m_currentPage == WifiPage || m_currentPage == ModelsPage) {
            showSettings();
        } else {
            showChatPage();
        }
    });
    connect(m_chatNav, &QPushButton::clicked, this, &MainWindow::showChatPage);
    connect(m_treeNav, &QPushButton::clicked, this, &MainWindow::showTreePage);
    connect(m_appsNav, &QPushButton::clicked, this, &MainWindow::showAppsPage);
    connect(m_historyNav, &QPushButton::clicked, this, &MainWindow::toggleHistoryPanel);
    connect(m_projectButton, &QToolButton::clicked, this, &MainWindow::showProjectMenu);
    connect(themeButton, &QToolButton::clicked, this, &MainWindow::toggleTheme);
    connect(settingsButton, &QToolButton::clicked, this, &MainWindow::showSettings);
    connect(m_input, &InputWidget::settingsRequested, this, &MainWindow::showSettings);
    m_networkLabel->installEventFilter(this);
    m_networkLabel->setCursor(Qt::PointingHandCursor);

    // Pre-populate context bar with current model
    {
        QSettings settings;
        const QString model = settings.value(
            QStringLiteral("provider/model"), QString()).toString();
        if (!model.isEmpty())
            m_input->setContextModel(model);
    }

    // ── Flash banner ──
    m_flashBanner->setObjectName(QStringLiteral("flashBanner"));
    m_flashBanner->setAlignment(Qt::AlignCenter);
    m_flashBanner->hide();
    m_flashTimer->setSingleShot(true);
    connect(m_flashTimer, &QTimer::timeout, m_flashBanner, &QWidget::hide);

    // ── Chat page ──
    m_chatPage->setObjectName(QStringLiteral("chatPage"));
    m_chatPage->setAutoFillBackground(true);
    auto *conversation = new QWidget(m_chatSplitter);
    conversation->setObjectName(QStringLiteral("chatConversation"));
    conversation->setAutoFillBackground(true);
    auto *conversationLayout = new QVBoxLayout(conversation);
    conversationLayout->setContentsMargins(0, 0, 0, 0);
    conversationLayout->setSpacing(0);
    m_activeNodeLabel->setObjectName(QStringLiteral("activeNodeBanner"));
    m_activeNodeLabel->setWordWrap(true);
    conversationLayout->addWidget(m_activeNodeLabel);
    conversationLayout->addWidget(m_chat, 1);
    conversationLayout->addWidget(m_input);

    m_treePanelVisible = QSettings().value(
        QStringLiteral("appearance/treePanelVisible"), false).toBool();

    m_treeSideHost->setObjectName(QStringLiteral("treeSideHost"));
    auto *treeSideLayout = new QVBoxLayout(m_treeSideHost);
    treeSideLayout->setContentsMargins(0, 0, 0, 0);
    treeSideLayout->setSpacing(0);

    auto *treeSideHeader = new QWidget(m_treeSideHost);
    treeSideHeader->setObjectName(QStringLiteral("sidePanelHeader"));
    auto *treeSideHeaderLayout = new QHBoxLayout(treeSideHeader);
    treeSideHeaderLayout->setContentsMargins(12, 6, 6, 6);
    auto *treeSideTitle = new QLabel(tr("项目树"), treeSideHeader);
    treeSideTitle->setObjectName(QStringLiteral("sidePanelTitle"));
    auto *closeTreePanel = new QToolButton(treeSideHeader);
    closeTreePanel->setObjectName(QStringLiteral("sidePanelClose"));
    closeTreePanel->setText(QStringLiteral("×"));
    closeTreePanel->setToolTip(tr("收起项目树"));
    treeSideHeaderLayout->addWidget(treeSideTitle);
    treeSideHeaderLayout->addStretch(1);
    treeSideHeaderLayout->addWidget(closeTreePanel);
    treeSideLayout->addWidget(treeSideHeader);
    treeSideLayout->addWidget(m_tree);

    connect(closeTreePanel, &QToolButton::clicked, this, [this] {
        m_treePanelVisible = false;
        QSettings().setValue(QStringLiteral("appearance/treePanelVisible"), false);
        applyResponsiveLayout();
        updateNavStates();
    });

    m_chatSplitter->addWidget(conversation);
    m_chatSplitter->addWidget(m_treeSideHost);
    m_chatSplitter->setStretchFactor(0, 1);
    m_chatSplitter->setStretchFactor(1, 0);
    m_chatSplitter->setCollapsible(0, false);
    m_chatSplitter->setCollapsible(1, true);

    m_historyPanel->setObjectName(QStringLiteral("historyPanel"));
    m_historyPanel->setFixedWidth(300);
    auto *historyLayout = new QVBoxLayout(m_historyPanel);
    historyLayout->setContentsMargins(10, 10, 10, 10);
    auto *historyTitle = new QLabel(tr("对话历史"), m_historyPanel);
    historyTitle->setObjectName(QStringLiteral("panelTitle"));
    auto *newChat = new QPushButton(tr("新对话"), m_historyPanel);
    newChat->setObjectName(QStringLiteral("primaryButton"));
    m_historySearch->setPlaceholderText(tr("搜索历史"));
    m_historyList->setObjectName(QStringLiteral("historyList"));
    historyLayout->addWidget(historyTitle);
    historyLayout->addWidget(newChat);
    historyLayout->addWidget(m_historySearch);
    historyLayout->addWidget(m_historyList, 1);
    m_historyPanel->hide();
    connect(newChat, &QPushButton::clicked, this, &MainWindow::newConversation);
    connect(m_historySearch, &QLineEdit::textChanged, this, [this](const QString &query) {
        for (int row = 0; row < m_historyList->count(); ++row) {
            QListWidgetItem *item = m_historyList->item(row);
            item->setHidden(!item->text().contains(query, Qt::CaseInsensitive));
        }
    });
    connect(m_historyList, &QListWidget::itemActivated, this, &MainWindow::onHistoryItemActivated);

    auto *chatPageLayout = new QHBoxLayout(m_chatPage);
    chatPageLayout->setContentsMargins(0, 0, 0, 0);
    chatPageLayout->setSpacing(0);
    chatPageLayout->addWidget(m_historyPanel);
    chatPageLayout->addWidget(m_chatSplitter, 1);

    // ── Tree page ──
    auto *treePageLayout = new QVBoxLayout(m_treePage);
    treePageLayout->setContentsMargins(0, 0, 0, 0);
    auto *treePageHint = new QLabel(
        tr("宽屏下项目树显示在对话右侧，小屏设备请通过此页面查看项目树。"),
        m_treePage);
    treePageHint->setObjectName(QStringLiteral("panelHint"));
    treePageHint->setWordWrap(true);
    treePageLayout->addWidget(treePageHint);

    // ── Populate stack ──
    m_pages->addWidget(m_chatPage);
    m_pages->addWidget(m_treePage);

    // ── Apps / project preview page ──
    m_apps = new AppsWidget(m_bridge, this);
    m_apps->setWorkspace(m_workspace);
    m_pages->addWidget(m_apps);
    m_appRunner = new AppRunner(this);
    connect(m_appRunner, &AppRunner::runRequested, this, &MainWindow::submitMessage);

    m_pages->addWidget(buildNewProjectPage());
    m_pages->addWidget(buildOpenProjectPage());
    m_pages->addWidget(buildSettingsPage());
    m_pages->addWidget(buildWifiPage());
    m_pages->addWidget(buildModelsPage());

    m_ime = new BoardImeController(central, this);
    m_modelFetcher = new ModelFetcher(this);

    rootLayout->addWidget(m_mainHeader);
    rootLayout->addWidget(m_subHeader);
    rootLayout->addWidget(m_flashBanner);
    rootLayout->addWidget(m_pages, 1);
    rootLayout->addWidget(m_ime->keyboard());
    setCentralWidget(central);
    statusBar()->addWidget(m_connectionLabel);
    statusBar()->addPermanentWidget(m_tokenLabel);

    connect(m_input, &InputWidget::softKeyboardToggleRequested, this, [this] {
        if (m_ime->isVisible()) {
            m_ime->setVisible(false);
        } else {
            m_input->focusEditor();
            m_ime->setVisible(true);
        }
    });
    connect(m_ime, &BoardImeController::visibilityChanged, m_input,
            &InputWidget::setKeyboardButtonChecked);

    m_networkTimer->setInterval(15000);
    connect(m_networkTimer, &QTimer::timeout, this, &MainWindow::refreshNetworkStatus);
    m_networkTimer->start();
    refreshNetworkStatus();

    if (qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb")) {
        enableBoardTouchScroll();
    }

    QSettings settings;
    m_lightTheme = settings.value(QStringLiteral("appearance/lightTheme"), false).toBool();
    applyTheme(m_lightTheme);

    // One-time: old default (40) is too low for block-based vibe workflows.
    if (!settings.value(QStringLiteral("agent/maxStepsMigrated_v2")).toBool()) {
        const int stored = settings.value(QStringLiteral("agent/maxSteps"), 40).toInt();
        if (stored <= 40) {
            settings.setValue(QStringLiteral("agent/maxSteps"), 200);
        }
        settings.setValue(QStringLiteral("agent/maxStepsMigrated_v2"), true);
    }

    connect(&LanguageManager::instance(), &LanguageManager::languageChanged,
            this, [this] { retranslateUi(); });
    connectSignals();

    if (m_workspace.isEmpty()) {
        enterNoProjectState();
    } else {
        updateRecentProjects(m_workspace);
        const QJsonObject options = loadBackendOptions();
        m_sessionId = options.value(QStringLiteral("session_id")).toString();
        if (!m_bridge->initialize(m_workspace, options)) {
            m_connectionLabel->setText(tr("后端不可用"));
        }
    }
    applyResponsiveLayout();
}

// ═══════════════════════════════════════════════════════════════
//  Built-in pages
// ═══════════════════════════════════════════════════════════════

QString MainWindow::projectsRoot() const
{
    const QString custom = QSettings().value(
        QStringLiteral("projects/root")).toString();
    if (!custom.isEmpty() && QFileInfo(custom).isDir()) {
        return custom;
    }
    return QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation)
        + QStringLiteral("/MoonCodingProjects");
}

QWidget *MainWindow::buildNewProjectPage()
{
    auto *page = new QWidget(this);
    auto *layout = new QVBoxLayout(page);
    layout->setContentsMargins(40, 40, 40, 40);
    layout->setSpacing(16);

    auto *emptyBanner = new QLabel(tr("还没有项目、请创建新项目"), page);
    emptyBanner->setObjectName(QStringLiteral("emptyProjectsBanner"));
    emptyBanner->setWordWrap(true);
    emptyBanner->setVisible(!hasAnyProjects());

    auto *title = new QLabel(tr("新建项目"), page);
    title->setObjectName(QStringLiteral("pageTitle"));

    auto *hint = new QLabel(
        tr("项目统一存储在：\n%1\n\n输入名称以创建新工作区。").arg(projectsRoot()),
        page);
    hint->setWordWrap(true);
    hint->setObjectName(QStringLiteral("panelHint"));

    auto *nameEdit = new QLineEdit(page);
    nameEdit->setPlaceholderText(tr("输入项目名称"));
    nameEdit->setMinimumHeight(48);

    auto *createBtn = new QPushButton(tr("创建并打开"), page);
    createBtn->setObjectName(QStringLiteral("primaryButton"));
    createBtn->setMinimumHeight(48);

    layout->addWidget(emptyBanner);
    layout->addWidget(title);
    layout->addWidget(hint);
    layout->addWidget(nameEdit);
    layout->addWidget(createBtn);
    layout->addStretch(1);

    connect(createBtn, &QPushButton::clicked, this, [this, nameEdit] {
        const QString name = nameEdit->text().trimmed();
        if (name.isEmpty()) {
            showFlashMessage(tr("请输入项目名称。"), 3000);
            return;
        }
        if (name.contains('/') || name.contains('\\')) {
            showFlashMessage(tr("项目名称不能包含路径分隔符。"), 3000);
            return;
        }
        createProjectHere(name);
    });

    return page;
}

QWidget *MainWindow::buildOpenProjectPage()
{
    auto *page = new QWidget(this);
    auto *layout = new QVBoxLayout(page);
    layout->setContentsMargins(40, 20, 40, 20);
    layout->setSpacing(10);

    auto *title = new QLabel(tr("打开项目"), page);
    title->setObjectName(QStringLiteral("pageTitle"));

    auto *hint = new QLabel(
        tr("项目目录：%1\n删除后无法找回，请谨慎操作。").arg(projectsRoot()),
        page);
    hint->setWordWrap(true);
    hint->setObjectName(QStringLiteral("panelHint"));

    auto *list = new QListWidget(page);
    list->setObjectName(QStringLiteral("projectList"));
    list->setSpacing(4);

    auto *btnRow = new QHBoxLayout();
    auto *openBtn = new QPushButton(tr("打开"), page);
    openBtn->setObjectName(QStringLiteral("primaryButton"));
    openBtn->setMinimumHeight(44);
    auto *deleteBtn = new QPushButton(tr("删除所选…"), page);
    deleteBtn->setMinimumHeight(44);
    btnRow->addWidget(openBtn, 1);
    btnRow->addWidget(deleteBtn, 1);

    layout->addWidget(title);
    layout->addWidget(hint);
    layout->addWidget(list, 1);
    layout->addLayout(btnRow);

    const QString root = projectsRoot();
    const QDir rootDir(root);
    if (rootDir.exists()) {
        const auto entries = rootDir.entryInfoList(
            QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
        for (const QFileInfo &entry : entries) {
            auto *item = new QListWidgetItem(entry.fileName());
            item->setData(Qt::UserRole, entry.absoluteFilePath());
            item->setSizeHint(QSize(0, 48));
            list->addItem(item);
        }
    }
    if (list->count() == 0) {
        auto *emptyItem = new QListWidgetItem(tr("暂无项目，请先创建。"));
        emptyItem->setFlags(Qt::NoItemFlags);
        list->addItem(emptyItem);
    }

    auto openSelected = [this, list] {
        QListWidgetItem *item = list->currentItem();
        if (!item) {
            showFlashMessage(tr("请先选择一个项目。"), 3000);
            return;
        }
        const QString path = item->data(Qt::UserRole).toString();
        if (!path.isEmpty()) {
            switchProject(path);
        }
    };

    connect(list, &QListWidget::itemActivated, this, [this](QListWidgetItem *item) {
        const QString path = item->data(Qt::UserRole).toString();
        if (!path.isEmpty()) {
            switchProject(path);
        }
    });
    connect(openBtn, &QPushButton::clicked, this, openSelected);
    connect(deleteBtn, &QPushButton::clicked, this, [this, list] {
        QListWidgetItem *item = list->currentItem();
        if (!item) {
            showFlashMessage(tr("请先选择要删除的项目。"), 3000);
            return;
        }
        const QString path = item->data(Qt::UserRole).toString();
        if (path.isEmpty()) {
            return;
        }
        deleteProject(path);
    });

    list->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(list, &QWidget::customContextMenuRequested, this, [this, list](const QPoint &pos) {
        QListWidgetItem *item = list->itemAt(pos);
        if (!item || item->data(Qt::UserRole).toString().isEmpty()) {
            return;
        }
        list->setCurrentItem(item);
        QMenu menu(list);
        const QString path = item->data(Qt::UserRole).toString();
        menu.addAction(tr("打开"), this, [this, path] { switchProject(path); });
        menu.addAction(tr("删除…"), this, [this, path] { deleteProject(path); });
        menu.exec(list->viewport()->mapToGlobal(pos));
    });

    return page;
}

QWidget *MainWindow::buildSettingsPage()
{
    auto *page = new QWidget(this);
    page->setObjectName(QStringLiteral("settingsPage"));
    page->setAutoFillBackground(true);
    auto *scrollArea = new QScrollArea(page);
    scrollArea->setObjectName(QStringLiteral("settingsScroll"));
    scrollArea->setWidgetResizable(true);
    scrollArea->setFrameShape(QFrame::NoFrame);
    scrollArea->viewport()->setAutoFillBackground(true);
    auto *container = new QWidget(scrollArea);
    container->setObjectName(QStringLiteral("settingsContainer"));
    container->setAutoFillBackground(true);
    scrollArea->setWidget(container);

    auto *pageLayout = new QVBoxLayout(page);
    pageLayout->setContentsMargins(0, 0, 0, 0);
    pageLayout->addWidget(scrollArea);

    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    auto *layout = new QFormLayout(container);
    layout->setContentsMargins(board ? 16 : 40, 16, board ? 16 : 40, 24);
    layout->setSpacing(12);
    layout->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);

    QSettings settings;

    auto *sourceCombo = new QComboBox(container);
    auto *sourceModel = new QStandardItemModel(sourceCombo);
    auto *customItem = new QStandardItem(tr("自定义 API"));
    customItem->setData(QStringLiteral("custom"), Qt::UserRole);
    sourceModel->appendRow(customItem);
    auto *managedItem = new QStandardItem(tr("托管 API（即将推出）"));
    managedItem->setData(QStringLiteral("managed"), Qt::UserRole);
    managedItem->setEnabled(false);
    sourceModel->appendRow(managedItem);
    sourceCombo->setModel(sourceModel);
    const QString curSource = settings.value(
        QStringLiteral("provider/api_source"), QStringLiteral("custom")).toString();
    sourceCombo->setCurrentIndex(curSource == QStringLiteral("managed") ? 1 : 0);

    auto *baseUrl = new QLineEdit(
        settings.value(QStringLiteral("provider/base_url")).toString(), container);
    baseUrl->setPlaceholderText(QStringLiteral("https://api.deepseek.com/v1"));
    baseUrl->setMinimumHeight(40);

    auto *apiKeyEdit = new QLineEdit(
        settings.value(QStringLiteral("provider/api_key")).toString(), container);
    apiKeyEdit->setEchoMode(QLineEdit::Password);
    apiKeyEdit->setPlaceholderText(tr("输入 API Key（本地存储，不会上传）"));
    apiKeyEdit->setMinimumHeight(40);
    auto *toggleKeyBtn = new QPushButton(tr("显示"), container);
    toggleKeyBtn->setCheckable(true);
    toggleKeyBtn->setMaximumWidth(60);
    toggleKeyBtn->setMinimumHeight(40);
    toggleKeyBtn->setFocusPolicy(Qt::NoFocus);
    auto *keyRow = new QHBoxLayout;
    keyRow->setContentsMargins(0, 0, 0, 0);
    keyRow->addWidget(apiKeyEdit);
    keyRow->addWidget(toggleKeyBtn);
    connect(toggleKeyBtn, &QPushButton::toggled, this, [apiKeyEdit](bool show) {
        apiKeyEdit->setEchoMode(show ? QLineEdit::Normal : QLineEdit::Password);
    });

    m_settingsModelBtn = new QPushButton(container);
    m_settingsModelBtn->setObjectName(QStringLiteral("primaryButton"));
    m_settingsModelBtn->setMinimumHeight(48);
    m_settingsModelBtn->setFocusPolicy(Qt::NoFocus);
    updateSettingsModelButton();
    auto *modelHint = new QLabel(tr("模型列表在独立页面选择，避免板端下拉框无法弹出。"), container);
    modelHint->setObjectName(QStringLiteral("mutedLabel"));
    modelHint->setWordWrap(true);

    auto *maxSteps = new QSpinBox(container);
    maxSteps->setRange(1, 1000);
    maxSteps->setValue(settings.value(QStringLiteral("agent/maxSteps"), 200).toInt());
    auto *temperature = new QDoubleSpinBox(container);
    temperature->setRange(0.0, 2.0);
    temperature->setSingleStep(0.05);
    temperature->setDecimals(2);
    temperature->setValue(settings.value(QStringLiteral("provider/temperature"), 0.1).toDouble());

    auto *fontCombo = new QFontComboBox(container);
    fontCombo->setCurrentFont(QFont(
        settings.value(QStringLiteral("appearance/fontFamily"), QStringLiteral("Segoe UI")).toString()));
    auto *fontSizeSpin = new QSpinBox(container);
    fontSizeSpin->setRange(9, 24);
    fontSizeSpin->setValue(settings.value(QStringLiteral("appearance/fontSize"), 13).toInt());
    fontSizeSpin->setSuffix(QStringLiteral(" px"));
    auto *fontRow = new QHBoxLayout;
    fontRow->setContentsMargins(0, 0, 0, 0);
    fontRow->addWidget(fontCombo, 1);
    fontRow->addWidget(fontSizeSpin);

    auto *langCombo = new QComboBox(container);
    langCombo->addItem(tr("中文"), QStringLiteral("zh"));
    langCombo->addItem(QStringLiteral("English"), QStringLiteral("en"));
    const QString currentLang = LanguageManager::instance().currentLanguage();
    langCombo->setCurrentIndex(currentLang == QStringLiteral("en") ? 1 : 0);

    auto makeSection = [container](const QString &text) -> QLabel * {
        auto *lbl = new QLabel(text, container);
        lbl->setObjectName(QStringLiteral("settingsSection"));
        return lbl;
    };

    layout->addRow(makeSection(tr("API 设置")));
    layout->addRow(tr("API 来源"), sourceCombo);
    layout->addRow(tr("Base URL"), baseUrl);
    layout->addRow(tr("API Key"), keyRow);
    layout->addRow(tr("模型"), m_settingsModelBtn);
    layout->addRow(QString(), modelHint);

    {
        auto *sep = new QFrame(container);
        sep->setFrameShape(QFrame::HLine);
        layout->addRow(sep);
    }
    layout->addRow(makeSection(tr("Agent 参数")));
    layout->addRow(tr("最大步数"), maxSteps);
    layout->addRow(tr("温度"), temperature);

    {
        auto *sep = new QFrame(container);
        sep->setFrameShape(QFrame::HLine);
        layout->addRow(sep);
    }
    layout->addRow(makeSection(tr("界面")));
    layout->addRow(tr("字体"), fontRow);
    layout->addRow(tr("语言"), langCombo);

    {
        auto *sep = new QFrame(container);
        sep->setFrameShape(QFrame::HLine);
        layout->addRow(sep);
    }
    layout->addRow(makeSection(tr("网络")));
    auto *wifiBtn = new QPushButton(tr("WiFi 设置…"), container);
    wifiBtn->setObjectName(QStringLiteral("primaryButton"));
    wifiBtn->setMinimumHeight(48);
    layout->addRow(wifiBtn);
    connect(wifiBtn, &QPushButton::clicked, this, &MainWindow::showWifiPage);
    connect(m_settingsModelBtn, &QPushButton::clicked, this, [this] {
        if (m_settingsModelBtn) {
            m_settingsModelBtn->setFocus(Qt::OtherFocusReason);
        }
        showModelsPage();
    });

    {
        auto *sep = new QFrame(container);
        sep->setFrameShape(QFrame::HLine);
        layout->addRow(sep);
    }
    layout->addRow(makeSection(tr("中文输入")));
    auto *imeHint = new QLabel(
        tr("点任意输入框自动弹出软键盘；也可点「键」。收起后再次点输入框可重新打开。"),
        container);
    imeHint->setObjectName(QStringLiteral("mutedLabel"));
    imeHint->setWordWrap(true);
    layout->addRow(imeHint);

    auto *saveBtn = new QPushButton(tr("保存并应用"), container);
    saveBtn->setObjectName(QStringLiteral("primaryButton"));
    saveBtn->setMinimumHeight(48);
    saveBtn->setFocusPolicy(Qt::NoFocus);
    layout->addRow(saveBtn);

    connect(saveBtn, &QPushButton::clicked, this, [this, sourceCombo, baseUrl, apiKeyEdit,
                                                    maxSteps, temperature, fontCombo, fontSizeSpin,
                                                    langCombo] {
        QSettings s;
        const QString apiSource = sourceCombo->currentData(Qt::UserRole).toString();
        s.setValue(QStringLiteral("provider/api_source"), apiSource);
        s.setValue(QStringLiteral("provider/base_url"), baseUrl->text().trimmed());
        s.setValue(QStringLiteral("provider/api_key"), apiKeyEdit->text().trimmed());
        s.setValue(QStringLiteral("agent/maxSteps"), maxSteps->value());
        s.setValue(QStringLiteral("provider/temperature"), temperature->value());
        s.setValue(QStringLiteral("appearance/fontFamily"), fontCombo->currentFont().family());
        s.setValue(QStringLiteral("appearance/fontSize"), fontSizeSpin->value());
        applyTheme(m_lightTheme);
        if (m_chat) {
            m_chat->refreshFonts();
        }
        const QString lang = langCombo->currentData().toString();
        LanguageManager::instance().setLanguage(lang);

        m_input->setContextModel(
            s.value(QStringLiteral("provider/model")).toString());
        updateSettingsModelButton();

        const QJsonObject options = loadBackendOptions();
        if (!m_bridge->isBusy()) {
            m_bridge->reinitialize(m_workspace, options);
            showFlashMessage(tr("设置已保存并立即生效。"), 3000);
        } else {
            showFlashMessage(tr("设置已保存，API 更改将在当前任务完成后生效。"), 4000);
        }
    });

    return page;
}

QWidget *MainWindow::buildModelsPage()
{
    auto *page = new QWidget(this);
    page->setObjectName(QStringLiteral("modelsPage"));
    page->setAutoFillBackground(true);
    auto *layout = new QVBoxLayout(page);
    layout->setContentsMargins(12, 12, 12, 12);
    layout->setSpacing(10);

    m_modelsStatusLabel = new QLabel(tr("点「刷新列表」从 API 拉取模型。点一项即可选用。"), page);
    m_modelsStatusLabel->setObjectName(QStringLiteral("mutedLabel"));
    m_modelsStatusLabel->setWordWrap(true);

    m_modelsRefreshBtn = new QPushButton(tr("刷新列表"), page);
    m_modelsRefreshBtn->setObjectName(QStringLiteral("primaryButton"));
    m_modelsRefreshBtn->setMinimumHeight(48);
    m_modelsRefreshBtn->setFocusPolicy(Qt::NoFocus);

    m_modelsList = new QListWidget(page);
    m_modelsList->setObjectName(QStringLiteral("modelsList"));
    m_modelsList->setFocusPolicy(Qt::StrongFocus);
    m_modelsList->setUniformItemSizes(true);

    layout->addWidget(m_modelsStatusLabel);
    layout->addWidget(m_modelsRefreshBtn);
    layout->addWidget(m_modelsList, 1);

    connect(m_modelsRefreshBtn, &QPushButton::clicked, this, &MainWindow::refreshModelsPage);
    connect(m_modelsList, &QListWidget::itemClicked, this, [this](QListWidgetItem *item) {
        if (!item) {
            return;
        }
        const QString model = item->text().trimmed();
        if (model.isEmpty()) {
            return;
        }
        QSettings().setValue(QStringLiteral("provider/model"), model);
        if (m_input) {
            m_input->setContextModel(model);
        }
        updateSettingsModelButton();
        showFlashMessage(tr("已选择模型：%1").arg(model), 2500);
        if (!m_bridge->isBusy()) {
            m_bridge->reinitialize(m_workspace, loadBackendOptions());
        }
        showSettings();
    });

    return page;
}

void MainWindow::updateSettingsModelButton()
{
    if (!m_settingsModelBtn) {
        return;
    }
    const QString model = QSettings().value(QStringLiteral("provider/model")).toString().trimmed();
    if (model.isEmpty()) {
        m_settingsModelBtn->setText(tr("选择模型…"));
    } else {
        m_settingsModelBtn->setText(tr("模型：%1").arg(model));
    }
}

void MainWindow::refreshModelsPage()
{
    if (!m_modelsRefreshBtn || !m_modelsStatusLabel || !m_modelsList || !m_modelFetcher) {
        return;
    }
    m_modelsRefreshBtn->setEnabled(false);
    m_modelsRefreshBtn->setText(tr("拉取中…"));
    m_modelsStatusLabel->setText(tr("已收到点击，正在拉取模型列表…"));
    m_modelsRefreshBtn->update();
    m_modelsStatusLabel->update();

    QSettings s;
    const QString baseUrl = s.value(QStringLiteral("provider/base_url")).toString();
    const QString apiKey = s.value(QStringLiteral("provider/api_key")).toString();
    const QString saved = s.value(QStringLiteral("provider/model")).toString();

    m_modelFetcher->fetch(baseUrl, apiKey);
    connect(
        m_modelFetcher, &ModelFetcher::finished, this,
        [this, saved](const QStringList &models, const QString &error) {
            if (m_modelsRefreshBtn) {
                m_modelsRefreshBtn->setEnabled(true);
                m_modelsRefreshBtn->setText(tr("刷新列表"));
            }
            if (!m_modelsList || !m_modelsStatusLabel) {
                return;
            }
            if (!error.isEmpty() && models.isEmpty()) {
                m_modelsStatusLabel->setText(error);
                return;
            }
            m_modelsList->clear();
            for (const QString &m : models) {
                auto *item = new QListWidgetItem(m, m_modelsList);
                item->setSizeHint(QSize(0, 48));
                if (m == saved) {
                    item->setSelected(true);
                    m_modelsList->setCurrentItem(item);
                }
            }
            if (m_modelsList->count() == 0) {
                m_modelsStatusLabel->setText(tr("未获取到模型，请检查 Base URL / API Key / 网络。"));
            } else {
                m_modelsStatusLabel->setText(
                    tr("已加载 %1 个模型，点一项即可选用。").arg(m_modelsList->count()));
            }
        },
        Qt::SingleShotConnection);
}

QWidget *MainWindow::buildWifiPage()
{
    auto *page = new QWidget(this);
    page->setObjectName(QStringLiteral("wifiPage"));
    page->setAutoFillBackground(true);
    auto *layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    m_wifiPanel = new WifiPanel(page);
    layout->addWidget(m_wifiPanel, 1);
    connect(m_wifiPanel, &WifiPanel::statusChanged, this, [this](const WifiStatusInfo &info) {
        if (m_networkLabel) {
            m_networkLabel->setText(info.summary);
            m_networkLabel->setToolTip(
                info.connected
                    ? tr("已连接 %1%2")
                          .arg(info.ssid,
                               info.ip.isEmpty() ? QString() : tr(" · %1").arg(info.ip))
                    : tr("未连接 — 点此打开 WiFi 设置"));
        }
    });
    return page;
}

// ═══════════════════════════════════════════════════════════════
//  Navigation
// ═══════════════════════════════════════════════════════════════

void MainWindow::goToPage(PageIndex idx)
{
    m_currentPage = idx;
    m_pages->setCurrentIndex(idx);
    const bool isMain = (idx == ChatPage || idx == TreePage);
    m_mainHeader->setVisible(isMain);
    m_subHeader->setVisible(!isMain);

    switch (idx) {
    case ChatPage:
        if (width() >= kWideLayoutThreshold && m_treePanelVisible) {
            reparentTreeToSidePanel();
        }
        applyResponsiveLayout();
        break;
    case TreePage:
        m_treeSideHost->hide();
        reparentTreeToFullPage();
        break;
    case AppsPage:
        m_subHeaderTitle->setText(tr("应用"));
        if (m_apps) m_apps->refresh();
        break;
    case NewProjectPage:
        m_subHeaderTitle->setText(tr("新建项目"));
        if (auto *banner = m_pages->widget(NewProjectPage)
                               ->findChild<QLabel *>(QStringLiteral("emptyProjectsBanner"))) {
            banner->setVisible(!hasAnyProjects());
        }
        break;
    case OpenProjectPage:
        m_subHeaderTitle->setText(tr("打开项目"));
        if (auto *list = m_pages->widget(OpenProjectPage)->findChild<QListWidget *>()) {
            list->clear();
            const QDir rootDir(projectsRoot());
            if (rootDir.exists()) {
                for (const QFileInfo &entry : rootDir.entryInfoList(
                         QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
                    auto *item = new QListWidgetItem(entry.fileName());
                    item->setData(Qt::UserRole, entry.absoluteFilePath());
                    item->setSizeHint(QSize(0, 48));
                    list->addItem(item);
                }
            }
            if (list->count() == 0) {
                auto *empty = new QListWidgetItem(tr("暂无项目，请先创建。"));
                empty->setFlags(Qt::NoItemFlags);
                list->addItem(empty);
            }
        }
        break;
    case SettingsPage:
        m_subHeaderTitle->setText(tr("设置"));
        updateSettingsModelButton();
        break;
    case WifiPage:
        m_subHeaderTitle->setText(tr("WiFi"));
        if (m_wifiPanel) {
            m_wifiPanel->refreshStatus();
        }
        break;
    case ModelsPage:
        m_subHeaderTitle->setText(tr("选择模型"));
        break;
    }
    updateNavStates();
}

void MainWindow::showChatPage()
{
    if (m_workspace.isEmpty()) {
        goToPage(NewProjectPage);
        return;
    }
    goToPage(ChatPage);
}
void MainWindow::showTreePage()
{
    if (m_workspace.isEmpty()) {
        goToPage(NewProjectPage);
        return;
    }
    if (width() >= kWideLayoutThreshold && m_currentPage == ChatPage) {
        m_treePanelVisible = !m_treePanelVisible;
        QSettings().setValue(
            QStringLiteral("appearance/treePanelVisible"), m_treePanelVisible);
        if (m_treePanelVisible) {
            reparentTreeToSidePanel();
        }
        applyResponsiveLayout();
        updateNavStates();
        return;
    }
    goToPage(TreePage);
}
void MainWindow::showAppsPage()
{
    if (m_workspace.isEmpty()) {
        goToPage(NewProjectPage);
        return;
    }
    goToPage(AppsPage);
}
void MainWindow::showNewProjectPage() { goToPage(NewProjectPage); }
void MainWindow::showOpenProjectPage() { goToPage(OpenProjectPage); }
void MainWindow::showSettings() { goToPage(SettingsPage); }
void MainWindow::showWifiPage() { goToPage(WifiPage); }
void MainWindow::showModelsPage() { goToPage(ModelsPage); }

void MainWindow::refreshNetworkStatus()
{
    const WifiStatusInfo info = queryWifiStatus();
    if (!m_networkLabel) {
        return;
    }
    m_networkLabel->setText(info.summary);
    m_networkLabel->setToolTip(
        info.connected
            ? tr("已连接 %1%2 — 点此打开 WiFi")
                  .arg(info.ssid, info.ip.isEmpty() ? QString() : tr(" · %1").arg(info.ip))
            : tr("未连接 — 点此打开 WiFi 设置"));

#if defined(Q_OS_UNIX)
    // Product board: auto-heal when wpa dies / lease lost (AIC8800 common).
    // Never run recover while the agent is streaming — reduces UI-thread races.
    if (!info.connected && qEnvironmentVariableIsSet("MOONCODING_BOARD")
        && !(m_bridge && m_bridge->isBusy())) {
        const qint64 now = QDateTime::currentMSecsSinceEpoch();
        if (now - m_lastAutoNetHealAt >= 120000) {
            if (!m_netRecover || !m_netRecover->isRunning()) {
                m_lastAutoNetHealAt = now;
                if (!m_netRecover) {
                    m_netRecover = new BoardNetRecover(this);
                    connect(m_netRecover, &BoardNetRecover::finished, this,
                            [this](bool, const QString &) { refreshNetworkStatus(); });
                }
                m_netRecover->start();
            }
        }
    }
#endif
}

void MainWindow::enableBoardTouchScroll()
{
    touchscroll::enableRecursive(this);
}

bool MainWindow::eventFilter(QObject *watched, QEvent *event)
{
    if (watched == m_networkLabel) {
        if (event->type() == QEvent::MouseButtonPress) {
            m_networkPressAt = QDateTime::currentMSecsSinceEpoch();
            return true;
        }
        if (event->type() == QEvent::MouseButtonRelease) {
            const qint64 held = QDateTime::currentMSecsSinceEpoch() - m_networkPressAt;
            if (held >= 650) {
                if (m_netRecover && m_netRecover->isRunning()) {
                    showFlashMessage(tr("网络恢复已在进行中…"), 2000);
                } else {
                    showFlashMessage(tr("已收到长按 — 正在后台恢复网络…"), 2500);
                    if (!m_netRecover) {
                        m_netRecover = new BoardNetRecover(this);
                        connect(m_netRecover, &BoardNetRecover::finished, this,
                                [this](bool ok, const QString &) {
                                    showFlashMessage(
                                        ok ? tr("网络已恢复")
                                           : tr("恢复失败，请打开 WiFi 页点「一键恢复网络」"),
                                        3500);
                                    refreshNetworkStatus();
                                });
                    }
                    m_netRecover->start();
                }
            } else {
                showWifiPage();
            }
            return true;
        }
    }
    return QMainWindow::eventFilter(watched, event);
}

void MainWindow::showProjectMenu()
{
    QMenu menu(this);
    menu.addAction(tr("新建项目…"), this, &MainWindow::showNewProjectPage);
    menu.addAction(tr("打开项目…"), this, &MainWindow::showOpenProjectPage);
    menu.addAction(tr("删除当前项目…"), this, &MainWindow::deleteCurrentProject);
    menu.addSeparator();

    QSettings settings;
    const QStringList recent =
        settings.value(QStringLiteral("recentProjects")).toStringList();
    if (recent.isEmpty()) {
        menu.addAction(tr("暂无最近项目"))->setEnabled(false);
    } else {
        for (const QString &path : recent) {
            const QString label = QFileInfo(path).fileName();
            menu.addAction(label, this, [this, path] { switchProject(path); });
        }
    }
    menu.exec(m_projectButton->mapToGlobal(QPoint(0, m_projectButton->height())));
}

// ═══════════════════════════════════════════════════════════════
//  Project management
// ═══════════════════════════════════════════════════════════════

void MainWindow::createProjectHere(const QString &name)
{
    const QString root = projectsRoot();
    QDir().mkpath(root);
    const QString projectPath = QDir(root).filePath(name);
    if (QDir(projectPath).exists()) {
        showFlashMessage(tr("项目「%1」已存在。").arg(name), 3000);
        return;
    }
    if (!QDir().mkpath(projectPath)) {
        showFlashMessage(tr("创建项目文件夹失败。"), 3000);
        return;
    }
    // Seed an isolated empty web project — never inherit sibling apps/.
    seedNewProjectFiles(projectPath);
    // Force a fresh session id for this workspace key (no chat bleed).
    {
        QSettings settings;
        settings.beginGroup(QStringLiteral("workspaces/") + workspaceKey(projectPath));
        settings.setValue(
            QStringLiteral("sessionId"),
            QUuid::createUuid().toString(QUuid::WithoutBraces));
        settings.endGroup();
    }
    switchWorkspace(projectPath);
    showChatPage();
    showFlashMessage(tr("已创建独立工作区：%1").arg(name), 2500);
}

void MainWindow::seedNewProjectFiles(const QString &projectPath)
{
    const QString indexPath = QDir(projectPath).filePath(QStringLiteral("index.html"));
    if (QFileInfo::exists(indexPath)) {
        return;
    }
    QFile f(indexPath);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return;
    }
    const QByteArray html = R"HTML(<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>新项目</title>
<style>
:root { color-scheme: light; }
body { margin: 0; font-family: sans-serif; background: #f4f1ea; color: #1c1917; }
main { min-height: 100vh; padding: 28px 18px; box-sizing: border-box; }
h1 { font-size: 1.75rem; margin: 0 0 12px; }
p { font-size: 1rem; line-height: 1.55; margin: 0 0 18px; }
.hint { opacity: 0.75; font-size: 0.92rem; }
</style>
</head>
<body>
<main>
  <h1>独立工作区</h1>
  <p>本项目与其他项目完全隔离。请在聊天中从本目录的 index.html 开始新建竖屏应用。</p>
  <p class="hint">布局按手机竖屏 720×1280；入口必须是 index.html。</p>
</main>
</body>
</html>
)HTML";
    f.write(html);
}

void MainWindow::switchProject(const QString &workspace)
{
    if (m_bridge->isBusy()) {
        showFlashMessage(tr("请先停止 Agent 再切换项目。"), 3000);
        return;
    }
    switchWorkspace(workspace);
    showChatPage();
}

void MainWindow::deleteCurrentProject()
{
    deleteProject(m_workspace);
}

bool MainWindow::isUnderProjectsRoot(const QString &workspace) const
{
    const QString root = QDir::cleanPath(projectsRoot());
    const QString path = QDir::cleanPath(workspace);
    if (path == root) {
        return false;
    }
    const QString prefix = root + QLatin1Char('/');
    return path.startsWith(prefix, Qt::CaseInsensitive)
        || path.startsWith(QDir::toNativeSeparators(prefix), Qt::CaseInsensitive);
}

bool MainWindow::hasAnyProjects() const
{
    const QDir rootDir(projectsRoot());
    if (!rootDir.exists()) {
        return false;
    }
    return !rootDir.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot).isEmpty();
}

QString MainWindow::fallbackProjectAfterDelete(const QString &deletedPath) const
{
    const QString root = projectsRoot();
    const QDir rootDir(root);
    if (!rootDir.exists()) {
        return QString();
    }
    const QString deleted = QDir::cleanPath(deletedPath);
    for (const QFileInfo &entry : rootDir.entryInfoList(
             QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name)) {
        const QString path = QDir::cleanPath(entry.absoluteFilePath());
        if (path.compare(deleted, Qt::CaseInsensitive) != 0) {
            return path;
        }
    }
    return QString();
}

void MainWindow::enterNoProjectState()
{
    m_bridge->shutdown();
    m_workspace.clear();
    QSettings().remove(QStringLiteral("lastWorkspace"));
    m_projectButton->setText(tr("无项目"));
    m_projectButton->setToolTip(tr("还没有项目、请创建新项目"));
    m_connectionLabel->setText(tr("还没有项目、请创建新项目"));
    m_chat->clear();
    m_tree->setTree(QJsonObject{
        {QStringLiteral("version"), 0},
        {QStringLiteral("nodes"), QJsonArray{}},
    });
    goToPage(NewProjectPage);
}

void MainWindow::deleteProject(const QString &workspace)
{
    if (m_bridge->isBusy()) {
        showFlashMessage(tr("请先停止 Agent 再删除项目。"), 3000);
        return;
    }

    const QString path = QDir::cleanPath(workspace);
    const QFileInfo info(path);
    if (!info.exists() || !info.isDir()) {
        showFlashMessage(tr("项目不存在：%1").arg(path), 3000);
        return;
    }
    if (!isUnderProjectsRoot(path)) {
        showFlashMessage(
            tr("只能删除「%1」下的项目，不能删除外部目录。").arg(projectsRoot()),
            4000);
        return;
    }

    const QString name = info.fileName();
    const auto reply = QMessageBox::warning(
        this,
        tr("删除项目"),
        tr("无法找回，是否确认删除「%1」？\n\n将永久删除整个文件夹：\n%2")
            .arg(name, QDir::toNativeSeparators(path)),
        QMessageBox::Yes | QMessageBox::No,
        QMessageBox::No);
    if (reply != QMessageBox::Yes) {
        return;
    }

    const bool deletingCurrent =
        QDir::cleanPath(m_workspace).compare(path, Qt::CaseInsensitive) == 0;
    const QString nextWorkspace = deletingCurrent
        ? fallbackProjectAfterDelete(path)
        : QString();

    if (deletingCurrent) {
        // Leave the workspace before wiping files that the backend may hold open.
        if (!nextWorkspace.isEmpty()) {
            switchWorkspace(nextWorkspace);
        } else {
            m_bridge->shutdown();
        }
    }

    QDir dir(path);
    if (dir.exists() && !dir.removeRecursively()) {
        showFlashMessage(tr("删除失败：%1").arg(name), 4000);
        if (deletingCurrent && nextWorkspace.isEmpty() && QFileInfo(path).isDir()) {
            // Restore backend if the only project could not be removed.
            const QJsonObject options = loadBackendOptions();
            m_sessionId = options.value(QStringLiteral("session_id")).toString();
            m_bridge->initialize(path, options);
        }
        return;
    }

    removeFromRecentProjects(path);
    showFlashMessage(tr("已删除项目「%1」。").arg(name), 3000);

    if (deletingCurrent && nextWorkspace.isEmpty()) {
        enterNoProjectState();
    } else if (m_pages->currentIndex() == OpenProjectPage) {
        goToPage(OpenProjectPage); // refresh list
    } else if (deletingCurrent) {
        showChatPage();
    }
}

void MainWindow::switchWorkspace(const QString &workspace)
{
    QString normalized = QDir::cleanPath(workspace);
    QFileInfo info(normalized);
    if (!info.isDir()) {
        showFlashMessage(tr("不是目录：%1").arg(normalized), 3000);
        return;
    }
    m_workspace = normalized;
    QSettings().setValue(QStringLiteral("lastWorkspace"), normalized);
    m_projectButton->setText(info.fileName());
    m_projectButton->setToolTip(m_workspace);
    updateRecentProjects(m_workspace);
    if (m_apps) {
        m_apps->setWorkspace(m_workspace);
    }

    const QJsonObject options = loadBackendOptions();
    m_sessionId = options.value(QStringLiteral("session_id")).toString();
    m_chat->clear();
    if (!m_bridge->reinitialize(m_workspace, options)) {
        m_connectionLabel->setText(tr("后端不可用"));
        return;
    }
}

void MainWindow::newConversation()
{
    if (m_bridge->isBusy()) {
        showFlashMessage(tr("请先停止 Agent 再开始新对话。"), 3000);
        return;
    }
    switchSession(QUuid::createUuid().toString(QUuid::WithoutBraces));
}

void MainWindow::switchSession(const QString &sessionId)
{
    if (m_bridge->isBusy()) {
        showFlashMessage(tr("请先停止 Agent 再切换对话。"), 3000);
        return;
    }
    if (sessionId == m_sessionId && m_bridge->isReady()) {
        m_bridge->loadSession(sessionId);
        return;
    }
    m_sessionId = sessionId;
    persistSessionId();
    QJsonObject options = loadBackendOptions();
    options.insert(QStringLiteral("session_id"), sessionId);
    m_chat->clear();
    if (!m_bridge->reinitialize(m_workspace, options)) {
        m_connectionLabel->setText(tr("后端不可用"));
        return;
    }
}

// ═══════════════════════════════════════════════════════════════
//  Flash banner
// ═══════════════════════════════════════════════════════════════

void MainWindow::showFlashMessage(const QString &msg, int durationMs)
{
    m_flashBanner->setText(msg);
    m_flashBanner->show();
    m_flashTimer->start(durationMs);
}

// ═══════════════════════════════════════════════════════════════
//  Theme & events
// ═══════════════════════════════════════════════════════════════

void MainWindow::closeEvent(QCloseEvent *event)
{
    if (m_bridge->isBusy()) {
        m_closing = true;
        m_connectionLabel->setText(tr("正在停止 Agent，稍后关闭…"));
        m_bridge->interrupt();
        event->ignore();
        return;
    }
    m_closing = false;
    QMainWindow::closeEvent(event);
}

void MainWindow::resizeEvent(QResizeEvent *event)
{
    QMainWindow::resizeEvent(event);
    applyResponsiveLayout();
}

void MainWindow::submitMessage(const QString &message)
{
#ifdef Q_OS_UNIX
    // Soft check only — never ICMP-block the UI thread.
    // Detects AIC8800 "zombie WiFi": COMPLETED + route but gateway ARP stuck.
    if (qEnvironmentVariableIsSet("MOONCODING_BOARD") && !boardNetPingInternet()) {
        showFlashMessage(tr("链路异常 — 正在轻量恢复（重关联），请稍后再发"), 3500);
        if (!m_netRecover) {
            m_netRecover = new BoardNetRecover(this);
            connect(m_netRecover, &BoardNetRecover::finished, this,
                    [this](bool ok, const QString &) {
                        showFlashMessage(
                            ok ? tr("链路已恢复，可以继续对话")
                               : tr("恢复失败，请打开 WiFi 页点「一键恢复网络」"),
                            3500);
                        refreshNetworkStatus();
                    });
        }
        if (!m_netRecover->isRunning()) {
            m_netRecover->start();
        }
        return;
    }
#endif
    if (!m_bridge->sendMessage(message)) {
        showFlashMessage(tr("Agent 正忙，请稍后再试"), 3000);
        return;
    }
    m_chat->appendUserMessage(message);
    showChatPage();
    m_input->clearDraft();
}

void MainWindow::toggleTheme()
{
    m_lightTheme = !m_lightTheme;
    QSettings().setValue(QStringLiteral("appearance/lightTheme"), m_lightTheme);
    applyTheme(m_lightTheme);
}

void MainWindow::updateTokenStatus(quint64 tokensIn, quint64 tokensOut, quint64 steps)
{
    m_tokenLabel->setText(
        tr("%1 步 · %2 入 / %3 出").arg(steps).arg(tokensIn).arg(tokensOut));
}

void MainWindow::applyTheme(bool light)
{
    QFile stylesheet(light ? QStringLiteral(":/styles/light.qss") : QStringLiteral(":/styles/dark.qss"));
    if (stylesheet.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QByteArray css = stylesheet.readAll();
        if (css.trimmed().isEmpty()) {
            qWarning("MoonCoding theme stylesheet is empty — UI will look unstyled");
        } else {
            qInfo("MoonCoding theme loaded: %s (%d bytes)",
                  light ? "light" : "dark", int(css.size()));
        }
        applyAppFont();
        const int sz = configuredUiFontSize();
        const int szSm = qMax(9, sz - 1);
        const int szLg = sz + 2;
        const int szBrand = sz + 5;

        // Board: touch metrics — font sizes track the user setting.
        if (qEnvironmentVariableIsSet("MOONCODING_BOARD")) {
            css += QStringLiteral(R"(
#appHeader { padding: 4px 8px; }
#brand { font-size: %1px; font-weight: 700; }
#projectButton { min-height: 40px; font-size: %2px; font-weight: 700; }
#navButton {
    min-width: 0px; min-height: 48px; padding: 8px 4px;
    font-weight: 700; font-size: %2px; border-radius: 8px;
}
#promptEditor { border-width: 2px; border-radius: 10px; padding: 10px; font-size: %3px; font-weight: 600; }
#sendButton { min-height: 40px; min-width: 64px; max-width: 80px; border-radius: 8px; font-size: %2px; font-weight: 700; padding: 6px 8px; }
#attachButton { min-width: 40px; min-height: 40px; max-width: 40px; font-weight: 700; }
QStatusBar { min-height: 28px; font-weight: 600; }
)")
                       .arg(szBrand)
                       .arg(sz)
                       .arg(szLg)
                       .toUtf8();
        }

        // Append last so user font size wins over hardcoded 14px in theme files.
        // Do NOT put SoftKeyboard keys in the general QPushButton rule — they need larger glyphs.
        css += QStringLiteral(R"(
QMainWindow, QDialog, #ChatScrollArea, #MessageContainer, #InputFrame, #ContextBar {
    font-size: %1px;
    font-weight: 600;
}
QLabel, QLineEdit, QTextEdit, QPlainTextEdit, QTextBrowser,
QComboBox, QAbstractSpinBox, QListWidget, QTreeView, QCheckBox, QRadioButton,
QMenu, QStatusBar, #mutedLabel, #composerFooter,
#contextInfo, #promptEditor, #sendButton, #settingsSection, #networkStatus {
    font-size: %1px;
    font-weight: 600;
}
QPushButton, QToolButton {
    font-size: %1px;
    font-weight: 700;
}
#messageBody { font-size: %1px; font-weight: 600; }
#brand { font-size: %2px; font-weight: 700; }
#softKeyboard #imeCompose { font-size: %3px; font-weight: 700; }
#softKeyboard #imeKey, #softKeyboard #imeKeyWide, #softKeyboard #imeKeySpace {
    font-size: %4px; font-weight: 700;
}
#softKeyboard #imeCandidate { font-size: %5px; font-weight: 700; }
#softKeyboard #imePageBtn {
    font-size: %6px; font-weight: 700; min-width: 72px;
}
#softKeyboard #imeKeyWide, #softKeyboard #imeKeySpace { font-size: %6px; font-weight: 700; }
)")
                   .arg(sz)
                   .arg(szBrand)
                   .arg(qMax(16, sz + 2))
                   .arg(qMax(22, sz + 8))
                   .arg(qMax(20, sz + 6))
                   .arg(qMax(16, sz + 3))
                   .toUtf8();
        Q_UNUSED(szSm);

        qApp->setStyleSheet(QString::fromUtf8(css));
        // Force every widget to re-evaluate objectName selectors after swap.
        const auto widgets = QApplication::allWidgets();
        for (QWidget *w : widgets) {
            if (!w) {
                continue;
            }
            w->style()->unpolish(w);
            w->style()->polish(w);
            w->update();
        }
    } else {
        qWarning("MoonCoding failed to open theme stylesheet");
        applyAppFont();
    }
}

void MainWindow::toggleHistoryPanel()
{
    showChatPage();
    m_historyVisible = !m_historyVisible;
    m_historyPanel->setVisible(m_historyVisible);
    m_historyNav->setChecked(m_historyVisible);
    if (m_historyVisible) {
        m_bridge->refreshSessions();
    }
    updateNavStates();
}

void MainWindow::onHistoryItemActivated(QListWidgetItem *item)
{
    if (!item) return;
    const QString sessionId = item->data(Qt::UserRole).toString();
    if (sessionId.isEmpty() || sessionId == m_sessionId) return;
    switchSession(sessionId);
    showChatPage();
}

// ═══════════════════════════════════════════════════════════════
//  Backend options, signals, helpers
// ═══════════════════════════════════════════════════════════════

QJsonObject MainWindow::loadBackendOptions() const
{
    QSettings settings;
    const QString key = workspaceKey(m_workspace);
    settings.beginGroup(QStringLiteral("workspaces/") + key);
    QString sessionId = settings.value(QStringLiteral("sessionId")).toString();
    if (sessionId.isEmpty()) {
        sessionId = QUuid::createUuid().toString(QUuid::WithoutBraces);
        settings.setValue(QStringLiteral("sessionId"), sessionId);
    }
    settings.endGroup();

    QJsonObject options{{QStringLiteral("session_id"), sessionId}};
    options.insert(QStringLiteral("language"), LanguageManager::instance().currentLanguage());

    const QString apiSource = settings.value(
        QStringLiteral("provider/api_source"), QStringLiteral("custom")).toString();
    options.insert(QStringLiteral("api_source"), apiSource);
    if (apiSource == QStringLiteral("managed")) {
        const QString endpoint = settings.value(QStringLiteral("provider/managed_endpoint")).toString();
        const QString authToken = settings.value(QStringLiteral("provider/managed_auth_token")).toString();
        const QString projectId = settings.value(QStringLiteral("provider/managed_project_id")).toString();
        if (!endpoint.isEmpty()) options.insert(QStringLiteral("managed_endpoint"), endpoint);
        if (!authToken.isEmpty()) options.insert(QStringLiteral("managed_auth_token"), authToken);
        if (!projectId.isEmpty()) options.insert(QStringLiteral("managed_project_id"), projectId);
    }

    const QString baseUrl = settings.value(QStringLiteral("provider/base_url")).toString();
    const QString model = settings.value(QStringLiteral("provider/model")).toString();
    const QString apiKey = settings.value(QStringLiteral("provider/api_key")).toString();
    if (!baseUrl.isEmpty()) options.insert(QStringLiteral("base_url"), baseUrl);
    if (!model.isEmpty()) options.insert(QStringLiteral("model"), model);
    if (!apiKey.isEmpty()) options.insert(QStringLiteral("api_key"), apiKey);

    options.insert(QStringLiteral("max_steps"),
        settings.value(QStringLiteral("agent/maxSteps"), 200).toInt());
    options.insert(QStringLiteral("temperature"),
        settings.value(QStringLiteral("provider/temperature"), 0.1).toDouble());
    options.insert(QStringLiteral("max_tokens"),
        settings.value(QStringLiteral("provider/max_tokens"), 8192).toInt());

#ifdef Q_OS_WIN
    const QString vibeName = QStringLiteral("vibe.exe");
#else
    const QString vibeName = QStringLiteral("vibe");
#endif
    options.insert(QStringLiteral("vibe_exe"),
        QCoreApplication::applicationDirPath() + QLatin1Char('/') + vibeName);
    return options;
}

void MainWindow::connectSignals()
{
    connect(m_input, &InputWidget::messageSubmitted, this, &MainWindow::submitMessage);
    connect(m_input, &InputWidget::interruptRequested, m_bridge, &RustBridge::interrupt);
    connect(m_bridge, &RustBridge::busyChanged, m_input, &InputWidget::setAgentBusy);
    connect(m_bridge, &RustBridge::busyChanged, m_tree, &TreeWidget::setAgentBusy);
    connect(m_bridge, &RustBridge::busyChanged, this, [this](bool busy) {
        m_connectionLabel->setText(busy ? tr("Agent 工作中") : tr("后端就绪"));
        if (busy) {
            m_input->setContextSteps(0);
            m_input->setContextTokens(0, 0);
            if (m_networkTimer) {
                m_networkTimer->stop();
            }
        } else {
            if (m_networkTimer && !m_networkTimer->isActive()) {
                m_networkTimer->start();
            }
            refreshNetworkStatus();
        }
        if (!busy && m_closing) QTimer::singleShot(0, this, &QWidget::close);
    });
    connect(m_bridge, &RustBridge::readyChanged, this, [this](bool ready) {
        m_connectionLabel->setText(ready ? tr("后端就绪") : tr("后端不可用"));
        m_input->setBackendReady(ready);
        m_tree->setBackendReady(ready);
    });
    connect(m_bridge, &RustBridge::thinking, m_chat, &ChatWidget::beginAssistantMessage);
    connect(m_bridge, &RustBridge::thinkingDelta, m_chat, &ChatWidget::appendThinkingDelta);
    connect(m_bridge, &RustBridge::textDelta, m_chat, &ChatWidget::appendAssistantDelta);
    connect(m_bridge, &RustBridge::textDone, m_chat, &ChatWidget::finishAssistantMessage);
    connect(m_bridge, &RustBridge::toolCallStarted, m_chat, &ChatWidget::showToolStart);
    connect(m_bridge, &RustBridge::toolCallFinished, m_chat, &ChatWidget::showToolResult);
    connect(m_bridge, &RustBridge::treeUpdated, m_tree, &TreeWidget::setTree);
    connect(m_bridge, &RustBridge::treeUpdated, this, [this](const QJsonObject &tree) {
        if (m_bridge && m_bridge->isBusy()) {
            return;
        }
        updateActiveNodeBanner(tree);
    });
    connect(m_bridge, &RustBridge::agentDone, m_chat, &ChatWidget::agentDone);
    connect(m_bridge, &RustBridge::agentDone, this, &MainWindow::updateTokenStatus);
    connect(m_bridge, &RustBridge::agentDone, this, [this](quint64 tokensIn, quint64 tokensOut, quint64 steps) {
        m_input->setContextTokens(tokensIn, tokensOut);
        m_input->setContextSteps(steps);
    });
    connect(m_bridge, &RustBridge::errorOccurred, m_chat, &ChatWidget::showError);
    connect(m_bridge, &RustBridge::interrupted, m_chat, &ChatWidget::showInterrupted);
    connect(m_bridge, &RustBridge::sessionsUpdated, this, &MainWindow::populateHistory);
    connect(m_bridge, &RustBridge::sessionLoaded, this, [this](const QJsonObject &session) {
        m_sessionId = session.value(QStringLiteral("id")).toString(m_sessionId);
        persistSessionId();
        const QJsonArray messages = session.value(QStringLiteral("messages")).toArray();
        if (messages.isEmpty()) {
            m_chat->clear();
            m_chat->beginAssistantMessage();
            m_chat->finishAssistantMessage(
                tr("已就绪。在聊天中描述目标，我会边推进边构建项目树。"
                   "你随时可以在项目树面板中修正任何节点。"),
                0, 0);
            m_chat->agentDone(0, 0, 0);
        } else {
            m_chat->setMessages(messages);
        }
        const quint64 tokensIn = session.value(QStringLiteral("tokens_in")).toInteger();
        const quint64 tokensOut = session.value(QStringLiteral("tokens_out")).toInteger();
        const quint64 steps = session.value(QStringLiteral("steps")).toInteger();
        updateTokenStatus(tokensIn, tokensOut, steps);
        m_input->setContextTokens(tokensIn, tokensOut);
        m_input->setContextSteps(steps);
        const QJsonValue treeValue = session.value(QStringLiteral("tree"));
        if (treeValue.isObject()) {
            m_tree->setTree(treeValue.toObject());
            updateActiveNodeBanner(treeValue.toObject());
        } else {
            m_bridge->refreshTree();
        }
    });

    connect(m_tree, &TreeWidget::addRequested, m_bridge, &RustBridge::addTreeNode);
    connect(m_tree, &TreeWidget::updateRequested, m_bridge, &RustBridge::updateTreeNode);
    connect(m_tree, &TreeWidget::deleteRequested, m_bridge, &RustBridge::deleteTreeNode);
    connect(m_tree, &TreeWidget::reviewNodeRequested, this, [this](const QString &nodeId) {
        if (!m_bridge->reviewNode(nodeId)) return;
        m_chat->appendUserMessage(tr("严格审视项目树节点 `%1`。").arg(nodeId));
        // Card created by Thinking event from agent
        showChatPage();
    });
    connect(m_tree, &TreeWidget::reviewAllRequested, this, [this] {
        if (!m_bridge->reviewAll()) return;
        m_chat->appendUserMessage(tr("严格审视完整的项目树。"));
        // Card created by Thinking event from agent
        showChatPage();
    });
    connect(m_tree, &TreeWidget::refreshRequested, m_bridge, &RustBridge::refreshTree);
}

void MainWindow::updateRecentProjects(const QString &workspace)
{
    QSettings settings;
    QStringList recent = settings.value(QStringLiteral("recentProjects")).toStringList();
    recent.removeAll(workspace);
    recent.prepend(workspace);
    while (recent.size() > 8) recent.removeLast();
    settings.setValue(QStringLiteral("recentProjects"), recent);
}

void MainWindow::removeFromRecentProjects(const QString &workspace)
{
    QSettings settings;
    QStringList recent = settings.value(QStringLiteral("recentProjects")).toStringList();
    const QString cleaned = QDir::cleanPath(workspace);
    QStringList filtered;
    filtered.reserve(recent.size());
    for (const QString &path : recent) {
        if (QDir::cleanPath(path).compare(cleaned, Qt::CaseInsensitive) != 0) {
            filtered.append(path);
        }
    }
    settings.setValue(QStringLiteral("recentProjects"), filtered);
}

void MainWindow::populateHistory(const QJsonArray &sessions)
{
    const QString selectedId = m_sessionId;
    m_historyList->clear();
    for (const QJsonValue &value : sessions) {
        const QJsonObject session = value.toObject();
        const QString id = session.value(QStringLiteral("id")).toString();
        const QString title = session.value(QStringLiteral("title")).toString(tr("新对话"));
        const QString updatedAt = session.value(QStringLiteral("updated_at")).toString();
        QDateTime timestamp = QDateTime::fromString(updatedAt, Qt::ISODate);
        QString subtitle;
        if (timestamp.isValid()) subtitle = timestamp.toString(QStringLiteral("yyyy-MM-dd HH:mm"));
        const quint64 steps = static_cast<quint64>(session.value(QStringLiteral("steps")).toInteger());
        auto *item = new QListWidgetItem(
            subtitle.isEmpty()
                ? QStringLiteral("%1 · %2 步").arg(title).arg(steps)
                : QStringLiteral("%1\n%2 · %3 步").arg(title, subtitle).arg(steps),
            m_historyList);
        item->setData(Qt::UserRole, id);
        if (session.value(QStringLiteral("current")).toBool() || id == selectedId) {
            item->setSelected(true);
            m_historyList->setCurrentItem(item);
        }
    }
    const QString query = m_historySearch->text();
    if (!query.isEmpty()) {
        for (int row = 0; row < m_historyList->count(); ++row)
            m_historyList->item(row)->setHidden(
                !m_historyList->item(row)->text().contains(query, Qt::CaseInsensitive));
    }
}

void MainWindow::updateActiveNodeBanner(const QJsonObject &tree)
{
    const QJsonArray nodes = tree.value(QStringLiteral("nodes")).toArray();
    for (const QJsonValue &value : nodes) {
        const QJsonObject node = value.toObject();
        if (node.value(QStringLiteral("status")).toString() != QStringLiteral("in_progress")) continue;
        const QString title = node.value(QStringLiteral("title")).toString();
        QStringList files;
        for (const QJsonValue &file : node.value(QStringLiteral("target_files")).toArray())
            files.append(file.toString());
        const QString owner = node.value(QStringLiteral("last_modified_by")).toString();
        m_activeNodeLabel->setText(
            files.isEmpty()
                ? tr("活跃节点：%1（%2）").arg(title, owner)
                : tr("活跃节点：%1（%2）· %3").arg(title, owner, files.join(QStringLiteral(", "))));
        return;
    }
    m_activeNodeLabel->setText(tr("暂无活跃节点 —— 让 AI 构建项目树，或选择一个节点继续。"));
}

void MainWindow::applyResponsiveLayout()
{
    const bool wide = width() >= kWideLayoutThreshold;
    if (wide) {
        const bool showSideTree =
            m_currentPage == ChatPage && m_treePanelVisible;
        if (showSideTree) {
            reparentTreeToSidePanel();
            m_treeSideHost->setMinimumWidth(280);
            m_treeSideHost->setMaximumWidth(kTreeSidePanelWidth);
            m_treeSideHost->show();
            m_chatSplitter->setSizes(
                {qMax(1, width() - kTreeSidePanelWidth), kTreeSidePanelWidth});
        } else {
            m_treeSideHost->hide();
            m_chatSplitter->setSizes({qMax(1, width()), 0});
            if (m_currentPage == TreePage) reparentTreeToFullPage();
        }
        m_treeNav->setVisible(true);
    } else {
        m_treeSideHost->hide();
        m_treeNav->setVisible(true);
        m_chatSplitter->setSizes({qMax(1, width()), 0});
        if (m_currentPage == TreePage) reparentTreeToFullPage();
    }
    m_historyPanel->setFixedWidth(width() < 900 ? qMin(width() - 24, 280) : 300);
    const int treeWidth =
        m_currentPage == ChatPage && wide && m_treePanelVisible
            ? kTreeSidePanelWidth
            : width();
    m_tree->applyWidth(treeWidth);
}

void MainWindow::updateNavStates()
{
    // History is a side panel, not a page — keep Chat highlighted while still in chat.
    m_chatNav->setChecked(m_currentPage == ChatPage);
    m_treeNav->setChecked(
        m_currentPage == TreePage
        || (m_currentPage == ChatPage && m_treePanelVisible));
    m_appsNav->setChecked(m_currentPage == AppsPage);
    m_historyNav->setChecked(m_historyVisible);
}

void MainWindow::persistSessionId() const
{
    QSettings settings;
    settings.beginGroup(QStringLiteral("workspaces/") + workspaceKey(m_workspace));
    settings.setValue(QStringLiteral("sessionId"), m_sessionId);
    settings.endGroup();
}

void MainWindow::retranslateUi()
{
    m_chatNav->setText(tr("对话"));
    m_treeNav->setText(tr("项目树"));
    m_appsNav->setText(tr("预览"));
    m_historyNav->setText(tr("历史"));
    m_activeNodeLabel->setText(tr("暂无活跃节点"));
    setWindowTitle(tr("MoonCoding"));
}

void MainWindow::reparentTreeToSidePanel()
{
    if (m_tree->parentWidget() == m_treeSideHost) return;
    if (m_tree->parentWidget() && m_tree->parentWidget()->layout())
        m_tree->parentWidget()->layout()->removeWidget(m_tree);
    m_tree->setParent(m_treeSideHost);
    if (auto *l = m_treeSideHost->layout()) l->addWidget(m_tree);
    m_tree->show();
    if (auto *h = m_treePage->findChild<QLabel *>(QStringLiteral("panelHint"))) h->show();
}

void MainWindow::reparentTreeToFullPage()
{
    if (m_tree->parentWidget() == m_treePage) return;
    if (m_tree->parentWidget() && m_tree->parentWidget()->layout())
        m_tree->parentWidget()->layout()->removeWidget(m_tree);
    m_tree->setParent(m_treePage);
    if (auto *l = qobject_cast<QVBoxLayout *>(m_treePage->layout())) {
        if (auto *h = m_treePage->findChild<QLabel *>(QStringLiteral("panelHint"))) h->hide();
        l->addWidget(m_tree, 1);
    }
    m_tree->show();
}
