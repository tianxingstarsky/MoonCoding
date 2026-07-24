#include "touchscroll.h"

#include <QAbstractItemView>
#include <QEvent>
#include <QLabel>
#include <QMouseEvent>
#include <QObject>
#include <QScroller>
#include <QScrollerProperties>
#include <QTimer>
#include <Qt>

namespace touchscroll {
namespace {

constexpr int kDragSlopPx = 10;
constexpr int kLongPressMs = 520;

class SelectableScrollFilter final : public QObject
{
public:
    explicit SelectableScrollFilter(QLabel *label)
        : QObject(label)
        , m_label(label)
    {
        if (m_label) {
            m_label->setTextInteractionFlags(Qt::NoTextInteraction);
            m_label->setAttribute(Qt::WA_AcceptTouchEvents, true);
            m_label->installEventFilter(this);
        }
        m_timer.setSingleShot(true);
        QObject::connect(&m_timer, &QTimer::timeout, this, [this] {
            if (!m_label || m_dragged) {
                return;
            }
            m_armed = true;
            m_label->setTextInteractionFlags(Qt::TextSelectableByMouse);
        });
    }

protected:
    bool eventFilter(QObject *watched, QEvent *event) override
    {
        if (!m_label || watched != m_label) {
            return QObject::eventFilter(watched, event);
        }

        switch (event->type()) {
        case QEvent::MouseButtonPress: {
            auto *me = static_cast<QMouseEvent *>(event);
            if (me->button() != Qt::LeftButton) {
                break;
            }
            m_pressPos = me->pos();
            m_dragged = false;
            m_armed = false;
            m_label->setTextInteractionFlags(Qt::NoTextInteraction);
            m_label->setSelection(0, 0);
            m_timer.start(kLongPressMs);
            break;
        }
        case QEvent::MouseMove: {
            auto *me = static_cast<QMouseEvent *>(event);
            if (!(me->buttons() & Qt::LeftButton)) {
                break;
            }
            const QPoint delta = me->pos() - m_pressPos;
            if (!m_dragged
                && (qAbs(delta.x()) >= kDragSlopPx || qAbs(delta.y()) >= kDragSlopPx)) {
                m_dragged = true;
                m_timer.stop();
                m_armed = false;
                m_label->setTextInteractionFlags(Qt::NoTextInteraction);
                m_label->setSelection(0, 0);
            }
            // While dragging to scroll, swallow moves so QLabel cannot start a selection.
            if (m_dragged && !m_armed) {
                return true;
            }
            break;
        }
        case QEvent::MouseButtonRelease: {
            m_timer.stop();
            if (m_dragged && !m_armed) {
                m_label->setTextInteractionFlags(Qt::NoTextInteraction);
                m_label->setSelection(0, 0);
                // Swallow release after a scroll-drag so a tiny selection cannot stick.
                return true;
            }
            if (!m_armed) {
                m_label->setTextInteractionFlags(Qt::NoTextInteraction);
            }
            break;
        }
        case QEvent::Leave:
        case QEvent::FocusOut:
            m_timer.stop();
            break;
        default:
            break;
        }
        return QObject::eventFilter(watched, event);
    }

private:
    QLabel *m_label = nullptr;
    QTimer m_timer;
    QPoint m_pressPos;
    bool m_dragged = false;
    bool m_armed = false;
};

} // namespace

void enableOn(QAbstractScrollArea *area)
{
    if (!area || !area->viewport()) {
        return;
    }
    area->setHorizontalScrollBarPolicy(area->horizontalScrollBarPolicy());
    area->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    area->viewport()->setAttribute(Qt::WA_AcceptTouchEvents, true);

    // linuxfb synthesizes mouse from touch — LeftMouseButtonGesture matches that.
    QScroller::grabGesture(area->viewport(), QScroller::LeftMouseButtonGesture);

    QScroller *scroller = QScroller::scroller(area->viewport());
    if (!scroller) {
        return;
    }
    QScrollerProperties props = scroller->scrollerProperties();
    props.setScrollMetric(QScrollerProperties::VerticalOvershootPolicy,
                          QScrollerProperties::OvershootAlwaysOff);
    props.setScrollMetric(QScrollerProperties::HorizontalOvershootPolicy,
                          QScrollerProperties::OvershootAlwaysOff);
    // Slightly larger than before so tiny finger jitter does not fight scroll.
    props.setScrollMetric(QScrollerProperties::DragStartDistance, 0.012);
    props.setScrollMetric(QScrollerProperties::MinimumVelocity, 0.0);
    scroller->setScrollerProperties(props);

    if (auto *view = qobject_cast<QAbstractItemView *>(area)) {
        view->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
        view->setHorizontalScrollMode(QAbstractItemView::ScrollPerPixel);
        view->setDragDropMode(QAbstractItemView::NoDragDrop);
    }
}

void enableRecursive(QWidget *root)
{
    if (!root) {
        return;
    }
    const auto areas = root->findChildren<QAbstractScrollArea *>();
    for (QAbstractScrollArea *area : areas) {
        enableOn(area);
    }
}

void makeScrollFriendlySelectable(QLabel *label)
{
    if (!label) {
        return;
    }
    // One filter per label; parented to the label for lifetime.
    if (label->property("touchscrollSelectable").toBool()) {
        return;
    }
    label->setProperty("touchscrollSelectable", true);
    new SelectableScrollFilter(label);
}

} // namespace touchscroll
