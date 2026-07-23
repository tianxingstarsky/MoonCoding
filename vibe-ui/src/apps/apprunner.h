#pragma once

#include <QObject>
#include <QString>

/// Simple helper that forwards a CLI app run request to the chat input.
///
/// When the user clicks "Run" on a CLI app, AppRunner emits a signal that
/// MainWindow connects to its submitMessage slot, effectively pasting the
/// command into the chat so the AI agent can execute it through the
/// verify_command tool.
class AppRunner final : public QObject
{
    Q_OBJECT

public:
    explicit AppRunner(QObject *parent = nullptr);

    void requestRun(const QString &appName, const QString &command);

signals:
    /// Emitted when the user wants to run a CLI app. The receiver should
    /// submit this as a message to the AI agent.
    void runRequested(const QString &prompt);
};
