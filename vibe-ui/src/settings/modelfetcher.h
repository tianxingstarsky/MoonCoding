#pragma once

#include <QObject>
#include <QStringList>

class QProcess;

/// Fetches OpenAI-compatible /models list (python3 HTTPS preferred; curl/wget fallback).
class ModelFetcher final : public QObject
{
    Q_OBJECT

public:
    explicit ModelFetcher(QObject *parent = nullptr);

    void fetch(const QString &baseUrl, const QString &apiKey);

signals:
    void finished(const QStringList &models, const QString &error);

private slots:
    void onProcessFinished(int exitCode);

private:
    void tryNext();
    QString caBundlePath() const;
    QStringList parseModelsJson(const QByteArray &body) const;

    QProcess *m_process = nullptr;
    QString m_apiKey;
    QStringList m_urls;
    QString m_lastDetail;
    int m_urlIndex = 0;
};
