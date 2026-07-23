#pragma once

#include <QJsonArray>
#include <QJsonObject>
#include <QLibrary>
#include <QObject>
#include <QString>

#include "vibe_agent.h"

class QByteArray;
class QTimer;
extern "C" void mooncodingEventCallback(const char *eventJson, void *userData);

class RustBridge final : public QObject
{
    Q_OBJECT

public:
    explicit RustBridge(QObject *parent = nullptr);
    ~RustBridge() override;

    bool initialize(const QString &workspace, const QJsonObject &options = {});
    bool reinitialize(const QString &workspace, const QJsonObject &options = {});
    void shutdown();
    bool isReady() const;
    bool isBusy() const;
    QString lastError() const;

public slots:
    bool sendMessage(const QString &message);
    void interrupt();
    void refreshTree();
    void refreshSessions();
    void loadSession(const QString &sessionId);
    void addTreeNode(const QJsonObject &node, quint64 expectedVersion);
    void updateTreeNode(const QString &nodeId, const QJsonObject &patch, quint64 expectedVersion);
    void deleteTreeNode(const QString &nodeId, quint64 expectedVersion);
    void releaseTreeFields(
        const QString &nodeId,
        const QJsonArray &fields,
        quint64 expectedVersion);
    bool reviewNode(const QString &nodeId);
    bool reviewAll();

    QJsonObject appsList();
    QJsonObject appsGet(const QString &name);
    QJsonObject appsReadEntry(const QString &name);
    bool appsStart(const QString &name);
    bool appsSend(const QJsonObject &event);
    bool appsStop();
    QJsonObject appsStatus();

signals:
    void readyChanged(bool ready);
    void busyChanged(bool busy);
    void thinking();
    void thinkingDelta(const QString &text);
    void textDelta(const QString &text);
    void textDone(const QString &content, quint64 tokensIn, quint64 tokensOut);
    void toolCallStarted(const QString &id, const QString &name, const QString &input);
    void toolCallFinished(
        const QString &id,
        const QString &name,
        const QString &output,
        int exitCode,
        quint64 durationMs);
    void treeUpdated(const QJsonObject &tree);
    void sessionsUpdated(const QJsonArray &sessions);
    void sessionLoaded(const QJsonObject &session);
    void agentDone(quint64 tokensIn, quint64 tokensOut, quint64 steps);
    void interrupted(const QString &reason);
    void errorOccurred(const QString &message);
    void appRuntimeEvent(const QJsonObject &event);

private:
    friend void mooncodingEventCallback(const char *eventJson, void *userData);

    using ApiVersionFn = uint32_t (*)();
    using InitFn = VibeHandle *(*)(const char *, VibeEventCallback, void *);
    using SendFn = int32_t (*)(VibeHandle *, const char *);
    using InterruptFn = int32_t (*)(VibeHandle *);
    using TreeGetFn = char *(*)(VibeHandle *);
    using SessionGetFn = char *(*)(VibeHandle *, const char *);
    using TreeRequestFn = char *(*)(VibeHandle *, const char *);
    using ReviewNodeFn = int32_t (*)(VibeHandle *, const char *);
    using ReviewAllFn = int32_t (*)(VibeHandle *);
    using AppsListFn = char *(*)(VibeHandle *);
    using AppsGetFn = char *(*)(VibeHandle *, const char *);
    using AppsReadEntryFn = char *(*)(VibeHandle *, const char *);
    using AppsStartFn = int32_t (*)(VibeHandle *, const char *);
    using AppsSendFn = int32_t (*)(VibeHandle *, const char *);
    using AppsStopFn = int32_t (*)(VibeHandle *);
    using AppsStatusFn = char *(*)(VibeHandle *);
    using LastErrorFn = char *(*)();
    using StringFreeFn = void (*)(char *);
    using DestroyFn = void (*)(VibeHandle *);

    void enqueueEvent(const QByteArray &eventJson);
    void processEvent(const QByteArray &eventJson);
    void flushDeltaBuffers();
    void setBusy(bool busy);
    bool resolveApi();
    QJsonObject callTreeRequest(TreeRequestFn function, const QJsonObject &request);
    QJsonObject decodeResponse(char *raw);
    void reportLastError(const QString &fallback);

    QLibrary m_library;
    VibeHandle *m_handle = nullptr;
    bool m_busy = false;
    QTimer *m_deltaFlushTimer = nullptr;
    QString m_pendingTextDelta;
    QString m_pendingThinkingDelta;
    QJsonObject m_pendingTreeWhileBusy;
    bool m_hasPendingTreeWhileBusy = false;

    ApiVersionFn m_apiVersion = nullptr;
    InitFn m_init = nullptr;
    SendFn m_send = nullptr;
    InterruptFn m_interrupt = nullptr;
    TreeGetFn m_treeGet = nullptr;
    TreeGetFn m_sessionsGet = nullptr;
    SessionGetFn m_sessionGet = nullptr;
    TreeRequestFn m_treeAdd = nullptr;
    TreeRequestFn m_treeUpdate = nullptr;
    TreeRequestFn m_treeDelete = nullptr;
    TreeRequestFn m_treeRelease = nullptr;
    ReviewNodeFn m_reviewNode = nullptr;
    ReviewAllFn m_reviewAll = nullptr;
    AppsListFn m_appsListFn = nullptr;
    AppsGetFn m_appsGetFn = nullptr;
    AppsReadEntryFn m_appsReadEntryFn = nullptr;
    AppsStartFn m_appsStartFn = nullptr;
    AppsSendFn m_appsSendFn = nullptr;
    AppsStopFn m_appsStopFn = nullptr;
    AppsStatusFn m_appsStatusFn = nullptr;
    LastErrorFn m_lastError = nullptr;
    StringFreeFn m_stringFree = nullptr;
    DestroyFn m_destroy = nullptr;
};
