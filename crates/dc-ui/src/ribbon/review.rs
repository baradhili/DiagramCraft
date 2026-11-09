//! Review tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(
            "Proofing",
            vec![
                big(
                    RibbonButton::new(Icon::Spell, "Spelling", Action::Macros)
                        .enabled(false)
                        .tip("Spell check — planned"),
                ),
                stack(vec![RibbonButton::new(
                    Icon::FontA,
                    "Language",
                    Action::Macros,
                )
                .enabled(false)
                .tip("Language settings — planned")]),
            ],
        ),
        Group::new(
            "Comments",
            vec![big(
                RibbonButton::new(Icon::Comment, "New Comment", Action::Macros)
                    .enabled(false)
                    .tip("Comments — planned"),
            )],
        ),
    ]
}
