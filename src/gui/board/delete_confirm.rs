use gpui_kit::{
    Context, Window,
    component::{WindowExt, button::ButtonVariant, dialog::DialogButtonProps},
};

use super::RopyBoard;
use crate::i18n::I18n;

pub(super) fn open(window: &mut Window, cx: &mut Context<'_, RopyBoard>) {
    let owner = cx.entity().downgrade();
    window.open_alert_dialog(cx, move |dialog, _, cx| {
        let confirm_owner = owner.clone();
        let close_owner = owner.clone();
        dialog
            .confirm()
            .title(I18n::translate(cx, "delete_confirm_title"))
            .description(I18n::translate(cx, "delete_confirm_message"))
            .button_props(
                DialogButtonProps::default()
                    .ok_variant(ButtonVariant::Danger)
                    .ok_text(I18n::translate(cx, "delete_confirm_button"))
                    .cancel_text(I18n::translate(cx, "delete_confirm_cancel")),
            )
            .on_ok(move |_, _, cx| {
                let _ = confirm_owner.update(cx, |this, cx| {
                    this.confirm_pending_delete(cx);
                });
                true
            })
            .on_close(move |_, _, cx| {
                let _ = close_owner.update(cx, |this, cx| {
                    this.cancel_pending_delete(cx);
                });
            })
    });
}
