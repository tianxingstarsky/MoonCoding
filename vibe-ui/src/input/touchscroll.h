#pragma once

#include <QAbstractItemView>
#include <QAbstractScrollArea>
#include <QLabel>
#include <QScroller>
#include <QScrollerProperties>
#include <QWidget>

namespace touchscroll {

void enableOn(QAbstractScrollArea *area);
void enableRecursive(QWidget *root);

/// Prefer scroll over text selection: drag scrolls; long-press enables select/copy.
void makeScrollFriendlySelectable(QLabel *label);

} // namespace touchscroll
