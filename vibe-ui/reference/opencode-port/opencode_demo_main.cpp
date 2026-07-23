// =============================================================================
// opencode_demo_main.cpp — Full opencode desktop v2 demo
//
// Creates a complete window with:
//   - Chrome-style tab bar with 3 simulated sessions + Home tab
//   - Home page with project list
//   - Session view with split layout (chat + review panel)
//   - Settings dialog accessible via button/menu
//   - Command palette (Ctrl+K)
//   - Permission prompt demo
//
// opencode desktop v2 layout: The window has a tab bar at the top,
// below which is the active view (Home page or Session view). The
// right side (or bottom) has review panels and file tree.
// =============================================================================

#include "opencode_chat_widget.h"
#include "opencode_tool_block.h"
#include "opencode_input_widget.h"
#include "opencode_composer.h"
#include "opencode_tab_bar.h"
#include "opencode_home_widget.h"
#include "opencode_session_view.h"
#include "opencode_dialog_overlay.h"
#include "opencode_settings_dialog.h"
#include "opencode_antialias.h"

#include <QApplication>
#include <QMainWindow>
#include <QVBoxLayout>
#include <QStatusBar>
#include <QTimer>
#include <QFile>
#include <QTextStream>
#include <QLabel>
#include <QPushButton>
#include <QShortcut>
#include <QStackedWidget>
#include <QMenuBar>
#include <QMenu>
#include <QAction>
#include <QDateTime>
#include <QDir>
#include <QMessageBox>

// ---------------------------------------------------------------------------
// DemoWindow — Main application window
// ---------------------------------------------------------------------------

class DemoWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit DemoWindow(QWidget* parent = nullptr)
        : QMainWindow(parent)
    {
        setWindowTitle(tr("OpenCode Desktop v2 — Qt6 Reference Demo"));
        resize(1100, 720);
        setMinimumSize(800, 480);
        setObjectName(QStringLiteral("MainWindow"));

        // Central widget.
        auto* central = new QWidget(this);
        central->setObjectName(QStringLiteral("CentralWidget"));
        auto* layout = new QVBoxLayout(central);
        layout->setContentsMargins(0, 0, 0, 0);
        layout->setSpacing(0);

        // =====================================================================
        // Tab bar — Chrome-style tabs at the top
        // =====================================================================
        m_tabBar = new OpenCodeTabBar(central);
        m_tabBar->installTabShortcuts(this);
        layout->addWidget(m_tabBar);

        // =====================================================================
        // Stacked widget — holds Home page and Session views
        // =====================================================================
        m_viewStack = new QStackedWidget(central);
        m_viewStack->setObjectName(QStringLiteral("ViewStack"));

        // Home page (index 0).
        m_homeWidget = new OpenCodeHomeWidget(m_viewStack);
        m_viewStack->addWidget(m_homeWidget);

        // Session views — created lazily when tabs are added.
        // We pre-create 2 sessions for the demo.

        layout->addWidget(m_viewStack, 1);

        // =====================================================================
        // Status bar.
        // =====================================================================
        statusBar()->setObjectName(QStringLiteral("StatusBar"));
        statusBar()->showMessage(
            tr("Ctrl+K: Command Palette  |  Ctrl+N: New Tab  |  Ctrl+,: Settings  |  Ctrl+1-9: Switch Tab"));

        setCentralWidget(central);

        // =====================================================================
        // Menu bar.
        // =====================================================================
        setupMenuBar();

        // =====================================================================
        // Keyboard shortcuts.
        // =====================================================================
        setupShortcuts();

        // =====================================================================
        // Connect signals.
        // =====================================================================
        connect(m_tabBar, &OpenCodeTabBar::currentChanged,
                this, &DemoWindow::onTabChanged);
        connect(m_tabBar, &OpenCodeTabBar::tabCloseRequested,
                this, &DemoWindow::onTabCloseRequested);
        connect(m_tabBar, &OpenCodeTabBar::newTabRequested,
                this, &DemoWindow::addNewSessionTab);

        connect(m_homeWidget, &OpenCodeHomeWidget::projectSelected,
                this, &DemoWindow::onProjectOpened);

        // =====================================================================
        // Build demo content.
        // =====================================================================
        buildDemoContent();

        // Show home page first.
        m_viewStack->setCurrentWidget(m_homeWidget);
        m_tabBar->setCurrentIndex(m_tabBar->homeTabIndex());
    }

    ~DemoWindow() override = default;

private slots:
    // -------------------------------------------------------------------
    // Tab management.
    // -------------------------------------------------------------------
    void onTabChanged(int index)
    {
        if (index < 0) return;

        if (index == m_tabBar->homeTabIndex()) {
            m_viewStack->setCurrentWidget(m_homeWidget);
            return;
        }

        // Map tab index to session view.
        if (m_tabSessions.contains(index)) {
            m_viewStack->setCurrentWidget(m_tabSessions[index]);
        }
    }

    void onTabCloseRequested(int index)
    {
        if (index == m_tabBar->homeTabIndex()) return;

        if (m_tabSessions.contains(index)) {
            auto* session = m_tabSessions[index];
            m_viewStack->removeWidget(session);
            m_tabSessions.remove(index);
            session->deleteLater();
        }

        m_tabBar->removeTab(index);

        // Update session map (indices shifted).
        QMap<int, OpenCodeSessionView*> newMap;
        for (auto it = m_tabSessions.begin(); it != m_tabSessions.end(); ++it) {
            int newIdx = it.key() > index ? it.key() - 1 : it.key();
            newMap[newIdx] = it.value();
        }
        m_tabSessions = newMap;
    }

    void addNewSessionTab()
    {
        QString title = tr("Session %1").arg(m_sessionCounter++);
        int tabIdx = m_tabBar->addSessionTab(title);

        auto* session = new OpenCodeSessionView(m_viewStack);
        session->setSessionName(title);
        m_viewStack->addWidget(session);

        m_tabSessions[tabIdx] = session;
        m_tabBar->setCurrentIndex(tabIdx);
    }

    // -------------------------------------------------------------------
    // Home page events.
    // -------------------------------------------------------------------
    void onProjectOpened(const QString& path)
    {
        QDir dir(path);
        QString name = dir.dirName();

        // Check if this project already has a tab.
        for (auto it = m_tabSessions.begin(); it != m_tabSessions.end(); ++it) {
            if (it.value()->sessionName() == name) {
                m_tabBar->setCurrentIndex(it.key());
                return;
            }
        }

        // Create new session tab for this project.
        int tabIdx = m_tabBar->addSessionTab(name, path, QStringLiteral("main"));
        m_tabBar->setTabProject(tabIdx, name, path, QStringLiteral("main"));
        m_tabBar->setTabLastOpened(tabIdx, tr("Just now"));

        auto* session = new OpenCodeSessionView(m_viewStack);
        session->setSessionName(name);
        session->setProjectPath(path);
        m_viewStack->addWidget(session);

        m_tabSessions[tabIdx] = session;
        m_tabBar->setCurrentIndex(tabIdx);

        // Add to home page.
        m_homeWidget->addProject(name, path, QDateTime::currentDateTime());
    }

    // -------------------------------------------------------------------
    // Command palette.
    // -------------------------------------------------------------------
    void openCommandPalette()
    {
        auto* palette = new CommandPalette(this);

        palette->addCommand(tr("New Tab"),           tr("Open a new session tab"),     QStringLiteral("Ctrl+N"));
        palette->addCommand(tr("Open Project"),      tr("Open a project folder"),       QStringLiteral("Ctrl+O"));
        palette->addCommand(tr("Settings"),          tr("Open settings"),               QStringLiteral("Ctrl+,"));
        palette->addCommand(tr("Toggle File Tree"),  tr("Show/hide the file tree"),     QStringLiteral("Ctrl+E"));
        palette->addCommand(tr("Toggle Review Panel"), tr("Show/hide the review panel"), QStringLiteral("Ctrl+R"));
        palette->addCommand(tr("Home"),              tr("Go to home page"),             QStringLiteral("Ctrl+H"));
        palette->addCommand(tr("Close Tab"),         tr("Close the current tab"),       QStringLiteral("Ctrl+W"));

        connect(palette, &CommandPalette::commandSelected, this, [this](const QString& name) {
            if (name == tr("New Tab"))
                addNewSessionTab();
            else if (name == tr("Open Project"))
                m_homeWidget->findChild<QPushButton*>("HomeNewProjectBtn")->click();
            else if (name == tr("Settings"))
                openSettings();
            else if (name == tr("Toggle File Tree")) {
                auto* session = currentSession();
                if (session) session->setFileTreeVisible(!session->isFileTreeVisible());
            } else if (name == tr("Toggle Review Panel")) {
                auto* session = currentSession();
                if (session) session->setReviewPanelVisible(!session->isReviewPanelVisible());
            } else if (name == tr("Home"))
                m_tabBar->setCurrentIndex(m_tabBar->homeTabIndex());
            else if (name == tr("Close Tab"))
                onTabCloseRequested(m_tabBar->currentIndex());
        });

        palette->exec();
        palette->deleteLater();
    }

    // -------------------------------------------------------------------
    // Settings dialog.
    // -------------------------------------------------------------------
    void openSettings()
    {
        auto* settings = new OpenCodeSettingsDialog(this);

        settings->addProvider(QStringLiteral("DeepSeek"),
                              QStringLiteral("https://api.deepseek.com/v1"), true);
        settings->addProvider(QStringLiteral("OpenAI"),
                              QStringLiteral("https://api.openai.com/v1"), false);
        settings->addProvider(QStringLiteral("Groq"),
                              QStringLiteral("https://api.groq.com/openai/v1"), false);

        settings->addModel(QStringLiteral("DeepSeek"), QStringLiteral("deepseek-chat"));
        settings->addModel(QStringLiteral("DeepSeek"), QStringLiteral("deepseek-reasoner"));
        settings->addModel(QStringLiteral("OpenAI"), QStringLiteral("gpt-4.1"));
        settings->addModel(QStringLiteral("OpenAI"), QStringLiteral("gpt-4.1-mini"));
        settings->addModel(QStringLiteral("Groq"), QStringLiteral("llama-4-scout"));
        settings->addModel(QStringLiteral("Groq"), QStringLiteral("mixtral-8x7b"));

        connect(settings, &OpenCodeSettingsDialog::settingsApplied, this, []() {
            // Settings would be persisted here.
        });

        settings->exec();
        settings->deleteLater();
    }

    // -------------------------------------------------------------------
    // Permission prompt demo.
    // -------------------------------------------------------------------
    void showPermissionDemo()
    {
        auto* prompt = new PermissionPrompt(this);
        prompt->setToolName(tr("bash"));
        prompt->setToolCommand(QStringLiteral("rm -rf /tmp/build-cache"));
        prompt->setToolDescription(
            tr("The agent wants to execute a shell command that modifies your filesystem. "
               "Review the command carefully before approving."));

        connect(prompt, &PermissionPrompt::approved, this, [this]() {
            statusBar()->showMessage(tr("Tool approved."), 3000);
        });
        connect(prompt, &PermissionPrompt::denied, this, [this]() {
            statusBar()->showMessage(tr("Tool denied."), 3000);
        });

        prompt->exec();
        prompt->deleteLater();
    }

    // -------------------------------------------------------------------
    // Demo simulation in a session.
    // -------------------------------------------------------------------
    void runDemoInSession(OpenCodeSessionView* session)
    {
        if (!session) return;

        auto* chat = session->chatWidget();
        auto* composer = session->composer();

        composer->setModelInfo(QStringLiteral("DeepSeek"), QStringLiteral("deepseek-chat"));
        composer->setTokenCount(12400);
        composer->setStepCount(1);

        // Simulate a few messages.
        auto* userBlock = chat->addMessage(MessageBlock::User,
            tr("Add a login form component to src/components/Login.tsx"));
        userBlock->setTimestamp(tr("just now"));
        userBlock->setRoleLabel(tr("You"));
        chat->scrollToBottom();

        // Tool: glob.
        QTimer::singleShot(600, this, [chat]() {
            auto* t = chat->addToolBlock();
            t->setToolType(OpenCodeToolBlock::Glob);
            t->setToolName(QStringLiteral("glob"));
            t->setCommand(QStringLiteral("**/*Login*"));
            t->setState(OpenCodeToolBlock::Running);

            QTimer::singleShot(700, chat, [t]() {
                t->setState(OpenCodeToolBlock::Success);
                t->setOutput(QStringLiteral(
                    "src/components/Login.tsx\n"
                    "src/components/LoginForm.test.tsx\n"
                    "src/hooks/useLogin.ts"));
                t->setExpanded(true);
            });
        });

        // Tool: read.
        QTimer::singleShot(1400, this, [chat]() {
            auto* t = chat->addToolBlock();
            t->setToolType(OpenCodeToolBlock::Read);
            t->setToolName(QStringLiteral("read"));
            t->setCommand(QStringLiteral("src/components/Login.tsx"));
            t->setState(OpenCodeToolBlock::Running);

            QTimer::singleShot(500, chat, [t]() {
                t->setState(OpenCodeToolBlock::Success);
                t->setOutput(QStringLiteral(
                    "import React from 'react';\n"
                    "import { useForm } from 'react-hook-form';\n\n"
                    "export const Login = () => {\n"
                    "  const { register, handleSubmit } = useForm();\n"
                    "  // ... 23 lines\n"
                    "};"));
            });
        });

        // Tool: write.
        QTimer::singleShot(2000, this, [chat]() {
            auto* t = chat->addToolBlock();
            t->setToolType(OpenCodeToolBlock::Write);
            t->setToolName(QStringLiteral("write"));
            t->setCommand(QStringLiteral("src/components/LoginForm.tsx"));
            t->setState(OpenCodeToolBlock::Running);

            QTimer::singleShot(800, chat, [t]() {
                t->setState(OpenCodeToolBlock::Success);
                t->setOutput(QStringLiteral(
                    "import React, { useState } from 'react';\n"
                    "import { useForm } from 'react-hook-form';\n"
                    "import { zodResolver } from '@hookform/resolvers/zod';\n"
                    "import { z } from 'zod';\n\n"
                    "const loginSchema = z.object({\n"
                    "  email: z.string().email(),\n"
                    "  password: z.string().min(8),\n"
                    "});\n\n"
                    "export const LoginForm = () => {\n"
                    "  const [loading, setLoading] = useState(false);\n"
                    "  // ...\n"
                    "};"));
            });
        });

        // AI text response (streaming).
        QTimer::singleShot(3000, this, [chat, composer]() {
            auto* assistant = chat->addMessage(MessageBlock::Assistant);
            assistant->setTimestamp(tr("streaming..."));
            assistant->setRoleLabel(tr("Assistant"));
            assistant->showStreamingCursor(true);

            QString response = tr("I've created the LoginForm component with form validation using react-hook-form and zod. The component includes email and password fields with proper validation. I also added a loading state during form submission and accessible labels for screen readers.");

            // Stream word by word.
            QStringList words = response.split(QStringLiteral(" "));
            QTimer* streamTimer = new QTimer(chat);
            int* idx = new int(0);
            QObject::connect(streamTimer, &QTimer::timeout, chat, [=]() mutable {
                if (*idx < words.size()) {
                    assistant->appendText(words[*idx] + QStringLiteral(" "));
                    (*idx)++;
                    chat->scrollToBottom();
                } else {
                    streamTimer->stop();
                    assistant->showStreamingCursor(false);
                    assistant->setTimestamp(tr("2.1s"));
                    streamTimer->deleteLater();
                    delete idx;
                }
            });
            streamTimer->start(35);
        });
    }

private:
    // -------------------------------------------------------------------
    // Current session helper.
    // -------------------------------------------------------------------
    OpenCodeSessionView* currentSession()
    {
        int idx = m_tabBar->currentIndex();
        if (m_tabSessions.contains(idx))
            return m_tabSessions[idx];
        return nullptr;
    }

    // -------------------------------------------------------------------
    // Menu bar setup.
    // -------------------------------------------------------------------
    void setupMenuBar()
    {
        auto* menuBar = this->menuBar();
        menuBar->setObjectName(QStringLiteral("MainMenuBar"));

        // File menu.
        auto* fileMenu = menuBar->addMenu(tr("&File"));

        auto* newTabAction = new QAction(tr("&New Tab"), fileMenu);
        newTabAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+N")));
        QObject::connect(newTabAction, &QAction::triggered, this, &DemoWindow::addNewSessionTab);
        fileMenu->addAction(newTabAction);

        auto* openAction = new QAction(tr("&Open Project..."), fileMenu);
        openAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+O")));
        QObject::connect(openAction, &QAction::triggered, this, [this]() {
            m_homeWidget->findChild<QPushButton*>("HomeNewProjectBtn")->click();
        });
        fileMenu->addAction(openAction);

        fileMenu->addSeparator();

        auto* settingsAction = new QAction(tr("&Settings"), fileMenu);
        settingsAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+,")));
        QObject::connect(settingsAction, &QAction::triggered, this, &DemoWindow::openSettings);
        fileMenu->addAction(settingsAction);

        fileMenu->addSeparator();

        auto* quitAction = new QAction(tr("&Quit"), fileMenu);
        quitAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+Q")));
        QObject::connect(quitAction, &QAction::triggered, this, &QMainWindow::close);
        fileMenu->addAction(quitAction);

        // View menu.
        auto* viewMenu = menuBar->addMenu(tr("&View"));

        auto* homeAction = new QAction(tr("&Home"), viewMenu);
        homeAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+H")));
        QObject::connect(homeAction, &QAction::triggered, this, [this]() {
            m_tabBar->setCurrentIndex(m_tabBar->homeTabIndex());
        });
        viewMenu->addAction(homeAction);

        auto* cmdPaletteAction = new QAction(tr("Command &Palette"), viewMenu);
        cmdPaletteAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+K")));
        QObject::connect(cmdPaletteAction, &QAction::triggered,
                         this, &DemoWindow::openCommandPalette);
        viewMenu->addAction(cmdPaletteAction);

        viewMenu->addSeparator();

        auto* toggleTreeAction = new QAction(tr("Toggle File &Tree"), viewMenu);
        toggleTreeAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+E")));
        QObject::connect(toggleTreeAction, &QAction::triggered, this, [this]() {
            auto* s = currentSession();
            if (s) s->setFileTreeVisible(!s->isFileTreeVisible());
        });
        viewMenu->addAction(toggleTreeAction);

        auto* toggleReviewAction = new QAction(tr("Toggle &Review Panel"), viewMenu);
        toggleReviewAction->setShortcut(QKeySequence(QStringLiteral("Ctrl+R")));
        QObject::connect(toggleReviewAction, &QAction::triggered, this, [this]() {
            auto* s = currentSession();
            if (s) s->setReviewPanelVisible(!s->isReviewPanelVisible());
        });
        viewMenu->addAction(toggleReviewAction);

        // Demo menu.
        auto* demoMenu = menuBar->addMenu(tr("&Demo"));

        auto* permAction = new QAction(tr("&Permission Prompt"), demoMenu);
        QObject::connect(permAction, &QAction::triggered, this, &DemoWindow::showPermissionDemo);
        demoMenu->addAction(permAction);

        auto* modelPickerAction = new QAction(tr("&Model Picker"), demoMenu);
        QObject::connect(modelPickerAction, &QAction::triggered, this, [this]() {
            auto* picker = new ModelPicker(this);
            picker->addProvider(QStringLiteral("DeepSeek"),
                                {QStringLiteral("deepseek-chat"), QStringLiteral("deepseek-reasoner")},
                                {tr("General purpose"), tr("Reasoning-focused")});
            picker->addProvider(QStringLiteral("OpenAI"),
                                {QStringLiteral("gpt-4.1"), QStringLiteral("gpt-4.1-mini")},
                                {tr("Most capable"), tr("Fast and efficient")});
            picker->addProvider(QStringLiteral("Groq"),
                                {QStringLiteral("llama-4-scout"), QStringLiteral("mixtral-8x7b")},
                                {tr("17M context"), tr("Fast inference")});
            picker->exec();
            picker->deleteLater();
        });
    }

    // -------------------------------------------------------------------
    // Global shortcuts.
    // -------------------------------------------------------------------
    void setupShortcuts()
    {
        auto* cmdPaletteShortcut = new QShortcut(QKeySequence(QStringLiteral("Ctrl+K")), this);
        connect(cmdPaletteShortcut, &QShortcut::activated, this, &DemoWindow::openCommandPalette);

        auto* settingsShortcut = new QShortcut(QKeySequence(QStringLiteral("Ctrl+,")), this);
        connect(settingsShortcut, &QShortcut::activated, this, &DemoWindow::openSettings);

        auto* homeShortcut = new QShortcut(QKeySequence(QStringLiteral("Ctrl+H")), this);
        connect(homeShortcut, &QShortcut::activated, this, [this]() {
            m_tabBar->setCurrentIndex(m_tabBar->homeTabIndex());
        });
    }

    // -------------------------------------------------------------------
    // Build demo content (pre-populated sessions + home projects).
    // -------------------------------------------------------------------
    void buildDemoContent()
    {
        // Add demo projects to home page.
        m_homeWidget->addProject(
            tr("MoonCoding"), QStringLiteral("E:/newvibecode"),
            QDateTime::currentDateTime().addDays(-1));
        m_homeWidget->addProject(
            tr("Vibe Engine"), QStringLiteral("E:/newvibecode/vibe"),
            QDateTime::currentDateTime().addDays(-3));
        m_homeWidget->addProject(
            tr("Qt Reference UI"), QStringLiteral("E:/newvibecode/vibe-ui/reference"),
            QDateTime::currentDateTime().addSecs(-3600));

        m_homeWidget->addRecentlyClosed(
            tr("Old Project"), QStringLiteral("E:/old_project"));

        // Create session 1 — "MoonCoding" with demo content.
        int tab1 = m_tabBar->addSessionTab(
            tr("MoonCoding"), QStringLiteral("E:/newvibecode"), QStringLiteral("main"));
        m_tabBar->setTabProject(tab1, tr("MoonCoding"),
                                QStringLiteral("E:/newvibecode"), QStringLiteral("main"));
        m_tabBar->setTabLastOpened(tab1, tr("Today"));
        m_tabBar->setTabHasActivity(tab1, true);

        auto* session1 = new OpenCodeSessionView(m_viewStack);
        session1->setSessionName(tr("MoonCoding"));
        session1->setProjectPath(QStringLiteral("E:/newvibecode"));
        m_viewStack->addWidget(session1);
        m_tabSessions[tab1] = session1;

        // Run demo in session 1 after a short delay.
        QTimer::singleShot(800, this, [this, session1]() {
            runDemoInSession(session1);
        });

        // Create session 2 — "Qt Reference UI" (empty).
        int tab2 = m_tabBar->addSessionTab(
            tr("Qt Reference"), QStringLiteral("E:/newvibecode/vibe-ui/reference"),
            QStringLiteral("main"));
        m_tabBar->setTabProject(tab2, tr("Qt Reference UI"),
                                QStringLiteral("E:/newvibecode/vibe-ui/reference"),
                                QStringLiteral("main"));
        m_tabBar->setTabLastOpened(tab2, tr("Today"));

        auto* session2 = new OpenCodeSessionView(m_viewStack);
        session2->setSessionName(tr("Qt Reference UI"));
        session2->setProjectPath(QStringLiteral("E:/newvibecode/vibe-ui/reference"));
        m_viewStack->addWidget(session2);
        m_tabSessions[tab2] = session2;

        // Configure models.
        auto configureComposerModels = [](OpenCodeComposer* c) {
            c->addAvailableModel(QStringLiteral("DeepSeek"), QStringLiteral("deepseek-chat"),
                                 tr("General purpose"));
            c->addAvailableModel(QStringLiteral("DeepSeek"), QStringLiteral("deepseek-reasoner"),
                                 tr("Reasoning-focused"));
            c->addAvailableModel(QStringLiteral("OpenAI"), QStringLiteral("gpt-4.1"),
                                 tr("Most capable"));
            c->addAvailableModel(QStringLiteral("OpenAI"), QStringLiteral("gpt-4.1-mini"),
                                 tr("Fast and efficient"));
            c->addAvailableModel(QStringLiteral("Groq"), QStringLiteral("llama-4-scout"),
                                 tr("17M context window"));
        };
        configureComposerModels(session1->composer());
        configureComposerModels(session2->composer());
    }

private:
    OpenCodeTabBar* m_tabBar = nullptr;
    QStackedWidget* m_viewStack = nullptr;

    OpenCodeHomeWidget* m_homeWidget = nullptr;
    QMap<int, OpenCodeSessionView*> m_tabSessions;

    int m_sessionCounter = 3; // Start after the 2 pre-built sessions.
};

// =============================================================================
// main
// =============================================================================

int main(int argc, char* argv[])
{
    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("OpenCodePortDemo"));
    app.setApplicationVersion(QStringLiteral("2.0"));
    app.setOrganizationName(QStringLiteral("MoonCoding"));

    // -----------------------------------------------------------------------
    // Apply anti-aliased fonts (openCode desktop v2 requirement).
    // -----------------------------------------------------------------------
    opencode::applyFontConfig(app);

    // -----------------------------------------------------------------------
    // Load the opencode-style QSS stylesheet.
    // -----------------------------------------------------------------------
    QStringList searchPaths = {
        QApplication::applicationDirPath() + QStringLiteral("/opencode_styles.qss"),
        QApplication::applicationDirPath() + QStringLiteral("/../opencode_styles.qss"),
        QStringLiteral("opencode_styles.qss"),
        QStringLiteral("E:/newvibecode/vibe-ui/reference/opencode-port/opencode_styles.qss"),
    };

    QFile styleFile;
    for (const auto& path : searchPaths) {
        styleFile.setFileName(path);
        if (styleFile.exists()) {
            break;
        }
    }

    if (styleFile.open(QFile::ReadOnly | QFile::Text)) {
        QTextStream ts(&styleFile);
        QString styleSheet = ts.readAll();
        app.setStyleSheet(styleSheet);
        styleFile.close();
        qInfo("Loaded opencode stylesheet v2.");
    } else {
        qWarning("Could not find opencode_styles.qss — running with default system style.");
    }

    // -----------------------------------------------------------------------
    // Create and show the demo window.
    // -----------------------------------------------------------------------
    DemoWindow window;
    window.show();

    return app.exec();
}

#include "opencode_demo_main.moc"
