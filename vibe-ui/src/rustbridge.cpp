#include "rustbridge.h"

#include <QCoreApplication>
#include <QFile>
#include <QIODevice>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QMetaObject>
#include <QTimer>

namespace {
template<typename T>
T resolve(QLibrary &library, const char *symbol)
{
    return reinterpret_cast<T>(library.resolve(symbol));
}

void breadcrumb(const char *msg)
{
    QFile f(QStringLiteral("/tmp/mooncoding-breadcrumb.log"));
    if (f.open(QIODevice::WriteOnly | QIODevice::Append | QIODevice::Text)) {
        f.write(msg);
        f.write("\n");
    }
}
} // namespace

RustBridge::RustBridge(QObject *parent)
    : QObject(parent)
    , m_library(QCoreApplication::applicationDirPath() + QStringLiteral("/vibe_agent"))
    , m_deltaFlushTimer(new QTimer(this))
{
    m_deltaFlushTimer->setSingleShot(true);
    const bool board = qEnvironmentVariableIsSet("MOONCODING_BOARD")
        || qgetenv("QT_QPA_PLATFORM").startsWith("linuxfb");
    m_deltaFlushTimer->setInterval(board ? 250 : 120);
    connect(m_deltaFlushTimer, &QTimer::timeout, this, &RustBridge::flushDeltaBuffers);
}

RustBridge::~RustBridge()
{
    shutdown();
    if (m_library.isLoaded()) {
        m_library.unload();
    }
}

bool RustBridge::initialize(const QString &workspace, const QJsonObject &options)
{
    if (m_handle) {
        return true;
    }
    if (!m_library.load()) {
        emit errorOccurred(tr("无法加载 Rust 后端：%1").arg(m_library.errorString()));
        return false;
    }
    if (!resolveApi()) {
        emit errorOccurred(tr("Rust 后端 API 不兼容。"));
        m_library.unload();
        return false;
    }
    if (m_apiVersion() != 3) {
        emit errorOccurred(
            tr("Rust 后端 API 版本 %1 不受支持。").arg(m_apiVersion()));
        m_library.unload();
        return false;
    }

    QJsonObject initOptions = options;
    initOptions.insert(QStringLiteral("workspace"), workspace);
    const QByteArray encoded = QJsonDocument(initOptions).toJson(QJsonDocument::Compact);
    m_handle = m_init(encoded.constData(), &mooncodingEventCallback, this);
    if (!m_handle) {
        reportLastError(tr("无法初始化 Rust 后端。"));
        return false;
    }
    emit readyChanged(true);
    refreshTree();
    refreshSessions();
    const QString sessionId = options.value(QStringLiteral("session_id")).toString();
    if (!sessionId.isEmpty()) {
        loadSession(sessionId);
    }
    return true;
}

bool RustBridge::reinitialize(const QString &workspace, const QJsonObject &options)
{
    if (m_busy) {
        emit errorOccurred(tr("请先停止 Agent 再切换项目或对话。"));
        return false;
    }
    shutdown();
    return initialize(workspace, options);
}

void RustBridge::shutdown()
{
    if (m_handle && m_destroy) {
        m_destroy(m_handle);
        m_handle = nullptr;
    }
    setBusy(false);
    emit readyChanged(false);
}

bool RustBridge::isReady() const
{
    return m_handle != nullptr;
}

bool RustBridge::isBusy() const
{
    return m_busy;
}

QString RustBridge::lastError() const
{
    if (!m_lastError || !m_stringFree) {
        return {};
    }
    char *raw = m_lastError();
    if (!raw) {
        return {};
    }
    const QString message = QString::fromUtf8(raw);
    m_stringFree(raw);
    return message;
}

bool RustBridge::sendMessage(const QString &message)
{
    if (!m_handle || !m_send) {
        emit errorOccurred(tr("后端未就绪。"));
        return false;
    }
    if (m_busy) {
        emit errorOccurred(tr("Agent 正在工作中。"));
        return false;
    }
    const QByteArray encoded = message.toUtf8();
    if (m_send(m_handle, encoded.constData()) != 0) {
        reportLastError(tr("无法发送消息。"));
        return false;
    }
    setBusy(true);
    return true;
}

void RustBridge::interrupt()
{
    if (m_handle && m_interrupt && m_interrupt(m_handle) != 0) {
        reportLastError(tr("无法中断 Agent。"));
    }
}

void RustBridge::refreshTree()
{
    if (!m_handle || !m_treeGet) {
        return;
    }
    const QJsonObject response = decodeResponse(m_treeGet(m_handle));
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit treeUpdated(response.value(QStringLiteral("data")).toObject());
    }
}

void RustBridge::refreshSessions()
{
    if (!m_handle || !m_sessionsGet) {
        return;
    }
    const QJsonObject response = decodeResponse(m_sessionsGet(m_handle));
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit sessionsUpdated(response.value(QStringLiteral("data")).toArray());
    }
}

void RustBridge::loadSession(const QString &sessionId)
{
    if (!m_handle || !m_sessionGet || sessionId.trimmed().isEmpty()) {
        return;
    }
    const QByteArray encoded = sessionId.toUtf8();
    const QJsonObject response = decodeResponse(m_sessionGet(m_handle, encoded.constData()));
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit sessionLoaded(response.value(QStringLiteral("data")).toObject());
    }
}

void RustBridge::addTreeNode(const QJsonObject &node, quint64 expectedVersion)
{
    QJsonObject request{
        {QStringLiteral("expected_version"), static_cast<qint64>(expectedVersion)},
        {QStringLiteral("node"), node},
    };
    const QJsonObject response = callTreeRequest(m_treeAdd, request);
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit treeUpdated(
            response.value(QStringLiteral("data")).toObject().value(QStringLiteral("tree")).toObject());
    } else {
        refreshTree();
    }
}

void RustBridge::updateTreeNode(
    const QString &nodeId,
    const QJsonObject &patch,
    quint64 expectedVersion)
{
    QJsonObject request{
        {QStringLiteral("expected_version"), static_cast<qint64>(expectedVersion)},
        {QStringLiteral("node_id"), nodeId},
        {QStringLiteral("patch"), patch},
    };
    const QJsonObject response = callTreeRequest(m_treeUpdate, request);
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit treeUpdated(response.value(QStringLiteral("data")).toObject());
    } else {
        refreshTree();
    }
}

void RustBridge::deleteTreeNode(const QString &nodeId, quint64 expectedVersion)
{
    QJsonObject request{
        {QStringLiteral("expected_version"), static_cast<qint64>(expectedVersion)},
        {QStringLiteral("node_id"), nodeId},
    };
    const QJsonObject response = callTreeRequest(m_treeDelete, request);
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit treeUpdated(
            response.value(QStringLiteral("data")).toObject().value(QStringLiteral("tree")).toObject());
    } else {
        refreshTree();
    }
}

void RustBridge::releaseTreeFields(
    const QString &nodeId,
    const QJsonArray &fields,
    quint64 expectedVersion)
{
    QJsonObject request{
        {QStringLiteral("expected_version"), static_cast<qint64>(expectedVersion)},
        {QStringLiteral("node_id"), nodeId},
        {QStringLiteral("fields"), fields},
    };
    const QJsonObject response = callTreeRequest(m_treeRelease, request);
    if (response.value(QStringLiteral("ok")).toBool()) {
        emit treeUpdated(response.value(QStringLiteral("data")).toObject());
    } else {
        refreshTree();
    }
}

bool RustBridge::reviewNode(const QString &nodeId)
{
    if (!m_handle || !m_reviewNode || m_busy) {
        emit errorOccurred(tr("请等待 Agent 空闲后再审视节点。"));
        return false;
    }
    const QByteArray encoded = nodeId.toUtf8();
    if (m_reviewNode(m_handle, encoded.constData()) != 0) {
        reportLastError(tr("无法审视所选节点。"));
        return false;
    }
    setBusy(true);
    return true;
}

bool RustBridge::reviewAll()
{
    if (!m_handle || !m_reviewAll || m_busy) {
        emit errorOccurred(tr("请等待 Agent 空闲后再审视项目树。"));
        return false;
    }
    if (m_reviewAll(m_handle) != 0) {
        reportLastError(tr("无法审视项目树。"));
        return false;
    }
    setBusy(true);
    return true;
}

QJsonObject RustBridge::appsList()
{
    if (!m_handle || !m_appsListFn) {
        return {};
    }
    return decodeResponse(m_appsListFn(m_handle));
}

QJsonObject RustBridge::appsGet(const QString &name)
{
    if (!m_handle || !m_appsGetFn || name.trimmed().isEmpty()) {
        return {};
    }
    const QByteArray encoded = name.toUtf8();
    return decodeResponse(m_appsGetFn(m_handle, encoded.constData()));
}

QJsonObject RustBridge::appsReadEntry(const QString &name)
{
    if (!m_handle || !m_appsReadEntryFn || name.trimmed().isEmpty()) {
        return {};
    }
    const QByteArray encoded = name.toUtf8();
    return decodeResponse(m_appsReadEntryFn(m_handle, encoded.constData()));
}

bool RustBridge::appsStart(const QString &name)
{
    if (!m_handle || !m_appsStartFn || name.trimmed().isEmpty()) {
        emit errorOccurred(tr("应用运行时不可用。"));
        return false;
    }
    const QByteArray encoded = name.toUtf8();
    if (m_appsStartFn(m_handle, encoded.constData()) != 0) {
        reportLastError(tr("无法启动应用。"));
        return false;
    }
    return true;
}

bool RustBridge::appsSend(const QJsonObject &event)
{
    if (!m_handle || !m_appsSendFn) {
        emit errorOccurred(tr("应用运行时不可用。"));
        return false;
    }
    const QByteArray encoded = QJsonDocument(event).toJson(QJsonDocument::Compact);
    if (m_appsSendFn(m_handle, encoded.constData()) != 0) {
        reportLastError(tr("无法发送应用事件。"));
        return false;
    }
    return true;
}

bool RustBridge::appsStop()
{
    if (!m_handle || !m_appsStopFn) {
        emit errorOccurred(tr("应用运行时不可用。"));
        return false;
    }
    if (m_appsStopFn(m_handle) != 0) {
        reportLastError(tr("无法停止应用。"));
        return false;
    }
    return true;
}

QJsonObject RustBridge::appsStatus()
{
    if (!m_handle || !m_appsStatusFn) {
        return {};
    }
    return decodeResponse(m_appsStatusFn(m_handle));
}

extern "C" void mooncodingEventCallback(const char *eventJson, void *userData)
{
    if (!eventJson || !userData) {
        return;
    }
    auto *bridge = static_cast<RustBridge *>(userData);
    bridge->enqueueEvent(QByteArray(eventJson));
}

void RustBridge::enqueueEvent(const QByteArray &payload)
{
    QMetaObject::invokeMethod(
        this,
        [this, payload] { processEvent(payload); },
        Qt::QueuedConnection);
}

void RustBridge::flushDeltaBuffers()
{
    if (!m_pendingThinkingDelta.isEmpty()) {
        const QString chunk = m_pendingThinkingDelta;
        m_pendingThinkingDelta.clear();
        emit thinkingDelta(chunk);
    }
    if (!m_pendingTextDelta.isEmpty()) {
        const QString chunk = m_pendingTextDelta;
        m_pendingTextDelta.clear();
        emit textDelta(chunk);
    }
}

void RustBridge::processEvent(const QByteArray &eventJson)
{
    if (eventJson == "\"Thinking\"") {
        flushDeltaBuffers();
        setBusy(true);
        emit thinking();
        return;
    }

    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(eventJson, &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        emit errorOccurred(tr("无效的后端事件：%1").arg(parseError.errorString()));
        return;
    }

    const QJsonObject root = document.object();
    if (root.contains(QStringLiteral("ThinkingDelta"))) {
        setBusy(true);
        m_pendingThinkingDelta += root.value(QStringLiteral("ThinkingDelta")).toString();
        // Cap buffer so a runaway model cannot blow RAM on the board.
        if (m_pendingThinkingDelta.size() > 12000) {
            m_pendingThinkingDelta = m_pendingThinkingDelta.right(8000);
        }
        if (m_deltaFlushTimer && !m_deltaFlushTimer->isActive()) {
            m_deltaFlushTimer->start();
        }
        return;
    }
    if (root.contains(QStringLiteral("TextDelta"))) {
        setBusy(true);
        m_pendingTextDelta += root.value(QStringLiteral("TextDelta")).toString();
        if (m_pendingTextDelta.size() > 24000) {
            m_pendingTextDelta = m_pendingTextDelta.right(16000);
        }
        if (m_deltaFlushTimer && !m_deltaFlushTimer->isActive()) {
            m_deltaFlushTimer->start();
        }
        return;
    }

    // Non-delta events must see flushed text first (ordering).
    flushDeltaBuffers();

    if (root.contains(QStringLiteral("TextDone"))) {
        const QJsonObject value = root.value(QStringLiteral("TextDone")).toObject();
        emit textDone(
            value.value(QStringLiteral("content")).toString(),
            value.value(QStringLiteral("tokens_in")).toInteger(),
            value.value(QStringLiteral("tokens_out")).toInteger());
    } else if (root.contains(QStringLiteral("ToolCallStart"))) {
        breadcrumb("tool_start");
        const QJsonObject value = root.value(QStringLiteral("ToolCallStart")).toObject();
        emit toolCallStarted(
            value.value(QStringLiteral("id")).toString(),
            value.value(QStringLiteral("name")).toString(),
            value.value(QStringLiteral("input")).toString());
    } else if (root.contains(QStringLiteral("ToolCallResult"))) {
        breadcrumb("tool_result");
        const QJsonObject value = root.value(QStringLiteral("ToolCallResult")).toObject();
        QString output = value.value(QStringLiteral("output")).toString();
        constexpr int kMaxToolOut = 3500;
        if (output.size() > kMaxToolOut) {
            output = output.left(kMaxToolOut) + QStringLiteral("\n…");
        }
        emit toolCallFinished(
            value.value(QStringLiteral("id")).toString(),
            value.value(QStringLiteral("name")).toString(),
            output,
            value.value(QStringLiteral("exit_code")).toInt(),
            value.value(QStringLiteral("duration_ms")).toInteger());
    } else if (root.contains(QStringLiteral("TreeUpdated"))) {
        const QString encodedTree =
            root.value(QStringLiteral("TreeUpdated")).toObject().value(QStringLiteral("json")).toString();
        QJsonParseError treeError;
        const QJsonDocument treeDocument =
            QJsonDocument::fromJson(encodedTree.toUtf8(), &treeError);
        if (treeDocument.isObject()) {
            // While streaming, only keep the latest tree — never hammer UI/banner.
            if (m_busy) {
                m_pendingTreeWhileBusy = treeDocument.object();
                m_hasPendingTreeWhileBusy = true;
            } else {
                emit treeUpdated(treeDocument.object());
            }
        } else {
            emit errorOccurred(
                tr("无效的后端树更新：%1").arg(treeError.errorString()));
        }
    } else if (root.contains(QStringLiteral("Done"))) {
        breadcrumb("done");
        const QJsonObject value = root.value(QStringLiteral("Done")).toObject();
        setBusy(false);
        emit agentDone(
            value.value(QStringLiteral("tokens_in")).toInteger(),
            value.value(QStringLiteral("tokens_out")).toInteger(),
            value.value(QStringLiteral("steps")).toInteger());
        refreshSessions();
    } else if (root.contains(QStringLiteral("Interrupted"))) {
        breadcrumb("interrupted");
        setBusy(false);
        emit interrupted(root.value(QStringLiteral("Interrupted")).toString());
    } else if (root.contains(QStringLiteral("AppRuntime"))) {
        const QJsonValue value = root.value(QStringLiteral("AppRuntime"));
        if (value.isObject()) {
            emit appRuntimeEvent(value.toObject());
        } else {
            emit errorOccurred(tr("无效的应用运行时事件。"));
        }
    } else if (root.contains(QStringLiteral("Error"))) {
        breadcrumb("error");
        setBusy(false);
        emit errorOccurred(root.value(QStringLiteral("Error")).toString());
    } else {
        emit errorOccurred(tr("未知的后端事件类型。"));
    }
}

void RustBridge::setBusy(bool busy)
{
    if (m_busy == busy) {
        return;
    }
    m_busy = busy;
    emit busyChanged(busy);
    if (!busy && m_hasPendingTreeWhileBusy) {
        m_hasPendingTreeWhileBusy = false;
        const QJsonObject tree = m_pendingTreeWhileBusy;
        m_pendingTreeWhileBusy = QJsonObject{};
        emit treeUpdated(tree);
    }
}

bool RustBridge::resolveApi()
{
    m_apiVersion = resolve<ApiVersionFn>(m_library, "vibe_api_version");
    m_init = resolve<InitFn>(m_library, "vibe_init");
    m_send = resolve<SendFn>(m_library, "vibe_send");
    m_interrupt = resolve<InterruptFn>(m_library, "vibe_interrupt");
    m_treeGet = resolve<TreeGetFn>(m_library, "vibe_tree_get_json");
    m_sessionsGet = resolve<TreeGetFn>(m_library, "vibe_sessions_get_json");
    m_sessionGet = resolve<SessionGetFn>(m_library, "vibe_session_get_json");
    m_treeAdd = resolve<TreeRequestFn>(m_library, "vibe_tree_add_node");
    m_treeUpdate = resolve<TreeRequestFn>(m_library, "vibe_tree_update_node");
    m_treeDelete = resolve<TreeRequestFn>(m_library, "vibe_tree_delete_node");
    m_treeRelease = resolve<TreeRequestFn>(m_library, "vibe_tree_release_fields");
    m_reviewNode = resolve<ReviewNodeFn>(m_library, "vibe_tree_review_node");
    m_reviewAll = resolve<ReviewAllFn>(m_library, "vibe_tree_review_all");
    m_appsListFn = resolve<AppsListFn>(m_library, "vibe_apps_list_json");
    m_appsGetFn = resolve<AppsGetFn>(m_library, "vibe_apps_get_json");
    m_appsReadEntryFn = resolve<AppsReadEntryFn>(m_library, "vibe_apps_read_entry");
    m_appsStartFn = resolve<AppsStartFn>(m_library, "vibe_apps_start");
    m_appsSendFn = resolve<AppsSendFn>(m_library, "vibe_apps_send");
    m_appsStopFn = resolve<AppsStopFn>(m_library, "vibe_apps_stop");
    m_appsStatusFn = resolve<AppsStatusFn>(m_library, "vibe_apps_status_json");
    m_lastError = resolve<LastErrorFn>(m_library, "vibe_last_error");
    m_stringFree = resolve<StringFreeFn>(m_library, "vibe_string_free");
    m_destroy = resolve<DestroyFn>(m_library, "vibe_destroy");
    return m_apiVersion && m_init && m_send && m_interrupt && m_treeGet && m_sessionsGet
        && m_sessionGet && m_treeAdd && m_treeUpdate
        && m_treeDelete && m_treeRelease && m_reviewNode && m_reviewAll
        && m_appsListFn && m_appsGetFn && m_appsReadEntryFn
        && m_appsStartFn && m_appsSendFn && m_appsStopFn && m_appsStatusFn
        && m_lastError && m_stringFree && m_destroy;
}

QJsonObject RustBridge::callTreeRequest(TreeRequestFn function, const QJsonObject &request)
{
    if (!m_handle || !function) {
        emit errorOccurred(tr("后端树 API 不可用。"));
        return {};
    }
    const QByteArray encoded = QJsonDocument(request).toJson(QJsonDocument::Compact);
    return decodeResponse(function(m_handle, encoded.constData()));
}

QJsonObject RustBridge::decodeResponse(char *raw)
{
    if (!raw || !m_stringFree) {
        reportLastError(tr("后端返回空响应。"));
        return {};
    }
    const QByteArray encoded(raw);
    m_stringFree(raw);
    const QJsonDocument document = QJsonDocument::fromJson(encoded);
    if (!document.isObject()) {
        emit errorOccurred(tr("后端返回格式错误的 JSON。"));
        return {};
    }
    const QJsonObject response = document.object();
    if (!response.value(QStringLiteral("ok")).toBool()) {
        emit errorOccurred(response.value(QStringLiteral("error")).toString());
    }
    return response;
}

void RustBridge::reportLastError(const QString &fallback)
{
    const QString backendError = lastError();
    emit errorOccurred(backendError.isEmpty() ? fallback : backendError);
}
