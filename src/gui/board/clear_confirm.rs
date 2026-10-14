use gpui_kit::{
    Context, Window,
    component::{WindowExt, button::ButtonVariant, dialog::DialogButtonProps},
};
use rust_i18n::t;

use super::{RopyBoard, filtering::ClearConfirmAction};

pub(super) fn open(
    action: ClearConfirmAction,
    window: &mut Window,
    cx: &mut Context<'_, RopyBoard>,
) {
    let owner = cx.entity().downgrade();
    window.open_alert_dialog(cx, move |dialog, _, _cx| {
        let (title, message) = match action {
            ClearConfirmAction::AllHistory => {
                (t!("clear_confirm_title"), t!("clear_confirm_message"))
            }
            ClearConfirmAction::OrdinaryRecords => (
                t!("clear_ordinary_confirm_title"),
                t!("clear_ordinary_confirm_message"),
            ),
        };
        let confirm_owner = owner.clone();
        let close_owner = owner.clone();
        dialog
            .confirm()
            .title(title)
            .description(message)
            .button_props(
                DialogButtonProps::default()
                    .ok_variant(ButtonVariant::Danger)
                    .ok_text(t!("clear_confirm_button"))
                    .cancel_text(t!("clear_confirm_cancel")),
            )
            .on_ok(move |_, _, cx| {
                let _ = confirm_owner.update(cx, |this, cx| {
                    this.confirm_clear_action(cx);
                });
                true
            })
            .on_close(move |_, _, cx| {
                let _ = close_owner.update(cx, |this, cx| {
                    this.ui_state.clear_confirm = super::ClearConfirmState::Hidden;
                    cx.notify();
                });
            })
    });
}
