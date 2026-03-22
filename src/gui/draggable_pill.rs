// Draggable pill wrapper for any child widget.
//
// Behavior overview (for non-Rust readers):
// - Press starts a potential drag and records the cursor origin.
// - Drag starts only after moving past `DRAG_DEADBAND` pixels.
// - While dragging, the original pill is dimmed in place and a ghost pill is drawn in an overlay.

use iced::advanced::{
    layout, mouse, overlay, renderer,
    widget::{tree, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
};

const DRAG_DEADBAND: f32 = 10.0;

#[derive(Default, Clone)]
struct State {
    // Per-widget transient input state kept by iced's widget tree.
    pressed: bool,
    // `Option<T>` is Rust's null-safe maybe type (`Some(value)` or `None`).
    // Cursor position where left press began (None when idle).
    origin: Option<Point>,
    // Latest cursor position from move events.
    cursor: Point,
    // Ensures `on_drag_start` is published once per drag gesture.
    drag_fired: bool,
}

// Visual drag preview drawn above normal content.
struct GhostOverlay {
    pill_size: Size,
    cursor: Point,
    grab_offset: Vector,
}

impl<Message, Theme, Renderer: iced::advanced::Renderer> overlay::Overlay<Message, Theme, Renderer>
    for GhostOverlay
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        let top_left = self.cursor - self.grab_offset;
        layout::Node::new(self.pill_size).move_to(top_left)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();

        if bounds.width > 0.0 && bounds.height > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        color: Color::from_rgba(0.0, 0.6, 1.0, 0.95),
                        width: 1.0,
                        radius: (bounds.height / 2.0).into(),
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                Background::Color(Color::from_rgba(0.05, 0.35, 0.75, 0.80)),
            );
        }
    }
}

// `'a` is a lifetime parameter: how long borrowed data inside this type must stay valid.
pub struct DraggablePill<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_drag_start: Option<Message>,
}

impl<'a, Message, Theme, Renderer> DraggablePill<'a, Message, Theme, Renderer> {
    /// Creates a draggable wrapper around `content`.
    ///
    /// `on_drag_start` is emitted once when movement crosses the deadband.
    pub fn new(
        // `impl Into<T>` means "any type that can be converted into T".
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_drag_start: Message,
    ) -> Self {
        Self {
            content: content.into(),
            on_drag_start: Some(on_drag_start),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<DraggablePill<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    // `where` adds generic constraints (similar to C# `where T : ...`).
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(pill: DraggablePill<'a, Message, Theme, Renderer>) -> Self {
        Element::new(pill)
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DraggablePill<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        // `&T` is a shared borrow; `&mut T` is an exclusive mutable borrow.
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        // `'_` asks Rust to infer this lifetime parameter.
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        if state.pressed {
            if let Some(origin) = state.origin {
                if state.cursor.distance(origin) > DRAG_DEADBAND {
                    self.content.as_widget().draw(
                        &tree.children[0],
                        renderer,
                        theme,
                        style,
                        layout,
                        cursor,
                        viewport,
                    );

                    let b = layout.bounds();

                    if b.width > 0.0 && b.height > 0.0 {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: b,
                                border: Border::default(),
                                shadow: Shadow::default(),
                                snap: false,
                            },
                            Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.40)),
                        );
                    }

                    return;
                }
            }
        }

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        // `dyn Trait` is a trait object (runtime-dispatched interface value).
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        // `match` is an exhaustive pattern match expression (like a strict `switch`).
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(layout.bounds()) {
                    state.pressed = true;
                    state.origin = Some(pos);
                    state.cursor = pos;
                    state.drag_fired = false;
                    // Capture keeps this gesture routed to this widget until release.
                    shell.capture_event();
                    return;
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                state.cursor = *position;

                if state.pressed && !state.drag_fired {
                    if let Some(origin) = state.origin {
                        if position.distance(origin) > DRAG_DEADBAND {
                            state.drag_fired = true;

                            // Message is cloned because publishing takes ownership.
                            if let Some(msg) = self.on_drag_start.clone() {
                                shell.publish(msg);
                            }
                        }
                    }
                }

                return;
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.pressed {
                    state.pressed = false;
                    state.origin = None;
                    state.drag_fired = false;
                    return;
                }
            }

            _ => {}
        }

        if !state.pressed {
            // Child widget handles normal events only when we are not in an active press.
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            )
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.pressed {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Grab
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<State>();

        // Overlay appears only after the drag threshold is crossed.
        if state.pressed {
            if let Some(origin) = state.origin {
                if state.cursor.distance(origin) > DRAG_DEADBAND {
                    let bounds = layout.bounds();
                    let grab_offset = origin - bounds.position();
                    let ghost = GhostOverlay {
                        pill_size: bounds.size(),
                        cursor: state.cursor,
                        grab_offset,
                    };

                    return Some(overlay::Element::new(Box::new(ghost)));
                }
            }
        }

        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
