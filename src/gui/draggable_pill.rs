use iced::advanced::{
    Clipboard, Layout, Shell, Widget,
    graphics::core::event,
    layout, mouse, overlay, renderer,
    widget::{Tree, tree},
};
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
};

const DRAG_DEADBAND: f32 = 10.0;

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
struct State {
    /// Mouse is currently held down on this pill.
    pressed: bool,
    /// Screen position where the mouse pressed.
    origin: Option<Point>,
    /// Latest known cursor position (updated every CursorMoved).
    cursor: Point,
    /// Whether we have already fired the on_drag_start message.
    drag_fired: bool,
}

// ─── Ghost overlay ────────────────────────────────────────────────────────────

struct GhostOverlay {
    pill_size: Size,
    cursor: Point,
    grab_offset: Vector, // cursor - pill.top_left at the moment of press
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
                },
                Background::Color(Color::from_rgba(0.05, 0.35, 0.75, 0.80)),
            );
        }
    }

    /// Never block the cursor from reaching widgets below — the ghost is visual only.
    fn is_over(&self, _layout: Layout<'_>, _renderer: &Renderer, _cursor: Point) -> bool {
        false
    }
}

// ─── DraggablePill ────────────────────────────────────────────────────────────

/// Wraps any element and makes it draggable. After the cursor moves more than
/// `DRAG_DEADBAND` pixels from the press point, fires `on_drag_start` and
/// shows a ghost pill via an overlay (which escapes any parent clipping).
pub struct DraggablePill<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_drag_start: Option<Message>,
}

impl<'a, Message, Theme, Renderer> DraggablePill<'a, Message, Theme, Renderer> {
    pub fn new(
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
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        // When dragging (past deadband), draw the original pill dimmed.
        if state.pressed {
            if let Some(origin) = state.origin {
                if state.cursor.distance(origin) > DRAG_DEADBAND {
                    // Draw pill dimmed to show it's "lifted"
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

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        let state = tree.state.downcast_mut::<State>();

        match &event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(layout.bounds()) {
                    state.pressed = true;
                    state.origin = Some(pos);
                    state.cursor = pos;
                    state.drag_fired = false;
                    return event::Status::Captured;
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                state.cursor = *position;

                // Fire on_drag_start exactly once, after crossing the deadband.
                if state.pressed && !state.drag_fired {
                    if let Some(origin) = state.origin {
                        if position.distance(origin) > DRAG_DEADBAND {
                            state.drag_fired = true;
                            if let Some(msg) = self.on_drag_start.clone() {
                                shell.publish(msg);
                            }
                        }
                    }
                }
                // Always Ignored so other widgets still see cursor movement.
                return event::Status::Ignored;
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.pressed {
                    state.pressed = false;
                    state.origin = None;
                    state.drag_fired = false;
                    // Ignored: let DropZone and the subscription also handle this.
                    return event::Status::Ignored;
                }
            }

            _ => {}
        }

        // Forward other events to inner content when not mid-drag.
        if !state.pressed {
            self.content.as_widget_mut().on_event(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            )
        } else {
            event::Status::Ignored
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
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<State>();

        if state.pressed {
            if let Some(origin) = state.origin {
                if state.cursor.distance(origin) > DRAG_DEADBAND {
                    let bounds = layout.bounds();
                    // grab_offset = where in the pill the user pressed
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

        // Not dragging — delegate to inner content's overlay (e.g. dropdowns).
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}
