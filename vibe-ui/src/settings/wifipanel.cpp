#include "wifipanel.h"

#include "boardnetrecover.h"

#include <QApplication>
#include <QDir>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QProcess>
#include <QPushButton>
#include <QRegularExpression>
#include <QSize>
#include <QSizePolicy>
#include <QThread>
#include <QTimer>
#include <QVBoxLayout>

namespace {

QString runCmdStatic(const QStringList &args, int timeoutMs = 1200)
{
    QProcess p;
    p.start(args.value(0), args.mid(1));
    if (!p.waitForStarted(400)) {
        return QStringLiteral("ERROR: start failed");
    }
    if (!p.waitForFinished(timeoutMs)) {
        p.kill();
        p.waitForFinished(200);
        return QStringLiteral("ERROR: timeout");
    }
    return QString::fromUtf8(p.readAllStandardOutput() + p.readAllStandardError()).trimmed();
}

QString wpaCliStatic(const QStringList &args, int timeoutMs = 1200)
{
    QStringList full{QStringLiteral("wpa_cli"), QStringLiteral("-i"), QStringLiteral("wlan0")};
    full.append(args);
    return runCmdStatic(full, timeoutMs);
}

bool wifiIfaceExistsStatic()
{
    return QFileInfo::exists(QStringLiteral("/sys/class/net/wlan0"));
}

bool ensureSupplicantStatic()
{
    if (!wifiIfaceExistsStatic()) {
        return false;
    }
    QDir().mkpath(QStringLiteral("/var/run/wpa_supplicant"));
    runCmdStatic({QStringLiteral("ip"), QStringLiteral("link"), QStringLiteral("set"),
                  QStringLiteral("wlan0"), QStringLiteral("up")},
                 800);
    if (!QFileInfo::exists(QStringLiteral("/var/run/wpa_supplicant/wlan0"))) {
        runCmdStatic({QStringLiteral("wpa_supplicant"), QStringLiteral("-B"), QStringLiteral("-i"),
                      QStringLiteral("wlan0"), QStringLiteral("-c"),
                      QStringLiteral("/etc/wpa_supplicant.conf")},
                     1500);
    }
    return wifiIfaceExistsStatic()
        && QFileInfo::exists(QStringLiteral("/var/run/wpa_supplicant/wlan0"));
}

} // namespace

WifiStatusInfo queryWifiStatus()
{
    WifiStatusInfo info;
#ifndef Q_OS_UNIX
    info.summary = QObject::tr("桌面");
    info.connected = true;
    info.chipPresent = true;
    info.interfaceUp = true;
    return info;
#else
    // Read-only + short timeouts. Must never start wpa or hang the UI timer.
    info.chipPresent = wifiIfaceExistsStatic();
    if (!info.chipPresent) {
        info.summary = QObject::tr("无WiFi");
        info.detail = QObject::tr(
            "wlan0 不存在：AIC8800 模组未枚举（常见于 USB EMI 掉线）。"
            "请点「重新初始化」，仍不行则断电重启板子。");
        info.state = QStringLiteral("NO_IFACE");
        return info;
    }

    info.interfaceUp = true;
    if (!QFileInfo::exists(QStringLiteral("/var/run/wpa_supplicant/wlan0"))) {
        info.summary = QObject::tr("WiFi 异常");
        info.detail = QObject::tr("wlan0 在，但 wpa_supplicant 未就绪。");
        info.state = QStringLiteral("NO_WPA");
        return info;
    }

    const QString status = wpaCliStatic({QStringLiteral("status")}, 800);
    for (const QString &line : status.split(QLatin1Char('\n'))) {
        if (line.startsWith(QLatin1String("ssid="))) {
            info.ssid = line.mid(5);
        } else if (line.startsWith(QLatin1String("wpa_state="))) {
            info.state = line.mid(10);
        } else if (line.startsWith(QLatin1String("ip_address="))) {
            info.ip = line.mid(11);
        }
    }
    if (info.ip.isEmpty()) {
        const QString addr = runCmdStatic(
            {QStringLiteral("ip"), QStringLiteral("-4"), QStringLiteral("-o"), QStringLiteral("addr"),
             QStringLiteral("show"), QStringLiteral("dev"), QStringLiteral("wlan0")},
            800);
        const auto m = QRegularExpression(QStringLiteral("inet\\s+([\\d.]+)")).match(addr);
        if (m.hasMatch()) {
            info.ip = m.captured(1);
        }
    }
    if (info.state == QLatin1String("INTERFACE_DISABLED")) {
        info.summary = QObject::tr("WiFi 禁用");
        info.detail = QObject::tr("接口被禁用，尝试「重新初始化」或重启。");
        return info;
    }
    info.connected = !info.ssid.isEmpty()
        && (info.state == QLatin1String("COMPLETED") || !info.ip.isEmpty());
    if (info.connected) {
        info.summary = info.ssid;
        if (info.summary.size() > 12) {
            info.summary = info.summary.left(11) + QChar(0x2026);
        }
    } else if (info.state == QLatin1String("SCANNING")) {
        info.summary = QObject::tr("扫描中");
    } else {
        info.summary = QObject::tr("未连接");
    }
    return info;
#endif
}

WifiPanel::WifiPanel(QWidget *parent)
    : QWidget(parent)
{
    setObjectName(QStringLiteral("wifiPanel"));
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(12, 12, 12, 12);
    root->setSpacing(10);

    m_statusLabel = new QLabel(tr("WiFi 状态：检测中…"), this);
    m_statusLabel->setObjectName(QStringLiteral("mutedLabel"));
    m_statusLabel->setWordWrap(true);

    m_networkList = new QListWidget(this);
    m_networkList->setObjectName(QStringLiteral("wifiNetworkList"));
    m_networkList->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    m_passwordEdit = new QLineEdit(this);
    m_passwordEdit->setEchoMode(QLineEdit::Password);
    m_passwordEdit->setPlaceholderText(tr("WiFi 密码（开放网络可留空）"));
    m_passwordEdit->setMinimumHeight(44);

    auto *row = new QHBoxLayout;
    m_recoverBtn = new QPushButton(tr("一键恢复网络"), this);
    m_reinitBtn = new QPushButton(tr("重新初始化"), this);
    m_scanBtn = new QPushButton(tr("扫描"), this);
    m_connectBtn = new QPushButton(tr("连接"), this);
    m_disconnectBtn = new QPushButton(tr("断开"), this);
    m_recoverBtn->setObjectName(QStringLiteral("primaryButton"));
    m_connectBtn->setObjectName(QStringLiteral("primaryButton"));
    m_recoverBtn->setMinimumHeight(48);
    m_reinitBtn->setMinimumHeight(44);
    m_scanBtn->setMinimumHeight(44);
    m_connectBtn->setMinimumHeight(44);
    m_disconnectBtn->setMinimumHeight(44);

    auto *recoverRow = new QHBoxLayout;
    recoverRow->addWidget(m_recoverBtn, 1);

    row->addWidget(m_reinitBtn);
    row->addWidget(m_scanBtn);
    row->addWidget(m_connectBtn, 1);
    row->addWidget(m_disconnectBtn);

    root->addWidget(m_statusLabel);
    root->addLayout(recoverRow);
    root->addWidget(m_networkList, 1);
    root->addWidget(m_passwordEdit);
    root->addLayout(row);

    connect(m_recoverBtn, &QPushButton::clicked, this, &WifiPanel::recoverNetwork);
    connect(m_reinitBtn, &QPushButton::clicked, this, &WifiPanel::reinitWifi);
    connect(m_scanBtn, &QPushButton::clicked, this, &WifiPanel::scanNetworks);
    connect(m_connectBtn, &QPushButton::clicked, this, &WifiPanel::connectSelected);
    connect(m_disconnectBtn, &QPushButton::clicked, this, &WifiPanel::disconnectWifi);

    m_recoverBtnLabel = m_recoverBtn->text();
    m_recover = new BoardNetRecover(this);
    connect(m_recover, &BoardNetRecover::finished, this, [this](bool ok, const QString &log) {
        setBusyUi(false, ok
            ? tr("网络：恢复成功，外网可达。\n%1").arg(log.right(350))
            : tr("网络：恢复后仍不可用。可再点一次，或断电重启。\n%1").arg(log.right(350)));
        m_recoverBtn->setText(m_recoverBtnLabel);
        refreshStatus();
        emit recoverFinished(ok, log);
    });

    refreshStatus();
}

void WifiPanel::paintFeedbackNow()
{
    // Avoid QApplication::processEvents — re-entrancy freezes/crashes linuxfb UI.
    if (m_statusLabel) {
        m_statusLabel->update();
    }
    if (m_recoverBtn) {
        m_recoverBtn->update();
    }
    repaint();
}

void WifiPanel::setBusyUi(bool busy, const QString &statusText)
{
    m_statusLabel->setText(statusText);
    m_recoverBtn->setEnabled(!busy);
    m_reinitBtn->setEnabled(!busy);
    m_scanBtn->setEnabled(!busy);
    m_connectBtn->setEnabled(!busy);
    m_disconnectBtn->setEnabled(!busy);
    m_networkList->setEnabled(!busy);
    m_passwordEdit->setEnabled(!busy);
    paintFeedbackNow();
}

QString WifiPanel::runCmd(const QStringList &args, int timeoutMs) const
{
    return runCmdStatic(args, timeoutMs);
}

QString WifiPanel::wpaCli(const QStringList &args, int timeoutMs) const
{
    return wpaCliStatic(args, timeoutMs);
}

bool WifiPanel::wifiIfaceExists() const
{
    return wifiIfaceExistsStatic();
}

bool WifiPanel::ensureSupplicant()
{
    return ensureSupplicantStatic();
}

void WifiPanel::refreshStatus()
{
    const WifiStatusInfo info = queryWifiStatus();
    if (!info.chipPresent) {
        m_statusLabel->setText(tr("WiFi：模组未就绪\n%1").arg(info.detail));
    } else if (!info.interfaceUp) {
        m_statusLabel->setText(tr("WiFi：接口异常\n%1").arg(info.detail));
    } else if (info.state == QLatin1String("INTERFACE_DISABLED")) {
        m_statusLabel->setText(tr("WiFi：接口已禁用\n%1").arg(info.detail));
    } else if (!info.connected) {
        m_statusLabel->setText(tr("WiFi：未连接（%1）")
                                   .arg(info.state.isEmpty() ? tr("未知") : info.state));
    } else {
        m_statusLabel->setText(tr("WiFi：已连接 %1%2")
                                   .arg(info.ssid,
                                        info.ip.isEmpty() ? QString()
                                                          : tr(" · IP %1").arg(info.ip)));
    }
    emit statusChanged(info);
}

void WifiPanel::reinitWifi()
{
    if (m_opBusy) {
        return;
    }
    m_opBusy = true;
    setBusyUi(true, tr("WiFi：已收到点击，正在重新初始化模组…"));
    m_networkList->clear();
    QTimer::singleShot(0, this, [this] {
        runCmd({QStringLiteral("killall"), QStringLiteral("-q"), QStringLiteral("wpa_supplicant")}, 1500);
        if (QFileInfo::exists(QStringLiteral("/usr/bin/wifibt-init.sh"))) {
            runCmd({QStringLiteral("sh"), QStringLiteral("/usr/bin/wifibt-init.sh")}, 12000);
        }
        if (!wifiIfaceExists()) {
            runCmd({QStringLiteral("rmmod"), QStringLiteral("aic8800_fdrv")}, 3000);
            runCmd({QStringLiteral("rmmod"), QStringLiteral("aic_load_fw")}, 3000);
            runCmd({QStringLiteral("insmod"), QStringLiteral("/lib/modules/aic_load_fw.ko")}, 3000);
            runCmd({QStringLiteral("insmod"), QStringLiteral("/lib/modules/aic8800_fdrv.ko")}, 3000);
            if (QFileInfo::exists(QStringLiteral("/usr/bin/wifibt-init.sh"))) {
                runCmd({QStringLiteral("sh"), QStringLiteral("/usr/bin/wifibt-init.sh")}, 12000);
            }
        }
        if (!wifiIfaceExists()) {
            m_opBusy = false;
            setBusyUi(false, tr("WiFi：仍无 wlan0。请断电重启板子后再试。"));
            refreshStatus();
            return;
        }
        ensureSupplicant();
        m_opBusy = false;
        setBusyUi(false, tr("WiFi：模组已恢复，可点扫描"));
        refreshStatus();
    });
}

void WifiPanel::recoverNetwork()
{
    if (m_recover && m_recover->isRunning()) {
        m_statusLabel->setText(tr("网络：恢复已在进行中，请稍候…"));
        paintFeedbackNow();
        return;
    }
    m_recoverBtn->setText(tr("恢复中…"));
    setBusyUi(true, tr("网络：已收到点击，正在后台恢复…"));
    m_recover->start();
}

void WifiPanel::scanNetworks()
{
    if (m_opBusy) {
        return;
    }
    m_opBusy = true;
    setBusyUi(true, tr("WiFi：已收到点击，正在扫描…"));
    QTimer::singleShot(0, this, [this] {
        if (!wifiIfaceExists()) {
            m_opBusy = false;
            setBusyUi(false, tr("WiFi：无法扫描 — wlan0 不存在。请先点「重新初始化」。"));
            return;
        }
        if (!ensureSupplicant()) {
            m_opBusy = false;
            setBusyUi(false, tr("WiFi：无法启动 wpa_supplicant"));
            return;
        }
        const QString scanOut = wpaCli({QStringLiteral("scan")}, 1500);
        if (scanOut.contains(QLatin1String("FAIL"))) {
            m_opBusy = false;
            setBusyUi(false, tr("WiFi：scan 失败（%1）").arg(scanOut));
            refreshStatus();
            return;
        }
        m_statusLabel->setText(tr("WiFi：扫描中，请稍候…"));
        paintFeedbackNow();
        QTimer::singleShot(2000, this, [this] {
            const QString results = wpaCli({QStringLiteral("scan_results")}, 1500);
            m_networkList->clear();
            const QStringList lines = results.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
            for (int i = 1; i < lines.size(); ++i) {
                const QStringList cols = lines.at(i).split(QLatin1Char('\t'));
                if (cols.size() < 5) {
                    continue;
                }
                const QString ssid = cols.at(4).trimmed();
                if (ssid.isEmpty()) {
                    continue;
                }
                const QString rssi = cols.at(2);
                auto *item = new QListWidgetItem(tr("%1  (%2 dBm)").arg(ssid, rssi), m_networkList);
                item->setData(Qt::UserRole, ssid);
                item->setSizeHint(QSize(0, 48));
            }
            m_opBusy = false;
            if (m_networkList->count() == 0) {
                setBusyUi(false, tr("WiFi：未扫到网络（可再点扫描）"));
            } else {
                setBusyUi(false, tr("WiFi：扫到 %1 个网络").arg(m_networkList->count()));
            }
            refreshStatus();
        });
    });
}

void WifiPanel::connectSelected()
{
    auto *item = m_networkList->currentItem();
    if (!item) {
        m_statusLabel->setText(tr("WiFi：请先选择网络"));
        paintFeedbackNow();
        return;
    }
    if (!wifiIfaceExists() || !ensureSupplicant()) {
        m_statusLabel->setText(tr("WiFi：模组未就绪，无法连接"));
        paintFeedbackNow();
        return;
    }
    const QString ssid = item->data(Qt::UserRole).toString();
    const QString pass = m_passwordEdit->text();
    setBusyUi(true, tr("WiFi：已收到点击，正在连接 %1…").arg(ssid));

    const QString addOut = wpaCli({QStringLiteral("add_network")});
    bool ok = false;
    const int netId = addOut.trimmed().toInt(&ok);
    if (!ok) {
        setBusyUi(false, tr("WiFi：add_network 失败：%1").arg(addOut));
        return;
    }
    wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("ssid"),
            QStringLiteral("\"%1\"").arg(ssid)});
    if (pass.isEmpty()) {
        wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("key_mgmt"),
                QStringLiteral("NONE")});
    } else {
        wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("psk"),
                QStringLiteral("\"%1\"").arg(pass)});
    }
    wpaCli({QStringLiteral("enable_network"), QString::number(netId)});
    wpaCli({QStringLiteral("select_network"), QString::number(netId)});
    wpaCli({QStringLiteral("save_config")});
    m_statusLabel->setText(tr("WiFi：已关联，正在获取 IP…"));
    paintFeedbackNow();
    runCmd({QStringLiteral("ip"), QStringLiteral("-4"), QStringLiteral("addr"),
            QStringLiteral("flush"), QStringLiteral("dev"), QStringLiteral("wlan0")},
           3000);
    runCmd({QStringLiteral("udhcpc"), QStringLiteral("-i"), QStringLiteral("wlan0"),
            QStringLiteral("-n"), QStringLiteral("-q"), QStringLiteral("-t"), QStringLiteral("8"),
            QStringLiteral("-T"), QStringLiteral("3")},
           30000);
    // Keep renewing — one-shot leases go stale on AIC8800/home APs.
    runCmd({QStringLiteral("sh"), QStringLiteral("-c"),
            QStringLiteral(
                "ps | grep -q '[u]dhcpc -i wlan0' || udhcpc -i wlan0 -b")},
           3000);
    setBusyUi(false, tr("WiFi：连接流程结束"));
    refreshStatus();
}

void WifiPanel::disconnectWifi()
{
    if (!wifiIfaceExists() || !ensureSupplicant()) {
        return;
    }
    wpaCli({QStringLiteral("disconnect")});
    refreshStatus();
}
