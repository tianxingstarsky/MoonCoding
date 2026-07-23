#pragma once

#include <QAbstractItemView>
#include <QAbstractScrollArea>
#include <QScroller>
#include <QScrollerProperties>
#include <QWidget>

namespace touchscroll {

inline void enableOn(QAbstractScrollArea *area)
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
    props.setScrollMetric(QScrollerProperties::DragStartDistance, 0.002);
    props.setScrollMetric(QScrollerProperties::MinimumVelocity, 0.0);
    scroller->setScrollerProperties(props);

    if (auto *view = qobject_cast<QAbstractItemView *>(area)) {
        view->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
        view->setHorizontalScrollMode(QAbstractItemView::ScrollPerPixel);
        view->setDragDropMode(QAbstractItemView::NoDragDrop);
    }
}

inline void enableRecursive(QWidget *root)
{
    if (!root) {
        return;
    }
    const auto areas = root->findChildren<QAbstractScrollArea *>();
    for (QAbstractScrollArea *area : areas) {
        enableOn(area);
    }
}

} // namespace touchscroll
