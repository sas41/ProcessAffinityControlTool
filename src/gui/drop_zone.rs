use iced::advanced::{
    layout, mouse, overlay, renderer,
    widget::{tree, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Background, Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Vector};

/// Drag-and-drop target wrapper for any child element.
///
/// External DnD state is passed in via `dropping`:
/// - `None`: idle, acts exactly like the wrapped content.
/// - `Some(name)`: drag is active, show affordance and accept drop.
///
/// On left-button release inside bounds, this emits `on_drop(name)` and captures
/// the event so the wrapped child does not also handle that release.
pub struct DropZone<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    // `'a` is a lifetime parameter (roughly: how long borrowed data must stay valid).
    content: Element<'a, Message, Theme, Renderer>,
    dropping: Option<String>, // `Option<T>` = nullable-like enum: `Some(value)` or `None`.
    // `Box<dyn Fn...>` is a heap-allocated trait object (runtime-polymorphic callback).
    on_drop: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message, Theme, Renderer> DropZone<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>, // `impl Trait` = any type implementing this trait.
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
    // `where` lists generic constraints (similar to C# `where T : ...`).
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
    // The wrapper has no local state; all DnD context comes from props.
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        // Iced widgets store per-child state in a parallel `Tree`.
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
        layout: Layout<'_>, // `'_` = inferred/anonymous lifetime.
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        // Render child first, then paint drop affordance over it.
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

            // Avoid invalid bounds that can panic tiny-skia.
            if bounds.width > 0.0 && bounds.height > 0.0 {
                let over = cursor.is_over(bounds);

                // Hovered: stronger border + tint. Not hovered: subtle outline
                // to indicate this region is still a valid drop target.
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
        // Event flow:
        // 1) Intercept only a completed drop gesture (left release in bounds).
        // 2) Emit one message and capture the event.
        // 3) Otherwise, forward everything to wrapped content unchanged.
        // `if let` pattern-matches one case and skips the rest.
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if let (Some(name), Some(on_drop)) = (&self.dropping, &self.on_drop) {
                if cursor.is_over(layout.bounds()) {
                    shell.publish(on_drop(name.clone()));
                    shell.capture_event();
                    return;
                }
            }
        }

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
        // Cursor feedback mirrors drop availability during active drag.
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
        // Keep child overlays (tooltips/popups) working through the wrapper.
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
