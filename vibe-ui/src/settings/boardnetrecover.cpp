#include "boardnetrecover.h"

#include <QCoreApplication>
#include <QFile>
#include <QFileDevice>
#include <QList>
#include <QProcess>
#include <QTimer>

#include <utility>

namespace {

constexpr int kRecoverTimeoutMs = 45000;

// /proc/net/route gateway is little-endian hex (e.g. 0100A8C0 -> 192.168.0.1).
QString gatewayFromHex(const QByteArray &hex)
{
    bool ok = false;
    const quint32 le = hex.toUInt(&ok, 16);
    if (!ok || le == 0) {
        return {};
    }
    return QStringLiteral("%1.%2.%3.%4")
        .arg(le & 0xff)
        .arg((le >> 8) & 0xff)
        .arg((le >> 16) & 0xff)
        .arg((le >> 24) & 0xff);
}

bool isLoopbackIface(const QByteArray &name)
{
    return name == "lo" || name.startsWith("lo:");
}

bool isWirelessIface(const QByteArray &name)
{
    return name.startsWith("wlan") || name.startsWith("wl");
}

struct DefaultRoute {
    QByteArray iface;
    QString gateway;
};

// All IPv4 default routes (any NIC). Do not hard-require wlan0 — boards often use eth/usb.
QList<DefaultRoute> listDefaultRoutes()
{
    QList<DefaultRoute> out;
    QFile route(QStringLiteral("/proc/net/route"));
    if (!route.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return out;
    }
    route.readLine();
    while (!route.atEnd()) {
        const QByteArray line = route.readLine().trimmed();
        const QList<QByteArray> cols = line.split('\t');
        if (cols.size() < 3) {
            continue;
        }
        if (cols.at(1) != "00000000" || isLoopbackIface(cols.at(0))) {
            continue;
        }
        const QString gw = gatewayFromHex(cols.at(2));
        if (gw.isEmpty()) {
            continue;
        }
        out.push_back(DefaultRoute{cols.at(0), gw});
    }
    return out;
}

bool pingHostOnce(const QString &host, int timeoutMs)
{
    if (host.isEmpty()) {
        return false;
    }
    QProcess ping;
    ping.start(QStringLiteral("ping"),
               {QStringLiteral("-c"), QStringLiteral("1"), QStringLiteral("-W"),
                QStringLiteral("2"), host});
    if (!ping.waitForFinished(qMax(500, timeoutMs))) {
        ping.kill();
        ping.waitForFinished(300);
        return false;
    }
    return ping.exitCode() == 0;
}

// Product datapath: any working default route.
// - Prefer ICMP to gateway (detects AIC8800 "zombie WiFi": route+ARP but 100% loss).
// - Wired (eth/usb/en…): default route alone is enough when ICMP is filtered.
bool hasProductDatapathFast()
{
    const QList<DefaultRoute> routes = listDefaultRoutes();
    if (routes.isEmpty()) {
        return false;
    }
    for (const DefaultRoute &r : routes) {
        if (pingHostOnce(r.gateway, 2000)) {
            return true;
        }
    }
    for (const DefaultRoute &r : routes) {
        if (!isWirelessIface(r.iface)) {
            return true;
        }
    }
    return false;
}

} // namespace

QByteArray boardNetRecoverScriptBytes()
{
    QFile res(QStringLiteral(":/board/board-net-ready.sh"));
    if (res.open(QIODevice::ReadOnly)) {
        return res.readAll();
    }
    const QStringList candidates{
        QCoreApplication::applicationDirPath() + QStringLiteral("/board-net-ready.sh"),
        QStringLiteral("/root/mooncoding/board-net-ready.sh"),
    };
    for (const QString &path : candidates) {
        QFile f(path);
        if (f.open(QIODevice::ReadOnly)) {
            return f.readAll();
        }
    }
    return {};
}

bool boardNetPingInternet(int timeoutMs)
{
    Q_UNUSED(timeoutMs);
    // Any usable product datapath (wired or wireless). Do not require wlan0.
    return hasProductDatapathFast();
}

BoardNetRecover::BoardNetRecover(QObject *parent)
    : QObject(parent)
{
}

BoardNetRecover::~BoardNetRecover()
{
    if (m_proc) {
        m_proc->disconnect(this);
        if (m_proc->state() != QProcess::NotRunning) {
            m_proc->kill();
            // Do not waitForFinished here — destructor on UI thread must stay instant.
        }
        m_proc->deleteLater();
        m_proc = nullptr;
    }
}

bool BoardNetRecover::isRunning() const
{
    return m_running;
}

void BoardNetRecover::start()
{
    if (m_running) {
        return;
    }

#ifndef Q_OS_UNIX
    finishWith(true, QStringLiteral("desktop: skip board network recover"));
    return;
#else
    const QByteArray script = boardNetRecoverScriptBytes();
    if (script.isEmpty()) {
        finishWith(false, QStringLiteral("embedded board-net-ready.sh missing"));
        return;
    }

    m_scriptPath = QStringLiteral("/tmp/mooncoding-board-net-ready.sh");
    QFile out(m_scriptPath);
    if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        finishWith(false, QStringLiteral("cannot write %1").arg(m_scriptPath));
        return;
    }
    out.write(script);
    out.close();
    QFile::setPermissions(
        m_scriptPath,
        QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner
            | QFileDevice::ReadGroup | QFileDevice::ExeGroup | QFileDevice::ReadOther
            | QFileDevice::ExeOther);

    m_running = true;
    m_log.clear();

    if (m_proc) {
        m_proc->disconnect(this);
        m_proc->deleteLater();
        m_proc = nullptr;
    }
    m_proc = new QProcess(this);
    m_proc->setProcessChannelMode(QProcess::MergedChannels);
    connect(m_proc, &QProcess::readyRead, this, [this] {
        if (m_proc) {
            m_log.append(QString::fromUtf8(m_proc->readAll()));
        }
    });
    connect(m_proc,
            QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
            this,
            [this](int, QProcess::ExitStatus) { onProcessFinished(); });
    connect(m_proc, &QProcess::errorOccurred, this, [this](QProcess::ProcessError) {
        onProcessError();
    });

    QTimer::singleShot(kRecoverTimeoutMs, this, [this] {
        if (!m_running || !m_proc) {
            return;
        }
        m_log.append(QStringLiteral("\n[board-net] timed out — killing"));
        m_proc->kill();
    });

    m_proc->start(QStringLiteral("sh"), {m_scriptPath});
#endif
}

void BoardNetRecover::onProcessFinished()
{
    if (!m_running) {
        return;
    }
    if (m_proc) {
        m_log.append(QString::fromUtf8(m_proc->readAll()));
    }
    const bool ok = hasProductDatapathFast();
    if (ok) {
        m_log.append(QStringLiteral("\n[board-net] recover ok"));
    } else {
        m_log.append(QStringLiteral("\n[board-net] still no product datapath (wired/wireless)"));
    }
    finishWith(ok, m_log.trimmed());
}

void BoardNetRecover::onProcessError()
{
    if (!m_running || !m_proc) {
        return;
    }
    if (m_proc->state() == QProcess::NotRunning) {
        m_log.append(QStringLiteral("\n[board-net] process error: %1")
                         .arg(m_proc->errorString()));
        finishWith(false, m_log.trimmed());
    }
}

void BoardNetRecover::finishWith(bool ok, const QString &log)
{
    m_running = false;
    if (m_proc) {
        m_proc->disconnect(this);
        m_proc->deleteLater();
        m_proc = nullptr;
    }
    emit finished(ok, log);
}
