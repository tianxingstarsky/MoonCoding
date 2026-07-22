#include "apps/appswidget.h"
#include "languagemanager.h"
#include "mainwindow.h"
#include "opencode_antialias.h"

#include <QApplication>
#include <QCommandLineOption>
#include <QCommandLineParser>
#include <QDir>
#include <QFileInfo>
#include <QFontDatabase>
#include <QMessageBox>
#include <QScreen>
#include <QSettings>
#include <QStandardPaths>
#include <QStyleFactory>

static QString projectsRoot()
{
    const QString custom = QSettings().value(
        QStringLiteral("projects/root")).toString();
    if (!custom.isEmpty() && QFileInfo(custom).isDir()) {
        return custom;
    }
    return QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation)
        + QStringLiteral("/MoonCodingProjects");
}

static QString defaultWorkspace()
{
    QSettings s;
    const QString last = s.value(QStringLiteral("lastWorkspace")).toString();
    if (!last.isEmpty() && QFileInfo(last).isDir()) {
        return last;
    }
    const QString root = projectsRoot();
    const QDir rootDir(root);
    if (!rootDir.exists()) {
        return QString();
    }
    const QFileInfoList entries = rootDir.entryInfoList(
        QDir::Dirs | QDir::NoDotAndDotDot, QDir::Time);
    if (entries.isEmpty()) {
        return QString();
    }
    return entries.first().absoluteFilePath();
}

static void loadBoardFonts()
{
    const QString dir = qEnvironmentVariable(
        "QT_QPA_FONTDIR", QStringLiteral("/root/mooncoding/fonts"));
    // Prefer static fonts first — variable fonts are expensive to rasterize on A7.
    const QStringList files = {
        QStringLiteral("simhei.ttf"),
        QStringLiteral("NotoSansSC-Regular.otf"),
        QStringLiteral("NotoSansSC-VF.ttf"),
    };
    QString preferredFamily;
    for (const QString &name : files) {
        const QString path = dir + QLatin1Char('/') + name;
        if (!QFileInfo::exists(path)) {
            continue;
        }
        // Skip VF once we already have a static CJK face.
        if (name.contains(QLatin1String("-VF")) && !preferredFamily.isEmpty()) {
            qInfo("MoonCoding: skip variable font %s (static face already loaded)",
                  qPrintable(name));
            continue;
        }
        const int id = QFontDatabase::addApplicationFont(path);
        if (id < 0) {
            qWarning("MoonCoding: failed to load font %s", qPrintable(path));
            continue;
        }
        const QStringList families = QFontDatabase::applicationFontFamilies(id);
        qInfo("MoonCoding: loaded font %s -> %s",
              qPrintable(name),
              qPrintable(families.join(QLatin1String(", "))));
        if (!families.isEmpty()) {
            if (preferredFamily.isEmpty()) {
                preferredFamily = families.first();
            }
            QFont::insertSubstitutions(
                QStringLiteral("Noto Sans CJK SC"), families);
            QFont::insertSubstitutions(
                QStringLiteral("Noto Sans SC"), families);
            QFont::insertSubstitutions(
                QStringLiteral("sans-serif"), families);
        }
    }
    if (!preferredFamily.isEmpty()
        && (!QSettings().contains(QStringLiteral("appearance/fontFamily"))
            || QSettings().value(QStringLiteral("appearance/fontFamily")).toString()
                   .contains(QLatin1String("Segoe"))
            || QSettings().value(QStringLiteral("appearance/fontFamily")).toString()
                   .contains(QLatin1String("Noto Sans SC")))) {
        // Pin to the first static face we loaded (simhei / regular).
        QSettings().setValue(QStringLiteral("appearance/fontFamily"), preferredFamily);
    }
}

static int showBoardPortrait(MainWindow *window)
{
    QScreen *screen = QGuiApplication::primaryScreen();
    const QSize phys = screen ? screen->size() : QSize(720, 1280);
    qInfo("MoonCoding board portrait fullscreen: fb=%dx%d (native, no software rotate)",
          phys.width(), phys.height());

    // Native linuxfb path: no QGraphicsProxy / offscreen render host.
    // Touch goes straight to widgets — required for Goodix on RK3506.
    window->setMinimumSize(360, 640);
    window->showFullScreen();
    return QApplication::exec();
}

int main(int argc, char *argv[])
{
    // Chromium flags must be set before QApplication; paths resolved on second call.
    AppsWidget::prepareWebEngineEnvironment();
    QApplication application(argc, argv);
    AppsWidget::prepareWebEngineEnvironment();
    if (QStyleFactory::keys().contains(QStringLiteral("Fusion"), Qt::CaseInsensitive)) {
        application.setStyle(QStyleFactory::create(QStringLiteral("Fusion")));
    }
    opencode::applyFontConfig(application);
    QCoreApplication::setOrganizationName(QStringLiteral("MoonCoding"));
    QCoreApplication::setApplicationName(QStringLiteral("MoonCoding"));
    QCoreApplication::setApplicationVersion(QStringLiteral("0.2.0"));

    QCommandLineParser parser;
    parser.setApplicationDescription(
        QStringLiteral("Human-directed, tree-structured coding agent"));
    parser.addHelpOption();
    parser.addVersionOption();
    QCommandLineOption workspaceOption(
        {QStringLiteral("C"), QStringLiteral("workspace")},
        QStringLiteral("Project workspace directory (optional)"),
        QStringLiteral("path"));
    parser.addOption(workspaceOption);
    QCommandLineOption uiProfileOption(
        QStringLiteral("ui-profile"),
        QStringLiteral("Preview a target screen profile: 720p, 480p, or portrait"),
        QStringLiteral("profile"));
    parser.addOption(uiProfileOption);
    parser.process(application);

    QString workspace;
    if (parser.isSet(workspaceOption)) {
        const QFileInfo wi(parser.value(workspaceOption));
        if (!wi.isDir()) {
            QMessageBox::critical(
                nullptr, QStringLiteral("MoonCoding"),
                QStringLiteral("工作区目录不存在：\n%1").arg(wi.absoluteFilePath()));
            return 2;
        }
        workspace = wi.canonicalFilePath();
        if (workspace.isEmpty()) workspace = wi.absoluteFilePath();
    } else {
        workspace = defaultWorkspace();
    }
    if (!workspace.isEmpty()) {
        QSettings().setValue(QStringLiteral("lastWorkspace"), workspace);
    }

    const QString language = QSettings().value(
        QStringLiteral("appearance/language"), QStringLiteral("zh")).toString();
    LanguageManager::instance().setLanguage(language);

    const QByteArray qpa = qgetenv("QT_QPA_PLATFORM");
    const bool boardLinuxFb = qpa.startsWith("linuxfb")
        || qEnvironmentVariableIsSet("MOONCODING_BOARD");

    if (boardLinuxFb) {
        loadBoardFonts();
        application.setCursorFlashTime(0);
        // Goodix touch → Qt mouse synthesis into real widgets.
        QApplication::setAttribute(Qt::AA_SynthesizeMouseForUnhandledTouchEvents, true);
        if (!QSettings().contains(QStringLiteral("appearance/fontFamily"))
            || QSettings().value(QStringLiteral("appearance/fontFamily")).toString()
                   .contains(QLatin1String("Segoe"))) {
            QSettings().setValue(
                QStringLiteral("appearance/fontFamily"),
                QStringLiteral("SimHei"));
        }
        // Re-apply after board fonts are registered (cheaper AA/hinting).
        opencode::applyFontConfig(application);
    }

    auto *window = new MainWindow(workspace);
    const QString profile = parser.value(uiProfileOption).trimmed().toLower();

    if (boardLinuxFb) {
        return showBoardPortrait(window);
    }

    if (profile == QStringLiteral("portrait")
        || profile == QStringLiteral("720x1280")) {
        window->resize(720, 1280);
    } else if (profile == QStringLiteral("720p") || profile == QStringLiteral("1280x720")) {
        window->resize(1280, 720);
    } else if (profile == QStringLiteral("480p") || profile == QStringLiteral("800x480")) {
        window->resize(800, 480);
    }
    if (QScreen *screen = window->screen()) {
        window->move(screen->availableGeometry().center() - window->rect().center());
    }
    window->show();
    return application.exec();
}
