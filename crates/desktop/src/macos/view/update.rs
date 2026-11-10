use super::*;
use crate::update::{self, Control};

impl Desktop {
    pub(super) fn update_row(&self, theme: Theme, cx: &mut Context<Self>) -> Option<Div> {
        let notice = self.update_notice()?;
        let mut text = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .child(label(notice.text, theme));
        if let Some(caption) = notice.caption {
            text = text.child(
                div()
                    .text_size(px(11.))
                    .line_height(px(17.))
                    .text_color(theme.muted)
                    .child(caption),
            );
        }
        let row = div()
            .w_full()
            .min_h(px(40.))
            .flex_none()
            .px(px(16.))
            .py(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.line)
            .child(text);
        Some(match notice.control {
            Some(Control::RestartToUpdate) => row.child(
                button("update-restart", "Restart to update", true, true, theme)
                    .flex_none()
                    .on_click(cx.listener(|app, _, _, cx| app.restart_to_update(cx))),
            ),
            Some(Control::ReleasePage) => row.child(
                button("update-release-page", "Release page", true, false, theme)
                    .flex_none()
                    .on_click(cx.listener(|_, _, _, cx| cx.open_url(update::RELEASES_PAGE))),
            ),
            None => row,
        })
    }
}
