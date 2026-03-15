use iced::advanced::{
    graphics::core::event,
    layout, mouse, overlay, renderer,
    widget::{tree, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Background, Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Vector};

// ─── DropZone ─────────────────────────────────────────────────────────────────

/// Wraps any element and turns it into a drag-and-drop target.
///
/// - When `dropping` is `Some(name)`, a visual highlight is drawn:
///   - A subtle border when cursor is not over the zone (shows it's droppable).
///   - A bright border + tint when cursor is over the zone (ready to accept).
/// - On `ButtonReleased` while `dropping.is_some()` and cursor is over bounds,
///   fires `on_drop(name)` and captures the event.
pub struct DropZone<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    dropping: Option<String>,
    on_drop: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message, Theme, Renderer> DropZone<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        dropping: Option<String>,
        on_drop: impl Fn(String) -> Message + 'a,
    ) -> Self {
        Self {
            content: content.into(),
            dropping,
            on_drop: Some(Box::new(on_drop)),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<DropZone<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(zone: DropZone<'a, Message, Theme, Renderer>) -> Self {
        Element::new(zone)
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DropZone<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
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
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );

        if self.dropping.is_some() {
            let bounds = layout.bounds();
            // Guard against zero/NaN/infinite bounds that would panic tiny-skia.
            if bounds.width > 0.0 && bounds.height > 0.0 {
                let over = cursor.is_over(bounds);
                if over {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: Border {
                                color: Color::from_rgba(0.0, 0.58, 1.0, 1.0),
                                width: 2.0,
                                radius: 5.0.into(),
                            },
                            shadow: Shadow::default(),
                            snap: false,
                        },
                        Background::Color(Color::from_rgba(0.0, 0.45, 0.9, 0.12)),
                    );
                } else {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: Border {
                                color: Color::from_rgba(0.55, 0.55, 0.55, 0.5),
                                width: 1.0,
                                radius: 5.0.into(),
                            },
                            shadow: Shadow::default(),
                            snap: false,
                        },
                        Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.0)),
                    );
                }
            }
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Intercept release only when a drag is active and cursor is over us.
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if let (Some(name), Some(on_drop)) = (&self.dropping, &self.on_drop) {
                if cursor.is_over(layout.bounds()) {
                    shell.publish(on_drop(name.clone()));
                    shell.capture_event();
                    return;
                }
            }
        }

        // Forward everything else to the inner content as normal.
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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.dropping.is_some() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Copy
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
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}
