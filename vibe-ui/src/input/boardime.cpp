#include "boardime.h"
#include "softkeyboard.h"

#include <QApplication>
#include <QEvent>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonValue>
#include <QLineEdit>
#include <QMouseEvent>
#include <QPlainTextEdit>
#include <QTextCursor>
#include <QTextEdit>
#include <QWidget>

#ifdef HAS_QT_WEBENGINE
#include <QWebEnginePage>
#include <QWebEngineView>
#endif


#ifdef HAS_QT_WEBENGINE
namespace {
// Walk into same-origin iframes (board browser proxy) to find the real focused field.
constexpr const char kWebImeResolveActive[] =
    "function __mcDeepActive(){"
    "function walk(doc){"
    "try{"
    "var el=doc.activeElement;"
    "if(!el)return null;"
    "var tag=(el.tagName||'').toUpperCase();"
    "if(tag==='IFRAME'||tag==='FRAME'){"
    "try{if(el.contentDocument){var inner=walk(el.contentDocument);if(inner)return inner;}}catch(e){}"
    "return el;"
    "}"
    "return el;"
    "}catch(e){return null;}"
    "}"
    "try{"
    "var t=window.__mooncodingImeTarget;"
    "if(t){"
    "var tt=(t.tagName||'').toUpperCase();"
    "if(tt==='INPUT'||tt==='TEXTAREA'||t.isContentEditable)return t;"
    "}"
    "}catch(e){}"
    "return walk(document);"
    "}";
} // namespace
#endif


BoardImeController::BoardImeController(QWidget *anchorParent, QObject *parent)
    : QObject(parent)
    , m_keyboard(new SoftKeyboard(anchorParent))
{
    m_keyboard->setVisible(false);
    m_keyboard->setFocusPolicy(Qt::NoFocus);

    connect(qApp, &QApplication::focusChanged, this, &BoardImeController::onFocusChanged);
    connect(m_keyboard, &SoftKeyboard::textCommitted, this, &BoardImeController::onTextCommitted);
    connect(m_keyboard, &SoftKeyboard::backspacePressed, this, &BoardImeController::onBackspace);
    connect(m_keyboard, &SoftKeyboard::enterPressed, this, &BoardImeController::onEnter);
    connect(m_keyboard, &SoftKeyboard::hideRequested, this, &BoardImeController::onHideRequested);
    qApp->installEventFilter(this);
}

bool BoardImeController::eventFilter(QObject *watched, QEvent *event)
{
    if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonRelease) {
        auto *w = qobject_cast<QWidget *>(watched);
        if (isEditable(w) && !isInsideKeyboard(w)) {
            // Re-tap already-focused field after「收起」.
            showFor(w);
        }
    }
    return QObject::eventFilter(watched, event);
}

void BoardImeController::showFor(QWidget *w)
{
    if (!w) {
        return;
    }
    clearWebMode();
    bindTarget(w);
    m_forceHidden = false;
    if (!m_keyboard->isVisible()) {
        m_keyboard->setVisible(true);
        emit visibilityChanged(true);
    }
}

void BoardImeController::showForWebView(QWidget *webView)
{
    if (!webView) {
        return;
    }
    m_webMode = true;
    bindTarget(webView);
    m_forceHidden = false;
    if (!m_keyboard->isVisible()) {
        m_keyboard->setVisible(true);
        emit visibilityChanged(true);
    }
}

void BoardImeController::notifyWebEditableBlur()
{
    if (!m_webMode) {
        return;
    }
    clearWebMode();
    if (m_keyboard->isVisible() && !m_forceHidden) {
        m_keyboard->clearComposing();
        m_keyboard->setVisible(false);
        emit visibilityChanged(false);
    }
}

bool BoardImeController::isVisible() const
{
    return m_keyboard && m_keyboard->isVisible();
}

void BoardImeController::setVisible(bool visible)
{
    if (!m_keyboard) {
        return;
    }
    if (!visible) {
        m_forceHidden = true;
        clearWebMode();
        m_keyboard->clearComposing();
        m_keyboard->setVisible(false);
        emit visibilityChanged(false);
        return;
    }
    m_forceHidden = false;
    if (!m_target) {
        if (QWidget *f = QApplication::focusWidget()) {
            if (isEditable(f)) {
                bindTarget(f);
            }
        }
    }
    m_keyboard->setVisible(true);
    emit visibilityChanged(true);
}

void BoardImeController::toggle()
{
    setVisible(!isVisible());
}

bool BoardImeController::isEditable(QWidget *w) const
{
    if (!w || !w->isEnabled()) {
        return false;
    }
    if (auto *edit = qobject_cast<QLineEdit *>(w)) {
        return !edit->isReadOnly();
    }
    if (auto *edit = qobject_cast<QTextEdit *>(w)) {
        return !edit->isReadOnly();
    }
    if (auto *edit = qobject_cast<QPlainTextEdit *>(w)) {
        return !edit->isReadOnly();
    }
    return false;
}

bool BoardImeController::isInsideKeyboard(QWidget *w) const
{
    return w && m_keyboard && (w == m_keyboard || m_keyboard->isAncestorOf(w));
}

bool BoardImeController::isWebViewTarget(QWidget *w) const
{
    if (!m_webMode || !m_target || !w) {
        return false;
    }
    if (w == m_target.data() || m_target->isAncestorOf(w)) {
        return true;
    }
    // Chromium focus proxy under QWebEngineView.
    if (QWidget *proxy = m_target->focusProxy()) {
        if (w == proxy || proxy->isAncestorOf(w) || w->isAncestorOf(proxy)) {
            return true;
        }
    }
    return false;
}

void BoardImeController::bindTarget(QWidget *w)
{
    m_target = w;
}

void BoardImeController::clearWebMode()
{
    m_webMode = false;
}

void BoardImeController::onFocusChanged(QWidget *old, QWidget *now)
{
    Q_UNUSED(old);
    if (isInsideKeyboard(now)) {
        return;
    }
    if (isEditable(now)) {
        showFor(now);
        return;
    }
    // Keep keyboard while the micro-app web view (or its focus proxy) still has focus.
    if (isWebViewTarget(now)) {
        return;
    }
    if (m_webMode && now == nullptr) {
        // Transient focus loss while soft keys are pressed — ignore.
        return;
    }
    // Non-editable focus: hide keyboard (unless user just tapped a non-focus key — already NoFocus).
    if (m_keyboard->isVisible() && !m_forceHidden) {
        clearWebMode();
        m_keyboard->clearComposing();
        m_keyboard->setVisible(false);
        emit visibilityChanged(false);
    }
}

void BoardImeController::onTextCommitted(const QString &text)
{
    insertText(text);
}

void BoardImeController::runOnWebPage(const QString &javaScript)
{
#ifdef HAS_QT_WEBENGINE
    auto *view = qobject_cast<QWebEngineView *>(m_target.data());
    if (!view || !view->page()) {
        return;
    }
    view->page()->runJavaScript(javaScript);
#else
    Q_UNUSED(javaScript);
#endif
}

void BoardImeController::insertText(const QString &text)
{
    if (!m_target) {
        return;
    }
    if (m_webMode) {
        // Encode as a JSON string literal via a one-element array: ["text"] → "text"
        const QByteArray arrJson =
            QJsonDocument(QJsonArray{QJsonValue(text)}).toJson(QJsonDocument::Compact);
        const QString json =
            (arrJson.size() >= 2) ? QString::fromUtf8(arrJson.mid(1, arrJson.size() - 2))
                                  : QStringLiteral("\"\"");
        runOnWebPage(QStringLiteral(
                         "%1"
                         "(function(text){"
                         "var el=__mcDeepActive();"
                         "if(!el)return;"
                         "var tag=(el.tagName||'').toUpperCase();"
                         "if(tag==='INPUT'||tag==='TEXTAREA'){"
                         "var start=typeof el.selectionStart==='number'?el.selectionStart:(el.value||'').length;"
                         "var end=typeof el.selectionEnd==='number'?el.selectionEnd:start;"
                         "var v=el.value||'';"
                         "el.value=v.slice(0,start)+text+v.slice(end);"
                         "var pos=start+text.length;"
                         "try{el.setSelectionRange(pos,pos);}catch(e){}"
                         "el.dispatchEvent(new Event('input',{bubbles:true}));"
                         "el.dispatchEvent(new Event('change',{bubbles:true}));"
                         "return;}"
                         "if(el.isContentEditable){"
                         "try{document.execCommand('insertText',false,text);}catch(e){}"
                         "}"
                         "})(%2);")
                         .arg(QLatin1String(kWebImeResolveActive), json));
        return;
    }
    if (auto *edit = qobject_cast<QLineEdit *>(m_target.data())) {
        edit->insert(text);
        return;
    }
    if (auto *edit = qobject_cast<QTextEdit *>(m_target.data())) {
        edit->insertPlainText(text);
        return;
    }
    if (auto *edit = qobject_cast<QPlainTextEdit *>(m_target.data())) {
        edit->insertPlainText(text);
    }
}

void BoardImeController::onBackspace()
{
    if (!m_target) {
        return;
    }
    if (m_webMode) {
        runOnWebPage(QStringLiteral(
            "%1"
            "(function(){"
            "var el=__mcDeepActive();"
            "if(!el)return;"
            "var tag=(el.tagName||'').toUpperCase();"
            "if(tag==='INPUT'||tag==='TEXTAREA'){"
            "var start=typeof el.selectionStart==='number'?el.selectionStart:0;"
            "var end=typeof el.selectionEnd==='number'?el.selectionEnd:start;"
            "var v=el.value||'';"
            "if(start!==end){el.value=v.slice(0,start)+v.slice(end);}"
            "else if(start>0){el.value=v.slice(0,start-1)+v.slice(start);start-=1;}"
            "try{el.setSelectionRange(start,start);}catch(e){}"
            "el.dispatchEvent(new Event('input',{bubbles:true}));"
            "el.dispatchEvent(new Event('change',{bubbles:true}));"
            "return;}"
            "if(el.isContentEditable){"
            "try{document.execCommand('delete');}catch(e){}"
            "}"
            "})();").arg(QLatin1String(kWebImeResolveActive)));
        return;
    }
    if (auto *edit = qobject_cast<QLineEdit *>(m_target.data())) {
        edit->backspace();
        return;
    }
    if (auto *edit = qobject_cast<QTextEdit *>(m_target.data())) {
        QTextCursor c = edit->textCursor();
        if (!c.hasSelection()) {
            c.deletePreviousChar();
        } else {
            c.removeSelectedText();
        }
        edit->setTextCursor(c);
        return;
    }
    if (auto *edit = qobject_cast<QPlainTextEdit *>(m_target.data())) {
        QTextCursor c = edit->textCursor();
        if (!c.hasSelection()) {
            c.deletePreviousChar();
        } else {
            c.removeSelectedText();
        }
        edit->setTextCursor(c);
    }
}

void BoardImeController::onEnter()
{
    if (m_webMode) {
        runOnWebPage(QStringLiteral(
            "%1"
            "(function(){"
            "var el=__mcDeepActive();"
            "if(!el)return;"
            "var tag=(el.tagName||'').toUpperCase();"
            "if(tag==='TEXTAREA'||el.isContentEditable){"
            "var text='\\n';"
            "if(tag==='TEXTAREA'){"
            "var start=typeof el.selectionStart==='number'?el.selectionStart:(el.value||'').length;"
            "var end=typeof el.selectionEnd==='number'?el.selectionEnd:start;"
            "var v=el.value||'';"
            "el.value=v.slice(0,start)+text+v.slice(end);"
            "var pos=start+text.length;"
            "try{el.setSelectionRange(pos,pos);}catch(e){}"
            "el.dispatchEvent(new Event('input',{bubbles:true}));"
            "return;}"
            "try{document.execCommand('insertText',false,text);}catch(e){}"
            "return;}"
            "if(tag==='INPUT'){"
            "var form=el.form;"
            "if(form){"
            "if(typeof form.requestSubmit==='function'){try{form.requestSubmit();}catch(e){form.submit();}}"
            "else{form.submit();}"
            "}else{"
            "el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',keyCode:13,which:13,bubbles:true}));"
            "el.dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',code:'Enter',keyCode:13,which:13,bubbles:true}));"
            "}"
            "}"
            "})();").arg(QLatin1String(kWebImeResolveActive)));
        return;
    }
    if (qobject_cast<QLineEdit *>(m_target.data())) {
        // Line edits: keep focus; Enter often means submit elsewhere — insert nothing.
        return;
    }
    insertText(QStringLiteral("\n"));
}

void BoardImeController::onHideRequested()
{
    setVisible(false);
}
