mod scrollable;

pub use scrollable::*;
pub use wgpui_base::AutoScroll;
pub use wgpui_base::ScrollableMask;
pub use wgpui_base::{
    Scrollbar, ScrollbarAxis, ScrollbarEntrance, ScrollbarHandle, ScrollbarMode, ScrollbarMotion,
    ScrollbarStyles, ScrollbarThumbStyle, ScrollbarTrackStyle,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scrollbar_is_the_base_type() {
        fn accepts_base(_: wgpui_base::Scrollbar) {}
        fn accepts_handle(_: impl wgpui_base::ScrollbarHandle) {}

        let handle = wgpui::ScrollHandle::default();
        let scrollbar: crate::scroll::Scrollbar = Scrollbar::vertical(&handle).styles(|styles| {
            styles
                .track(|style| style.bg(wgpui::transparent_black()))
                .thumb(|style| style.bg(wgpui::transparent_black()))
                .thumb_hover(|style| style.bg(wgpui::transparent_black()))
                .thumb_active(|style| style.bg(wgpui::transparent_black()))
        });
        accepts_base(scrollbar);
        accepts_handle(handle);
    }
}
