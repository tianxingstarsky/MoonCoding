#include "wifipanel.h"

#include "boardnetrecover.h"

#include <QAbstractButton>
#include <QApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMessageBox>
#include <QProcess>
#include <QProgressBar>
#include <QPushButton>
#include <QRegularExpression>
#include <QSize>
#include <QSizePolicy>
#include <QTimer>
#include <QVBoxLayout>

namespace {

constexpr int kRoleSsid = Qt::UserRole;
constexpr int kRoleNeedsPassword = Qt::UserRole + 1;
constexpr int kRoleFlags = Qt::UserRole + 2;

bool wifiFlagsNeedPassword(const QString &flags)
{
    const QString u = flags.toUpper();
    // Open APs usually only advertise ESS/WPS. Anything with WPA/RSN/WEP/PSK/SAE needs a key.
    return u.contains(QLatin1String("WPA"))
        || u.contains(QLatin1String("RSN"))
        || u.contains(QLatin1String("WEP"))
        || u.contains(QLatin1String("PSK"))
        || u.contains(QLatin1String("SAE"))
        || u.contains(QLatin1String("EAP"));
}

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

QString parseWpaState(const QString &status)
{
    for (const QString &line : status.split(QLatin1Char('\n'))) {
        if (line.startsWith(QLatin1String("wpa_state="))) {
            return line.mid(10).trimmed();
        }
    }
    return {};
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

    m_progressLabel = new QLabel(this);
    m_progressLabel->setObjectName(QStringLiteral("settingsSection"));
    m_progressLabel->setWordWrap(true);
    m_progressLabel->setAlignment(Qt::AlignCenter);
    m_progressLabel->hide();

    m_progressBar = new QProgressBar(this);
    m_progressBar->setObjectName(QStringLiteral("wifiConnectProgress"));
    m_progressBar->setRange(0, 0); // indeterminate
    m_progressBar->setTextVisible(false);
    m_progressBar->setMinimumHeight(18);
    m_progressBar->hide();

    m_networkList = new QListWidget(this);
    m_networkList->setObjectName(QStringLiteral("wifiNetworkList"));
    m_networkList->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    auto *hint = new QLabel(
        tr("选中网络后点「连接」→ 确认 → 若需密码再输入。开放网络不会要密码。"),
        this);
    hint->setObjectName(QStringLiteral("mutedLabel"));
    hint->setWordWrap(true);

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
    root->addWidget(m_progressLabel);
    root->addWidget(m_progressBar);
    root->addWidget(m_networkList, 1);
    root->addWidget(hint);
    root->addLayout(row);

    m_animTimer = new QTimer(this);
    m_animTimer->setInterval(450);
    connect(m_animTimer, &QTimer::timeout, this, &WifiPanel::tickConnectAnimation);

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
    if (m_progressLabel) {
        m_progressLabel->update();
    }
    if (m_progressBar) {
        m_progressBar->update();
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
    paintFeedbackNow();
}

void WifiPanel::setConnectProgress(bool visible, const QString &phaseText)
{
    m_progressLabel->setVisible(visible);
    m_progressBar->setVisible(visible);
    if (visible) {
        m_animBaseText = phaseText;
        m_progressLabel->setText(phaseText);
        m_progressBar->setRange(0, 0);
    } else {
        m_animBaseText.clear();
        m_progressLabel->clear();
    }
    paintFeedbackNow();
}

void WifiPanel::startConnectAnimation(const QString &ssid)
{
    m_pendingSsid = ssid;
    m_animTick = 0;
    setConnectProgress(true, tr("正在连接 %1").arg(ssid));
    m_animTimer->start();
}

void WifiPanel::stopConnectAnimation()
{
    m_animTimer->stop();
    setConnectProgress(false, QString());
    m_pendingSsid.clear();
    m_connectPhase = ConnectPhase::Idle;
}

void WifiPanel::tickConnectAnimation()
{
    ++m_animTick;
    static const char *const kDots[] = {"", ".", "..", "..."};
    const QString dots = QString::fromLatin1(kDots[m_animTick % 4]);
    QString phase;
    switch (m_connectPhase) {
    case ConnectPhase::Configuring:
        phase = tr("正在配置 %1").arg(m_pendingSsid);
        break;
    case ConnectPhase::Associating:
        phase = tr("正在关联 %1（鉴权中）").arg(m_pendingSsid);
        break;
    case ConnectPhase::GettingIp:
        phase = tr("已关联，正在获取 IP");
        break;
    default:
        phase = m_animBaseText.isEmpty() ? tr("连接中") : m_animBaseText;
        break;
    }
    m_progressLabel->setText(phase + dots);
    m_statusLabel->setText(tr("WiFi：%1%2").arg(phase, dots));
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
    if (m_connectPhase != ConnectPhase::Idle && m_connectPhase != ConnectPhase::Done
        && m_connectPhase != ConnectPhase::Failed) {
        emit statusChanged(info);
        return;
    }
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
    setConnectProgress(true, tr("正在扫描附近网络"));
    m_animBaseText = tr("正在扫描附近网络");
    m_animTimer->start();
    QTimer::singleShot(0, this, [this] {
        if (!wifiIfaceExists()) {
            m_opBusy = false;
            stopConnectAnimation();
            setBusyUi(false, tr("WiFi：无法扫描 — wlan0 不存在。请先点「重新初始化」。"));
            return;
        }
        if (!ensureSupplicant()) {
            m_opBusy = false;
            stopConnectAnimation();
            setBusyUi(false, tr("WiFi：无法启动 wpa_supplicant"));
            return;
        }
        const QString scanOut = wpaCli({QStringLiteral("scan")}, 1500);
        if (scanOut.contains(QLatin1String("FAIL"))) {
            m_opBusy = false;
            stopConnectAnimation();
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
                const QString flags = cols.at(3);
                const bool needsPass = wifiFlagsNeedPassword(flags);
                const QString lock = needsPass ? tr("[密]") : tr("[开]");
                auto *item = new QListWidgetItem(
                    tr("%1 %2  (%3 dBm)").arg(lock, ssid, rssi), m_networkList);
                item->setData(kRoleSsid, ssid);
                item->setData(kRoleNeedsPassword, needsPass);
                item->setData(kRoleFlags, flags);
                item->setSizeHint(QSize(0, 52));
            }
            m_opBusy = false;
            stopConnectAnimation();
            if (m_networkList->count() == 0) {
                setBusyUi(false, tr("WiFi：未扫到网络（可再点扫描）"));
            } else {
                setBusyUi(false, tr("WiFi：扫到 %1 个网络（含安全类型）").arg(m_networkList->count()));
            }
            refreshStatus();
        });
    });
}

bool WifiPanel::confirmConnect(const QString &ssid, bool needsPassword) const
{
    QMessageBox box(const_cast<WifiPanel *>(this));
    box.setWindowTitle(tr("确认连接"));
    box.setIcon(QMessageBox::Question);
    box.setText(tr("连接到「%1」？").arg(ssid));
    box.setInformativeText(needsPassword
                               ? tr("该网络已加密，确认后将要求输入密码。")
                               : tr("该网络为开放网络，确认后直接连接，不需要密码。"));
    auto *yes = box.addButton(tr("确认连接"), QMessageBox::AcceptRole);
    box.addButton(tr("取消"), QMessageBox::RejectRole);
    yes->setMinimumHeight(48);
    for (QAbstractButton *b : box.buttons()) {
        if (b) {
            b->setMinimumHeight(48);
        }
    }
    box.exec();
    return box.clickedButton() == yes;
}

QString WifiPanel::promptPassword(const QString &ssid) const
{
    QDialog dlg(const_cast<WifiPanel *>(this));
    dlg.setWindowTitle(tr("输入 WiFi 密码"));
    dlg.setModal(true);
    dlg.setMinimumWidth(qBound(280, width() - 40, 520));

    auto *lay = new QVBoxLayout(&dlg);
    lay->setContentsMargins(16, 16, 16, 16);
    lay->setSpacing(12);

    auto *msg = new QLabel(tr("「%1」需要密码才能连接。").arg(ssid), &dlg);
    msg->setWordWrap(true);
    msg->setObjectName(QStringLiteral("mutedLabel"));

    auto *edit = new QLineEdit(&dlg);
    edit->setEchoMode(QLineEdit::Password);
    edit->setPlaceholderText(tr("请输入 WiFi 密码"));
    edit->setMinimumHeight(48);
    edit->setFocus();

    auto *show = new QPushButton(tr("显示密码"), &dlg);
    show->setCheckable(true);
    show->setMinimumHeight(40);
    connect(show, &QPushButton::toggled, &dlg, [edit](bool on) {
        edit->setEchoMode(on ? QLineEdit::Normal : QLineEdit::Password);
    });

    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dlg);
    buttons->button(QDialogButtonBox::Ok)->setText(tr("开始连接"));
    buttons->button(QDialogButtonBox::Cancel)->setText(tr("取消"));
    buttons->button(QDialogButtonBox::Ok)->setMinimumHeight(48);
    buttons->button(QDialogButtonBox::Cancel)->setMinimumHeight(48);
    buttons->button(QDialogButtonBox::Ok)->setObjectName(QStringLiteral("primaryButton"));

    lay->addWidget(msg);
    lay->addWidget(edit);
    lay->addWidget(show);
    lay->addWidget(buttons);

    connect(buttons, &QDialogButtonBox::accepted, &dlg, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dlg, &QDialog::reject);

    if (dlg.exec() != QDialog::Accepted) {
        return {};
    }
    return edit->text();
}

void WifiPanel::connectSelected()
{
    if (m_opBusy) {
        return;
    }
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

    const QString ssid = item->data(kRoleSsid).toString();
    const bool needsPassword = item->data(kRoleNeedsPassword).toBool();
    if (ssid.isEmpty()) {
        m_statusLabel->setText(tr("WiFi：无效的网络名称"));
        return;
    }

    if (!confirmConnect(ssid, needsPassword)) {
        m_statusLabel->setText(tr("WiFi：已取消连接"));
        paintFeedbackNow();
        return;
    }

    QString password;
    if (needsPassword) {
        password = promptPassword(ssid);
        if (password.isEmpty()) {
            m_statusLabel->setText(tr("WiFi：已取消（加密网络需要密码）"));
            paintFeedbackNow();
            return;
        }
    }

    beginConnect(ssid, password, needsPassword);
}

void WifiPanel::beginConnect(const QString &ssid, const QString &password, bool needsPassword)
{
    m_opBusy = true;
    m_assocAttempts = 0;
    m_connectPhase = ConnectPhase::Configuring;
    setBusyUi(true, tr("WiFi：正在连接 %1…").arg(ssid));
    startConnectAnimation(ssid);

    QTimer::singleShot(0, this, [this, ssid, password, needsPassword] {
        const QString addOut = wpaCli({QStringLiteral("add_network")});
        bool ok = false;
        const int netId = addOut.trimmed().toInt(&ok);
        if (!ok) {
            finishConnect(false, tr("WiFi：add_network 失败：%1").arg(addOut));
            return;
        }

        wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("ssid"),
                QStringLiteral("\"%1\"").arg(ssid)});
        if (!needsPassword || password.isEmpty()) {
            wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("key_mgmt"),
                    QStringLiteral("NONE")});
        } else {
            wpaCli({QStringLiteral("set_network"), QString::number(netId), QStringLiteral("psk"),
                    QStringLiteral("\"%1\"").arg(password)});
        }
        wpaCli({QStringLiteral("enable_network"), QString::number(netId)});
        wpaCli({QStringLiteral("select_network"), QString::number(netId)});
        wpaCli({QStringLiteral("save_config")});

        m_connectPhase = ConnectPhase::Associating;
        tickConnectAnimation();
        pollAssociation(ssid, 0);
    });
}

void WifiPanel::pollAssociation(const QString &ssid, int attempt)
{
    if (!m_opBusy) {
        return;
    }
    const QString status = wpaCli({QStringLiteral("status")}, 800);
    const QString state = parseWpaState(status);
    const bool completed = state == QLatin1String("COMPLETED");
    const bool failed = state == QLatin1String("DISCONNECTED")
        || state == QLatin1String("INACTIVE")
        || state == QLatin1String("INTERFACE_DISABLED");

    if (completed) {
        m_connectPhase = ConnectPhase::GettingIp;
        tickConnectAnimation();
        runDhcp(ssid);
        return;
    }

    // Allow a few early DISCONNECTED samples while handshake starts.
    if (failed && attempt >= 3) {
        finishConnect(false,
                      tr("WiFi：关联失败（%1）。请检查密码或信号后重试。")
                          .arg(state.isEmpty() ? tr("未知") : state));
        return;
    }

    if (attempt >= 20) { // ~20s
        finishConnect(false, tr("WiFi：关联超时（%1）。可再试一次或检查密码。")
                                 .arg(state.isEmpty() ? tr("未知") : state));
        return;
    }

    QTimer::singleShot(1000, this, [this, ssid, attempt] {
        pollAssociation(ssid, attempt + 1);
    });
}

void WifiPanel::runDhcp(const QString &ssid)
{
    QTimer::singleShot(0, this, [this, ssid] {
        runCmd({QStringLiteral("ip"), QStringLiteral("-4"), QStringLiteral("addr"),
                QStringLiteral("flush"), QStringLiteral("dev"), QStringLiteral("wlan0")},
               3000);
        const QString dhcpOut = runCmd(
            {QStringLiteral("udhcpc"), QStringLiteral("-i"), QStringLiteral("wlan0"),
             QStringLiteral("-n"), QStringLiteral("-q"), QStringLiteral("-t"), QStringLiteral("8"),
             QStringLiteral("-T"), QStringLiteral("3")},
            30000);
        runCmd({QStringLiteral("sh"), QStringLiteral("-c"),
                QStringLiteral(
                    "ps | grep -q '[u]dhcpc -i wlan0' || udhcpc -i wlan0 -b")},
               3000);

        const WifiStatusInfo info = queryWifiStatus();
        if (info.connected && !info.ip.isEmpty()) {
            finishConnect(true, tr("WiFi：已连接 %1 · IP %2").arg(ssid, info.ip));
        } else if (info.connected) {
            finishConnect(true, tr("WiFi：已关联 %1，但暂未拿到 IP（可点一键恢复网络）").arg(ssid));
        } else {
            finishConnect(false,
                          tr("WiFi：DHCP 未成功。%1")
                              .arg(dhcpOut.right(160)));
        }
    });
}

void WifiPanel::finishConnect(bool ok, const QString &message)
{
    m_connectPhase = ok ? ConnectPhase::Done : ConnectPhase::Failed;
    stopConnectAnimation();
    m_opBusy = false;
    setBusyUi(false, message);
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
