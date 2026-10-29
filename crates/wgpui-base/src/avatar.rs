use smallvec::SmallVec;
use wgpui::{
    AnyElement, App, Div, ImageSource, InteractiveElement, Interactivity, IntoElement,
    ParentElement, RenderOnce, StyleRefinement, Styled, Window, div, img,
};

use crate::StyledExt as _;

/// An unstyled avatar root that renders its image slot or fallback slot.
#[derive(IntoElement)]
pub struct Avatar {
    base: Div,
    style: StyleRefinement,
    image: Option<AvatarImage>,
    fallback: Option<AvatarFallback>,
}

impl Avatar {
    pub fn new() -> Self {
        Self {
            base: div(),
            style: StyleRefinement::default(),
            image: None,
            fallback: None,
        }
    }

    pub fn image(mut self, image: AvatarImage) -> Self {
        self.image = Some(image);
        self
    }

    pub fn fallback(mut self, fallback: AvatarFallback) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Avatar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Avatar {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Avatar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let content = self
            .image
            .map(IntoElement::into_any_element)
            .or_else(|| self.fallback.map(IntoElement::into_any_element));
        self.base.children(content).refine_style(&self.style)
    }
}

/// An unstyled avatar image slot.
#[derive(IntoElement)]
pub struct AvatarImage {
    image: wgpui::Img,
    style: StyleRefinement,
}

impl AvatarImage {
    pub fn new(source: impl Into<ImageSource>) -> Self {
        Self {
            image: img(source),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for AvatarImage {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for AvatarImage {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.image.interactivity()
    }
}

impl RenderOnce for AvatarImage {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.image.refine_style(&self.style)
    }
}

/// An unstyled avatar fallback slot for initials or an application-owned icon.
#[derive(IntoElement)]
pub struct AvatarFallback {
    base: Div,
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 1]>,
}

impl AvatarFallback {
    pub fn new() -> Self {
        Self {
            base: div(),
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
    }
}

impl Default for AvatarFallback {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for AvatarFallback {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for AvatarFallback {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for AvatarFallback {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.base.children(self.children).refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpui::{Context, Render, prelude::FluentBuilder as _, px};

    struct Harness {
        image: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Avatar::new()
                .when(self.image, |this| {
                    this.image(
                        AvatarImage::new("avatar.png")
                            .debug_selector(|| "avatar-image".into())
                            .size(px(20.)),
                    )
                })
                .fallback(
                    AvatarFallback::new().child(
                        div()
                            .debug_selector(|| "avatar-fallback".into())
                            .size(px(20.)),
                    ),
                )
        }
    }

    #[wgpui::test]
    fn image_slot_takes_precedence_over_fallback(cx: &mut wgpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness { image: true });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("avatar-image").is_some());
        assert!(cx.debug_bounds("avatar-fallback").is_none());
    }

    #[wgpui::test]
    fn fallback_renders_without_an_image(cx: &mut wgpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness { image: false });
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(cx.debug_bounds("avatar-image").is_none());
        assert!(cx.debug_bounds("avatar-fallback").is_some());
    }
}
