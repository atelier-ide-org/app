//! Layout of the panes of a terminal tab: a binary tree of splits, as IntelliJ nests them.
//! Each split is side by side or stacked and has a ratio the user drags.

use eframe::egui::{self, Color32, Rect, Sense, pos2, vec2};

pub enum Node<T> {
    Leaf(T),
    Split {
        /// Children stacked top to bottom instead of side by side.
        stacked: bool,
        /// Share of the space taken by `first`, between 0 and 1.
        ratio: f32,
        first: Box<Node<T>>,
        second: Box<Node<T>>,
    },
    /// Only exists while the tree is being rebuilt.
    Empty,
}

/// A draggable line between the two children of a split.
pub struct Divider {
    /// Way down from the root: false is the first child, true the second.
    pub path: Vec<bool>,
    pub rect: Rect,
    pub stacked: bool,
    /// Area of the whole split, to turn a pointer position into a ratio.
    pub area: Rect,
    /// Share of the first child now.
    pub ratio: f32,
}

pub const MIN_RATIO: f32 = 0.1;

impl<T> Node<T> {
    pub fn leaves(&self) -> Vec<&T> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect<'a>(&'a self, out: &mut Vec<&'a T>) {
        match self {
            Node::Leaf(t) => out.push(t),
            Node::Split { first, second, .. } => {
                first.collect(out);
                second.collect(out);
            }
            Node::Empty => {}
        }
    }

    pub fn leaves_mut(&mut self) -> Vec<&mut T> {
        let mut out = Vec::new();
        self.collect_mut(&mut out);
        out
    }

    fn collect_mut<'a>(&'a mut self, out: &mut Vec<&'a mut T>) {
        match self {
            Node::Leaf(t) => out.push(t),
            Node::Split { first, second, .. } => {
                first.collect_mut(out);
                second.collect_mut(out);
            }
            Node::Empty => {}
        }
    }

    /// Replaces the leaf `is_target` by a split holding it and `new`, which comes second.
    pub fn split(&mut self, is_target: &impl Fn(&T) -> bool, stacked: bool, new: T) -> bool {
        self.split_with(is_target, stacked, false, Node::Leaf(new))
    }

    /// Same with a whole tree, placed before the leaf (left or above) or after it.
    pub fn split_with(
        &mut self,
        is_target: &impl Fn(&T) -> bool,
        stacked: bool,
        before: bool,
        new: Node<T>,
    ) -> bool {
        match self {
            Node::Leaf(t) if is_target(t) => {
                let old = std::mem::replace(self, Node::Empty);
                let (first, second) = if before { (new, old) } else { (old, new) };
                *self = Node::Split {
                    stacked,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            Node::Split { first, second, .. } => {
                // `new` moves into only one of the branches.
                if first.contains(is_target) {
                    first.split_with(is_target, stacked, before, new)
                } else {
                    second.split_with(is_target, stacked, before, new)
                }
            }
            _ => false,
        }
    }

    pub fn contains(&self, is_target: &impl Fn(&T) -> bool) -> bool {
        self.leaves().into_iter().any(is_target)
    }

    /// Removes the leaf `is_target`; its sibling takes the place of their split. The only
    /// remaining leaf cannot be removed.
    pub fn remove(&mut self, is_target: &impl Fn(&T) -> bool) -> bool {
        self.take(is_target).is_some()
    }

    /// Like `remove`, and gives the leaf back.
    pub fn take(&mut self, is_target: &impl Fn(&T) -> bool) -> Option<T> {
        let Node::Split { first, second, .. } = self else {
            return None;
        };
        let first_is_target = matches!(&**first, Node::Leaf(t) if is_target(t));
        let second_is_target = matches!(&**second, Node::Leaf(t) if is_target(t));
        if first_is_target || second_is_target {
            let (gone, kept) = if first_is_target {
                (first, second)
            } else {
                (second, first)
            };
            let Node::Leaf(leaf) = std::mem::replace(&mut **gone, Node::Empty) else {
                return None;
            };
            *self = std::mem::replace(&mut **kept, Node::Empty);
            Some(leaf)
        } else {
            first.take(is_target).or_else(|| second.take(is_target))
        }
    }

    pub fn ratio_mut(&mut self, path: &[bool]) -> Option<&mut f32> {
        let Node::Split {
            ratio,
            first,
            second,
            ..
        } = self
        else {
            return None;
        };
        match path.split_first() {
            None => Some(ratio),
            Some((false, rest)) => first.ratio_mut(rest),
            Some((true, rest)) => second.ratio_mut(rest),
        }
    }

    /// Rectangle of each leaf, in the order of `leaves`, and the dividers between them.
    pub fn layout(&self, rect: Rect, gap: f32) -> (Vec<Rect>, Vec<Divider>) {
        let (mut rects, mut dividers) = (Vec::new(), Vec::new());
        self.place(rect, gap, &mut Vec::new(), &mut rects, &mut dividers);
        (rects, dividers)
    }

    fn place(
        &self,
        rect: Rect,
        gap: f32,
        path: &mut Vec<bool>,
        rects: &mut Vec<Rect>,
        dividers: &mut Vec<Divider>,
    ) {
        match self {
            Node::Leaf(_) => rects.push(rect),
            Node::Empty => {}
            Node::Split {
                stacked,
                ratio,
                first,
                second,
            } => {
                let total = if *stacked {
                    rect.height()
                } else {
                    rect.width()
                } - gap;
                let size = (total * ratio).max(0.0);
                let (a, line, b) = if *stacked {
                    (
                        Rect::from_min_size(rect.min, vec2(rect.width(), size)),
                        Rect::from_min_size(
                            pos2(rect.left(), rect.top() + size),
                            vec2(rect.width(), gap),
                        ),
                        Rect::from_min_max(pos2(rect.left(), rect.top() + size + gap), rect.max),
                    )
                } else {
                    (
                        Rect::from_min_size(rect.min, vec2(size, rect.height())),
                        Rect::from_min_size(
                            pos2(rect.left() + size, rect.top()),
                            vec2(gap, rect.height()),
                        ),
                        Rect::from_min_max(pos2(rect.left() + size + gap, rect.top()), rect.max),
                    )
                };
                dividers.push(Divider {
                    path: path.clone(),
                    rect: line,
                    stacked: *stacked,
                    area: rect,
                    ratio: *ratio,
                });
                path.push(false);
                first.place(a, gap, path, rects, dividers);
                path.pop();
                path.push(true);
                second.place(b, gap, path, rects, dividers);
                path.pop();
            }
        }
    }
}

/// Width of the area that grabs a divider, whatever the gap drawn between the two sides.
const GRAB: f32 = 10.0;

impl Divider {
    /// The ratio after dragging the divider by `delta` (along its axis): it follows the pointer
    /// from where it was grabbed, without jumping to it.
    pub fn ratio_moved(&self, delta: f32, gap: f32) -> f32 {
        let length = if self.stacked {
            self.area.height()
        } else {
            self.area.width()
        };
        (self.ratio + delta / (length - gap).max(1.0)).clamp(MIN_RATIO, 1.0 - MIN_RATIO)
    }

    /// Draws the divider as a thin line, `idle` or `active` while the pointer is on it, and
    /// lets it be dragged; a double click gives both sides the same size. Returns the new ratio.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        gap: f32,
        idle: Color32,
        active: Color32,
    ) -> Option<f32> {
        let grab = if self.stacked {
            Rect::from_center_size(self.rect.center(), vec2(self.rect.width(), GRAB.max(gap)))
        } else {
            Rect::from_center_size(self.rect.center(), vec2(GRAB.max(gap), self.rect.height()))
        };
        let response = ui.interact(grab, id, Sense::click_and_drag());
        let hot = response.hovered() || response.dragged();
        if hot {
            ui.ctx().set_cursor_icon(if self.stacked {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
        }
        let thickness = if hot { 2.0 } else { 1.0 };
        let line = if self.stacked {
            Rect::from_center_size(self.rect.center(), vec2(self.rect.width(), thickness))
        } else {
            Rect::from_center_size(self.rect.center(), vec2(thickness, self.rect.height()))
        };
        if hot || idle != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(line, 0.0, if hot { active } else { idle });
        }
        if response.double_clicked() {
            return Some(0.5);
        }
        let response = response.on_hover_text(crate::i18n::t("reset_split"));
        if response.dragged() {
            let delta = response.drag_delta();
            let delta = if self.stacked { delta.y } else { delta.x };
            if delta != 0.0 {
                return Some(self.ratio_moved(delta, gap));
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "../test/terminal_layout/tests.rs"]
mod tests;
