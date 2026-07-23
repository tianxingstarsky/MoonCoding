#include "modelfetcher.h"

#include <QCoreApplication>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QStandardPaths>

ModelFetcher::ModelFetcher(QObject *parent)
    : QObject(parent)
    , m_process(new QProcess(this))
{
    connect(m_process, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
            this, [this](int code, QProcess::ExitStatus) { onProcessFinished(code); });
    connect(m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError) {
        if (m_process->state() == QProcess::NotRunning) {
            tryNext();
        }
    });
}

void ModelFetcher::fetch(const QString &baseUrl, const QString &apiKey)
{
    if (m_process->state() != QProcess::NotRunning) {
        m_process->kill();
        m_process->waitForFinished(500);
    }
    m_apiKey = apiKey;
    m_urls.clear();
    m_urlIndex = 0;
    m_lastDetail.clear();

    QString base = baseUrl.trimmed();
    while (base.endsWith(QLatin1Char('/'))) {
        base.chop(1);
    }
    if (base.isEmpty()) {
        emit finished({}, tr("请先填写 Base URL"));
        return;
    }
    m_urls << (base + QStringLiteral("/models"));
    if (!base.endsWith(QLatin1String("/v1"))) {
        m_urls << (base + QStringLiteral("/v1/models"));
    }
    tryNext();
}

QString ModelFetcher::caBundlePath() const
{
    const QStringList candidates{
        QCoreApplication::applicationDirPath() + QStringLiteral("/certs/cacert.pem"),
        QStringLiteral("/root/mooncoding/certs/cacert.pem"),
        QStringLiteral("/etc/ssl/certs/ca-certificates.crt"),
    };
    for (const QString &path : candidates) {
        if (QFile::exists(path)) {
            return path;
        }
    }
    return {};
}

void ModelFetcher::tryNext()
{
    if (m_urlIndex >= m_urls.size()) {
        QString msg = tr("无法获取模型列表（检查 WiFi / URL / Key）");
        if (!m_lastDetail.isEmpty()) {
            msg += QStringLiteral("\n") + m_lastDetail;
        }
        emit finished({}, msg);
        return;
    }
    const QString url = m_urls.at(m_urlIndex++);

    // Prefer python3: board BusyBox wget has no HTTPS; curl may be absent.
    const QString python = QStandardPaths::findExecutable(QStringLiteral("python3"));
    if (!python.isEmpty()) {
        const QString script = QStringLiteral(
            "import os,ssl,sys,urllib.request\n"
            "url=sys.argv[1]; key=sys.argv[2] if len(sys.argv)>2 else ''\n"
            "ca=os.environ.get('SSL_CERT_FILE') or ''\n"
            "ctx=ssl.create_default_context(cafile=ca) if ca else ssl.create_default_context()\n"
            "req=urllib.request.Request(url, headers={'User-Agent':'MoonCoding/1.0'})\n"
            "if key: req.add_header('Authorization','Bearer '+key)\n"
            "try:\n"
            "  with urllib.request.urlopen(req, timeout=20, context=ctx) as r:\n"
            "    sys.stdout.buffer.write(r.read())\n"
            "except Exception as e:\n"
            "  sys.stderr.write(str(e)); sys.exit(1)\n");
        m_process->setProcessChannelMode(QProcess::SeparateChannels);
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        const QString ca = caBundlePath();
        if (!ca.isEmpty()) {
            env.insert(QStringLiteral("SSL_CERT_FILE"), ca);
            env.insert(QStringLiteral("REQUESTS_CA_BUNDLE"), ca);
            env.insert(QStringLiteral("CURL_CA_BUNDLE"), ca);
        }
        m_process->setProcessEnvironment(env);
        m_process->start(python, {QStringLiteral("-c"), script, url, m_apiKey});
        return;
    }

    QString bin = QStandardPaths::findExecutable(QStringLiteral("curl"));
    QStringList args;
    if (!bin.isEmpty()) {
        args << QStringLiteral("-sS")
             << QStringLiteral("--max-time") << QStringLiteral("20");
        const QString ca = caBundlePath();
        if (!ca.isEmpty()) {
            args << QStringLiteral("--cacert") << ca;
        }
        if (!m_apiKey.isEmpty()) {
            args << QStringLiteral("-H")
                 << QStringLiteral("Authorization: Bearer %1").arg(m_apiKey);
        }
        args << url;
        m_process->start(bin, args);
        return;
    }

    // BusyBox wget: HTTP only — skip https URLs.
    if (url.startsWith(QLatin1String("https://"), Qt::CaseInsensitive)) {
        m_lastDetail = tr("板端无 python3/curl，BusyBox wget 不支持 HTTPS");
        tryNext();
        return;
    }
    bin = QStandardPaths::findExecutable(QStringLiteral("wget"));
    if (bin.isEmpty()) {
        emit finished({}, tr("板端无 python3/curl/wget，无法拉取模型列表"));
        return;
    }
    args << QStringLiteral("-qO-")
         << QStringLiteral("-T") << QStringLiteral("12");
    if (!m_apiKey.isEmpty()) {
        args << QStringLiteral("--header=Authorization: Bearer %1").arg(m_apiKey);
    }
    args << url;
    m_process->start(bin, args);
}

void ModelFetcher::onProcessFinished(int exitCode)
{
    const QByteArray body = m_process->readAllStandardOutput();
    const QByteArray errOut = m_process->readAllStandardError().trimmed();
    if (exitCode == 0 && !body.isEmpty()) {
        const QStringList models = parseModelsJson(body);
        if (!models.isEmpty()) {
            emit finished(models, QString());
            return;
        }
        m_lastDetail = tr("接口有返回，但未解析到 data[].id");
    } else if (!errOut.isEmpty()) {
        m_lastDetail = QString::fromUtf8(errOut.left(200));
    } else if (exitCode != 0) {
        m_lastDetail = tr("拉取失败（exit %1）").arg(exitCode);
    }
    tryNext();
}

QStringList ModelFetcher::parseModelsJson(const QByteArray &body) const
{
    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(body, &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject()) {
        return {};
    }
    const QJsonArray data = doc.object().value(QStringLiteral("data")).toArray();
    QStringList out;
    for (const QJsonValue &v : data) {
        const QString id = v.toObject().value(QStringLiteral("id")).toString().trimmed();
        if (!id.isEmpty() && !out.contains(id)) {
            out.append(id);
        }
    }
    out.sort(Qt::CaseInsensitive);
    return out;
}
