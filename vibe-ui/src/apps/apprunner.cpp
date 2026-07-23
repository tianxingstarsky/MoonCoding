#include "apprunner.h"

#include <QCoreApplication>

AppRunner::AppRunner(QObject *parent)
    : QObject(parent)
{
}

void AppRunner::requestRun(const QString &appName, const QString &command)
{
    // Determine if this is a Python or CLI app from the command prefix
    const bool isPython = command.startsWith(QStringLiteral("python "));

    const QString prompt = isPython
        ? QStringLiteral(
            "Run the Python app \"%1\" by executing: %2\n"
            "Use the bash tool to run it. The Python script should output HTML "
            "that I can view in the Apps page. After running, report the output "
            "and describe what the app does.")
              .arg(appName, command)
        : QStringLiteral(
            "Run the CLI app \"%1\" by executing its command: %2\n"
            "Use the bash tool to run it and report the output.")
              .arg(appName, command);

    emit runRequested(prompt);
}
