#pragma once

#include <QString>
#include <QWidget>

class BoardNetRecover;
class QLabel;
class QLineEdit;
class QListWidget;
class QListWidgetItem;
class QProgressBar;
class QPushButton;
class QTimer;

struct WifiStatusInfo {
    bool interfaceUp = false;
    bool chipPresent = false;
    bool connected = false;
    QString ssid;
    QString ip;
    QString state;
    QString summary;
    QString detail;
};

WifiStatusInfo queryWifiStatus();

class WifiPanel final : public QWidget
{
    Q_OBJECT

public:
    explicit WifiPanel(QWidget *parent = nullptr);

signals:
    void statusChanged(const WifiStatusInfo &info);
    void recoverFinished(bool ok, const QString &log);

public slots:
    void refreshStatus();
    void scanNetworks();
    void connectSelected();
    void disconnectWifi();
    void reinitWifi();
    void recoverNetwork();

private:
    enum class ConnectPhase {
        Idle,
        Configuring,
        Associating,
        GettingIp,
        Done,
        Failed
    };

    bool wifiIfaceExists() const;
    bool ensureSupplicant();
    QString runCmd(const QStringList &args, int timeoutMs = 8000) const;
    QString wpaCli(const QStringList &args, int timeoutMs = 8000) const;
    void setBusyUi(bool busy, const QString &statusText);
    void paintFeedbackNow();
    void setConnectProgress(bool visible, const QString &phaseText);
    void startConnectAnimation(const QString &ssid);
    void stopConnectAnimation();
    void tickConnectAnimation();
    bool confirmConnect(const QString &ssid, bool needsPassword) const;
    QString promptPassword(const QString &ssid) const;
    void beginConnect(const QString &ssid, const QString &password, bool needsPassword);
    void finishConnect(bool ok, const QString &message);
    void pollAssociation(const QString &ssid, int attempt);
    void runDhcp(const QString &ssid);

    QLabel *m_statusLabel = nullptr;
    QLabel *m_progressLabel = nullptr;
    QProgressBar *m_progressBar = nullptr;
    QListWidget *m_networkList = nullptr;
    QPushButton *m_recoverBtn = nullptr;
    QPushButton *m_scanBtn = nullptr;
    QPushButton *m_connectBtn = nullptr;
    QPushButton *m_disconnectBtn = nullptr;
    QPushButton *m_reinitBtn = nullptr;
    BoardNetRecover *m_recover = nullptr;
    QTimer *m_animTimer = nullptr;
    QString m_recoverBtnLabel;
    QString m_animBaseText;
    QString m_pendingSsid;
    int m_animTick = 0;
    int m_assocAttempts = 0;
    bool m_opBusy = false;
    ConnectPhase m_connectPhase = ConnectPhase::Idle;
};
