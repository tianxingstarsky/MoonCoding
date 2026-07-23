#include "boardime.h"
#include "softkeyboard.h"

#include <QApplication>
#include <QEvent>
#include <QLineEdit>
#include <QMouseEvent>
#include <QPlainTextEdit>
#include <QTextCursor>
#include <QTextEdit>
#include <QWidget>

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
    bindTarget(w);
    m_forceHidden = false;
    if (!m_keyboard->isVisible()) {
        m_keyboard->setVisible(true);
        emit visibilityChanged(true);
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

void BoardImeController::bindTarget(QWidget *w)
{
    m_target = w;
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
    // Non-editable focus: hide keyboard (unless user just tapped a non-focus key — already NoFocus).
    if (m_keyboard->isVisible() && !m_forceHidden) {
        m_keyboard->clearComposing();
        m_keyboard->setVisible(false);
        emit visibilityChanged(false);
    }
}

void BoardImeController::onTextCommitted(const QString &text)
{
    insertText(text);
}

void BoardImeController::insertText(const QString &text)
{
    if (!m_target) {
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
