//! Invite Dialog Component
//!
//! Shows incoming invite with accept/deny options.

use crate::services::PizzaInvite;
use dioxus::prelude::*;

#[component]
pub fn InviteDialog(
    invite: PizzaInvite,
    on_accept: EventHandler<PizzaInvite>,
    on_deny: EventHandler<PizzaInvite>,
) -> Element {
    let invite_for_accept = invite.clone();
    let invite_for_deny = invite.clone();

    rsx! {
        div {
            class: "modal-overlay",
            onclick: {
                let invite = invite_for_deny.clone();
                move |_| on_deny.call(invite.clone())
            },

            div {
                class: "modal",
                onclick: |e| e.stop_propagation(),

                div { class: "modal-header",
                    h3 { "You've Been Invited!" }
                }

                div { class: "modal-body",
                    div { class: "invite-details",
                        div { class: "invite-icon",
                            "🍕"
                        }
                        h4 { "{invite.order_name}" }
                        p { class: "invite-meta",
                            "Invited by {invite.inviter_name}"
                        }
                        p { class: "invite-description",
                            "You've been invited to join this pizza order. Accept to add your order to the group."
                        }
                    }
                }

                div { class: "modal-footer",
                    button {
                        class: "btn btn-outline",
                        onclick: {
                            let invite = invite_for_deny.clone();
                            move |_| on_deny.call(invite.clone())
                        },
                        "Decline"
                    }
                    button {
                        class: "btn btn-primary",
                        onclick: {
                            let invite = invite_for_accept.clone();
                            move |_| on_accept.call(invite.clone())
                        },
                        "Accept Invite"
                    }
                }
            }
        }
    }
}

/// Component showing pending invites badge/button
#[component]
pub fn PendingInvitesBadge(
    count: usize,
    on_click: EventHandler<()>,
) -> Element {
    if count == 0 {
        return rsx! {};
    }

    rsx! {
        button {
            class: "pending-invites-badge",
            onclick: move |_| on_click.call(()),
            span { class: "badge-icon", "✉️" }
            span { class: "badge-count", "{count}" }
        }
    }
}
