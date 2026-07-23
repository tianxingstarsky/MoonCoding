#pragma once

#include <QByteArray>
#include <QObject>
#include <QString>

class QProcess;

/// Async board network recovery (WiFi DHCP + public DNS + clock).
/// Never blocks the UI thread beyond a brief setup.
class BoardNetRecover : public QObject
{
    Q_OBJECT

public:
    explicit BoardNetRecover(QObject *parent = nullptr);
    ~BoardNetRecover() override;

    bool isRunning() const;
    void start();

signals:
    void finished(bool ok, const QString &log);

private:
    void finishWith(bool ok, const QString &log);
    void onProcessFinished();
    void onProcessError();

    QProcess *m_proc = nullptr;
    QString m_scriptPath;
    QString m_log;
    bool m_running = false;
};

QByteArray boardNetRecoverScriptBytes();
bool boardNetPingInternet(int timeoutMs = 2500);
