#pragma once

#include <QObject>
#include <QPointer>

class QEvent;
class QWidget;
class SoftKeyboard;

/// App-wide soft keyboard: auto-shows on focus of editable fields
/// (QLineEdit / QTextEdit / QPlainTextEdit, and WebEngine HTML inputs).
class BoardImeController final : public QObject
{
    Q_OBJECT

public:
    explicit BoardImeController(QWidget *anchorParent, QObject *parent = nullptr);

    SoftKeyboard *keyboard() const { return m_keyboard; }
    bool isVisible() const;
    void setVisible(bool visible);
    void toggle();
    bool eventFilter(QObject *watched, QEvent *event) override;

    /// HTML micro-app (QWebEngineView): show keyboard and route commits via JS.
    void showForWebView(QWidget *webView);
    /// HTML field blurred / left — hide if we were in web-input mode.
    void notifyWebEditableBlur();

signals:
    void visibilityChanged(bool visible);

private slots:
    void onFocusChanged(QWidget *old, QWidget *now);
    void onTextCommitted(const QString &text);
    void onBackspace();
    void onEnter();
    void onHideRequested();

private:
    bool isEditable(QWidget *w) const;
    bool isInsideKeyboard(QWidget *w) const;
    bool isWebViewTarget(QWidget *w) const;
    void bindTarget(QWidget *w);
    void clearWebMode();
    void insertText(const QString &text);
    void showFor(QWidget *w);
    void runOnWebPage(const QString &javaScript);

    SoftKeyboard *m_keyboard = nullptr;
    QPointer<QWidget> m_target;
    bool m_forceHidden = false;
    bool m_webMode = false;
};
