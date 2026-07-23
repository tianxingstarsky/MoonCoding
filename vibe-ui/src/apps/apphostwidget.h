#pragma once

#include <QJsonObject>
#include <QWidget>

class AppSchemaRenderer;
class QLabel;
class QPlainTextEdit;
class QPropertyAnimation;
class QResizeEvent;
class QToolButton;

class AppHostWidget final : public QWidget
{
    Q_OBJECT

public:
    enum class RuntimeState {
        Idle,
        Starting,
        Running,
        Stopping,
        Error
    };
    Q_ENUM(RuntimeState)

    explicit AppHostWidget(QWidget *parent = nullptr);

    bool setSchema(const QJsonObject &schema, QString *error = nullptr);

public slots:
    void setApp(const QString &title, const QString &name);
    void setRuntimeState(RuntimeState state);
    void applyPatch(const QJsonObject &patch);
    void appendLog(const QString &text);
    void clearLogs();
    void revealLogs();

signals:
    void backRequested();
    void startRequested();
    void stopRequested();
    void restartRequested();
    void uiEvent(QJsonObject event);

protected:
    void resizeEvent(QResizeEvent *event) override;

private:
    void setLogsExpanded(bool expanded);
    void updateRuntimeUi();
    void updateErrorBanner();
    int expandedLogHeight() const;

    QString m_appName;
    RuntimeState m_runtimeState = RuntimeState::Idle;
    QString m_schemaError;
    bool m_logsExpanded = false;

    QWidget *m_toolbar;
    QLabel *m_titleLabel;
    QLabel *m_statusLabel;
    QToolButton *m_backButton;
    QToolButton *m_startButton;
    QToolButton *m_stopButton;
    QToolButton *m_restartButton;
    QToolButton *m_logsButton;
    QLabel *m_errorBanner;
    AppSchemaRenderer *m_renderer;
    QPlainTextEdit *m_logDrawer;
    QPropertyAnimation *m_logAnimation;
};
