#pragma once

#include <QString>
#include <QWidget>

class BoardNetRecover;
class QLabel;
class QLineEdit;
class QListWidget;
class QPushButton;

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
    bool wifiIfaceExists() const;
    bool ensureSupplicant();
    QString runCmd(const QStringList &args, int timeoutMs = 8000) const;
    QString wpaCli(const QStringList &args, int timeoutMs = 8000) const;
    void setBusyUi(bool busy, const QString &statusText);
    void paintFeedbackNow();

    QLabel *m_statusLabel = nullptr;
    QListWidget *m_networkList = nullptr;
    QLineEdit *m_passwordEdit = nullptr;
    QPushButton *m_recoverBtn = nullptr;
    QPushButton *m_scanBtn = nullptr;
    QPushButton *m_connectBtn = nullptr;
    QPushButton *m_disconnectBtn = nullptr;
    QPushButton *m_reinitBtn = nullptr;
    BoardNetRecover *m_recover = nullptr;
    QString m_recoverBtnLabel;
    bool m_opBusy = false;
};
