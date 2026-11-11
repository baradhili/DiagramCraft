// This file is generated. Do not edit it manually.
use fluent_typed::prelude::*;
use std::{fmt::Display, ops::Deref, slice::Iter, str::FromStr};

static LANG_DATA: &[u8] = include_bytes!("../gen/translations.ftl");
/// The message contract of every generated accessor: what an `.ftl` must
/// define for this API. Pass it to `fluent_typed::validate_ftl` to check a
/// translation without loading it (e.g. in CI).
#[allow(dead_code)]
pub static MESSAGE_CONTRACTS: &[MessageContract] = &[
    MessageContract {
        message: "a-ruler-subdivision",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "a-shape-intersection",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "about-name",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "about-tarahi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "active",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "activex-era-likely-cut-scope-cal",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-a-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-connection-point-next-ix-on",
        attribute: None,
        vars: &["next_ix", "hit"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-in-hosting-arrives-with-m8",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-in-manager",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-in-manager-m8",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-ins",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-name-prop-row-test",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-to-new-container",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-to-quick-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "add-to-underlying-container",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "added-to-quick-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-against-the-page-instead-o",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-bottom",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-bottom-edges",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-bottom-kind",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-center",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-center-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-horizontal-middles",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-left",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-left-edges-first-selected",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-left-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-right",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-right-edges",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-right-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-top",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-top-edges",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-top-kind",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "align-vertical-centers",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "aligning-to-the-first-selected-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "aligning-to-the-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "alignment-and-spacing-hints-whil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "alignment-boxes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "all-changes-saved",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "all-pages-page-count",
        attribute: None,
        vars: &["page_count"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "already-aligned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "applies-to-the-ribbon-menus-and",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "apply",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "apply-custom-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "arrange",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "arrange-all",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "arrange-windows-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "assign-a-background-page-behind",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "auto-layout-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "auto-page-size-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "auto-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "autoconnect",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "autoconnect-arrows-the-108-remai",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "autoconnect-needs-a-2d-shape",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "autocorrect-options",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "autocorrect-options-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "automatic-resize",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "automatically-link",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "background",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "background-page-name",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "backgrounds",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "behavior",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "blank-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bold",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bold-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "boolean-shape-operations-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "border-title",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "borders-titles-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bring-forward",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bring-forward-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bring-to-front",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "browse",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "browse-for-an-existing-visio-pac",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bullets",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "bullets-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cad-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cad-drawing-import-planned-later",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "calibri",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "callout",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "callouts-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cancel",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "center",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "chart",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "charts-planned-later-milestone",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "checking-the-clipboard-for-a-pic",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "choose-the-display-language-for",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "classic-arrow",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clear",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clear-planned-later-milestone",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "click-a-shape-to-add-a-connectio",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clipboard",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clipboard-image-could-not-be-enc",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clipboard-planned-for-a-later-mi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "clipboard-probe-failed",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "close",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "close-group",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "close-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "closed-switched-to-name",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "closed-title",
        attribute: None,
        vars: &["title"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "code",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "color",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "colors",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "com-add-ins",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comment-add",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comment-add-inline",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comment-new",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comments",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comments-on-shape-target-count",
        attribute: None,
        vars: &["target", "count"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "comments-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "configure-snap-and-glue-targets",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "conn-point",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "conn-points",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connection-point-ix-is-inherited",
        attribute: None,
        vars: &["ix"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connection-point-ix-removed",
        attribute: None,
        vars: &["ix"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connection-point-ix-selected-del",
        attribute: None,
        vars: &["ix"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connection-point-next-ix-added-t",
        attribute: None,
        vars: &["next_ix", "hit"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connection-point-tool-off",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-end-released",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-is-glued-at-both-ends",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-move-refused-reason",
        attribute: None,
        vars: &["reason"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-moved-release-ends-glu",
        attribute: None,
        vars: &["release_ends"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-tool-drag-from-one-sha",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-tool-status",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connector-tool-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connectors",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "connpoint-tool-status",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container-margins-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container-style-style-applied",
        attribute: None,
        vars: &["style"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container-style-style-not-found",
        attribute: None,
        vars: &["style"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container-styles",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "container-tools",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "containers",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "containers-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "containers-wrap-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "contgal-prompt",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "controls",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy-as-picture",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy-as-picture-failed-e",
        attribute: None,
        vars: &["e"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy-as-picture-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy-picture",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "copy-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "could-not-open-address-e",
        attribute: None,
        vars: &["address", "e"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "create-a-background-page-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "create-a-stencil-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "create-new-data-graphic",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cross-functional",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "crow-s-foot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "crow-s-foot-tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "current",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "current-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "curve-control-customized-row-row",
        attribute: None,
        vars: &["row_ix"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "curved-connector",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "custom",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "custom-import",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cut",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "cut-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dash",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dash-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dash-dot-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "data",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "data-graphics",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "data-graphics-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "data-linking-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "date-time",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "date-time-field-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "decrease-indent",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "decrease-indent-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "define-shape-data",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "define-shape-data-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "delete",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "delete-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "delete-comment-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "delete-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "design",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "design-mode",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "design-mode-likely-cut-scope-cal",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "developer",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "diagram-design",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "diagram-parts",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "diamond-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "diamond-ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "disband-container",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "discard-new",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "display-data-graphics",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "display-language",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "display-options",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dist-h",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dist-v",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "distribute-horizontal-spacing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "distribute-needs-three-or-more-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "distribute-vertical-spacing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "document-closed",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "document-inspection-arrives-with",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "document-protection-arrives-with",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "document-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "document-stencil-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "don-t-save",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "double-arrow-filled",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "double-arrow-open",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "double-tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "double-tick-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "double-tick-ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "drawing-aids",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "drawing-explorer",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "drawing-explorer-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "drawing-scale-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "drawings-you-open-will-appear-he",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "duplicate",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "duplicate-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "dynamic-grid",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-data-graphic",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-hyperlink",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-text",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "edit-the-selected-shape-s-hyperl",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "editing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "editing-text-of-shape-shape-id",
        attribute: None,
        vars: &["shape_id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "effects",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ellipse-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ellipse-tool-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "endpoint-drag-refused-reason",
        attribute: None,
        vars: &["reason"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "endpoint-moved-but-name-is-formu",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "even-horizontal-spacing-3-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "even-vertical-spacing-3-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "eventdblclick-f-macro-and-naviga",
        attribute: None,
        vars: &["f"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-entry-desc",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-failed-worker-lost",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-finished",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-page-sub",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-save-as-label",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "export-start",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "exporting",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "external-data",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "external-data-window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "external-data-window-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "extracting-built-in-stencils-to",
        attribute: None,
        vars: &["done", "total"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fields-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "file",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "files-you-open-will-appear-here",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fill",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "filled-arrow-plain",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "filled-circle",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "filled-square",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "find",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "find-planned-later-milestone",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fine-dash",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fine-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "first-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fit-the-page-width-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fit-the-whole-page-in-the-window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fit-to-contents",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "flip-horizontal",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "flip-vertical",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "floor-plan",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "flowchart",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-decrease",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-decrease-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-family-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-increase",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-increase-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "font-size-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "foreground",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "fork-ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-painter",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-painter-cancelled",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-painter-click-a-shape-to",
        attribute: None,
        vars: &["sid"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-painter-click-shapes-to-a",
        attribute: None,
        vars: &["sid"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-shape",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-shape-fill",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-shape-line",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-shape-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "format-shape-text",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "freeform",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "freeform-drag-on-the-page-to-dra",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "freeform-off",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "freeform-shape-id",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "freeform-stroke-too-short-to-dra",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "glue",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "glue-to",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "go",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "gradient-horiz",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "gradient-radial",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "gradient-vert",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "grid",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "group",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "group-needs-2-shapes-on-the-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "group-the-selection-one-undo-ste",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "grow-with-the-members-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "guide-id",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "guides",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "guides-planned-113-s-objects-exi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "heading-style",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "heading-style-gallery-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "help-about",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "high-192-dpi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "highlight-members",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "home",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "hyperlink",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "hyperlink-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "hyperlink-new",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "hyperlink-pages",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "hyperlinks",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "illustrations",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "increase-indent",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "increase-indent-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "info",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "inherited",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "inherited-from-the-shape-s-maste",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "insert",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "insert-a-png-or-jpeg-picture-fro",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "insert-control",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "insert-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "insert-pages",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "inspect-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "inspect-the-selected-shape-s-cel",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "italic",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "italic-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "jpg",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "jump-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "justify",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "justify-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "label-a-choice-cannot-contain",
        attribute: None,
        vars: &["label"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "label-dialog-planned-for-a-later",
        attribute: None,
        vars: &["label"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "label-not-a-number",
        attribute: None,
        vars: &["label"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "landscape",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "language",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "language-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "language-name",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "language-settings-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lasso-select",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lasso-select-drag-a-freehand-tra",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lasso-select-off",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lasso-selected-n-shape-s",
        attribute: None,
        vars: &["n"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "last-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layer",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout-options",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout-options-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout-size-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "layout-spacing-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "legend",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "legends-m6-visio-paints-legend-o",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-color-label",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-jumps",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-jumps-hidden",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-jumps-shown",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-move-refused-reason",
        attribute: None,
        vars: &["reason"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-moved-but-name-is-formula-d",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-tool-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "line-weight",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "link-data-to-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "links",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "location",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lock-container",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lock-is-set-protection-section",
        attribute: None,
        vars: &["lock"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "lock-the-container-against-membe",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "long-arrow",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "long-dash",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "macro-security",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "macro-security-m7",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "macros",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "macros-arrive-with-m7-vba-vsdm-m",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "manage-reports",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "margins",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "markup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "markup-tracking-planned-no-marku",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "master",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "master-mid",
        attribute: None,
        vars: &["mid"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "member-highlight-off",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "member-highlight-on",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "membership",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "metric-stencils",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "middle",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "mm-in",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "more-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "move-container-and-members",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "move-down",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "move-line-endpoint-shape-shape-i",
        attribute: None,
        vars: &["shape_id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "move-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "move-up",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "name",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "name-page-id",
        attribute: None,
        vars: &["name", "id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "name-u-master-id",
        attribute: None,
        vars: &["name_u", "id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "network-diagram",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-background-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-blank-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-blank-drawing-unsaved",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-choice",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "new-window-planned-single-window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "next-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "next-comment-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "next-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-background",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-background-pages-right-click",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-comments-on-this-shape-yet",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-document-open",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-hyperlinks-on-this-shape",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-keytip-key",
        attribute: None,
        vars: &["key"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-matching-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-picture-on-the-clipboard",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "no-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "none",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nothing-on-this-page-matches-the",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nothing-selected-to-copy",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nothing-to-redo",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nothing-to-undo",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nudge-selection",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "numbering",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "numbering-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "nurbs-control-customized-the-e-f",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ok",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "online-pictures",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "online-pictures-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "only-png-and-jpeg-pictures-are-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-2-prong",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-a-visio-file",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-barbed",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-circle",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-diamond",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-double-chevron-large",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-existing-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-failed-e",
        attribute: None,
        vars: &["e"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-failed-the-load-worker-died",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-file-cta",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-file-title",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-fork",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-group-label",
        attribute: None,
        vars: &["group_label"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-square",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-triangle-convex-base",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-triangle-large",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-triangle-medium",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-triangle-medium-large",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-triangle-small",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-v-tall",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-vsdx-vstx-vssx-ctrl-o",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "open-wide",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "opened-stencil-stem",
        attribute: None,
        vars: &["stem"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "operations",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "options",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "options-arrive-with-a-later-mile",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "org-chart",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "orientation",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "output-file",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-breaks",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-breaks-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-name",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-name-is-a-foreground-page-a",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-name-marked-as-a-background",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-number",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-number-field-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-setup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-setup-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-size-iso-ansi-presets-or-cu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "page-width",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "paint-formatting-onto-shape-to",
        attribute: None,
        vars: &["to"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "painter-cells",
        attribute: None,
        vars: &["n", "to"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "painter-cells-sticky",
        attribute: None,
        vars: &["n", "to"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "paragraph",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "paste",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "pick-an-output-file-first",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "pictures",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "pointer-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "pointer-tool-select-and-drag-esc",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "portrait",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "presentation-mode",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "presentation-mode-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "preset-sizes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "previous-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "previous-comment-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "previous-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "print-384-dpi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "print-stub",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "print-the-current-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "printing-arrives-with-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "proofing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "protect-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "protection",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "quick-import",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "quick-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "quick-styles",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rail-export",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rail-new",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rail-print",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "re-layout-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "recent",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rectangle-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rectangle-tool-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "redo-ctrl-y",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "redo-l",
        attribute: None,
        vars: &["l"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "refresh-all",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "release-the-members-and-delete-t",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "remove",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "remove-background-mark",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "remove-connection-point-ix",
        attribute: None,
        vars: &["ix"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "remove-from-quick-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "removed-from-quick-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rename-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reorder-pages",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reorder-pages-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "replace",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "replace-planned-later-milestone",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reply",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "report-definitions-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reports",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "resize-refused-shape-is-protecte",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "resolution",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "resolution-dpi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "review",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "review-new-comment",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reviewing-pane",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "reviewing-pane-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "right-angle-connector",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ring-tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-left",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-left-90",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-right",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-right-90",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-the-selection-90-clockwis",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rotate-the-selection-90-counter",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rounding",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ruler-subdivisions",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "rulers",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-as",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-changes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-changes-to-name",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-ctrl-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-new",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "save-the-stencil-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "scale",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "screen-96-dpi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "search-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "search-templates",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-a-container-to-disband",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-a-format",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-a-group-to-ungroup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-a-shape-to-copy-formattin",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-a-shape-to-edit-its-hyper",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-all",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-by",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-by-type",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-by-type-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-by-type-pick-a-criterion",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-contents",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-every-shape-on-the-page-c",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-shapes-by-type-master-or",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-the-container-s-members-p",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-the-shapes-to-contain-fir",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "select-two-or-more-shapes-to-ali",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "selected-all-shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "selected-shape-shape-id",
        attribute: None,
        vars: &["shape_id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "selection",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "selection-pane",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "selection-pane-planned-112-shell",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "send-backward",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "send-backward-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "send-the-drawing-to-a-printer-or",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "send-to-back",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "set-as-background",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "set-connector-route-style",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "set-page-size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "set-proofing-language",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "set-proofing-language-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-behavior-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-data",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-data-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-data-window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-data-window-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-design",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-dup-id-connected-to-sid-au",
        attribute: None,
        vars: &["dup_id", "sid"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-effects-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-extensions",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-fill-color",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-handles",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-id-s-alignment-box",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-id-s-connection-point",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-id-s-extension",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-id-s-handle",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-id-s-vertex",
        attribute: None,
        vars: &["id"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-intersections",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-protection-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-report",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-reports-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-sid",
        attribute: None,
        vars: &["sid"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-styles",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-text",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-type",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shape-vertices",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shapes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shapesheet",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "shapesheet-name",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "share",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "share-a-copy",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "share-a-copy-of-the-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "share-a-copy-of-the-drawing-with",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "sharing-arrives-with-m6",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "short-dash",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-connection-points-planned-m",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-document-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-hide",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-hide-comments",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-hide-comments-planned-view",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-hide-data-graphics-on-the-p",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-line-jumps",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-markup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-markup-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "show-shapesheet",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "size",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "size-cell-is-formula-driven-f",
        attribute: None,
        vars: &["size_cell", "f"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "size-pt",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "size-the-container-to-its-member",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "slanted-tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "small-arrow",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "snap",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "snap-conn-points",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "snap-dynamic-grid",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "snap-glue",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "snap-to",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "solid",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "spacing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "spell-check-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "spelling",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "splash-wordmark",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "split",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "split-view-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stamp-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stamp-tool-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "start-an-empty-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "start-an-empty-drawing-on-a-lett",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "start-an-empty-us-letter-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "status",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stealth-dart",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil-editing-planned-scope-ca",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil-fallback",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil-settings",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil-settings-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "stencil-stem-is-already-open-exp",
        attribute: None,
        vars: &["stem"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "straight-connector",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "strikethrough",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "strikethrough-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "style",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "style-gallery-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "sub-address-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "suggested-flowchart-network-floo",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "svg-copied-n-bytes-vector-paste",
        attribute: None,
        vars: &["n"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "swap-portrait-landscape-swaps-wi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "switch-documents-the-tab-strip-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "switch-windows",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "switched-to-name",
        attribute: None,
        vars: &["name"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "symbol",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "symbol-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tarahi-v-cargo-pkg-version-clean",
        attribute: None,
        vars: &["version"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "task-panes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "task-panes-shapes-pan-zoom-plann",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tell-me",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tell-me-planned-for-a-later-mile",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tell-me-search-arrives-with-a-la",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tell-me-what-you-want-to-do",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "template-gallery-arrives-with-th",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-align-left",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-align-right",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-box",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-color",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-color-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-field",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-formatting-arrives-with-a-l",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-shapes-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-tool",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "text-tool-planned-double-click-e",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-current-drawing-has-unsaved",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-grid",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-last-page-cannot-be-deleted",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-link-has-no-address",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-picture-could-not-be-decoded",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-picture-lands-on-the-clipboa",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "the-svg-markup-lands-on-the-clip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "theme-variants-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "themes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "themes-planned-gallery-folds-bro",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "thesaurus",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "thesaurus-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "this-page-is-itself-a-background",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "this-page-renders-behind",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tick-chevron",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tick-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tick-double-chevron",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tick-ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tint-the-container-s-members-whi",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "title",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "title-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "to-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "toggle-display-units-model-stays",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "toggle-member-highlight",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "tools",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "track-markup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "translate",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "translate-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "transparent-background",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "triple-tick",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "triple-tick-dot",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "triple-tick-ring",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "underline",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "underline-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "undo-ctrl-z",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "undo-l",
        attribute: None,
        vars: &["l"],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ungroup",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "ungroup-the-selected-group-s",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "unsaved-changes",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "untitled",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "use-relative-paths-for-hyperlink",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "value",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "variant-colors-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "variant-connectors-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "variant-effects-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "variants",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "vba-arrives-with-m7-vsdm",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "vector-svg",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "view",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "views",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "visio-drawing",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "visio-stencil",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "visio-vsdx-family",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "visual-aids",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "visual-basic",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "weight",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "whole-page",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "wide-arrow",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "window",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "word-count",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "word-count-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "wrap-the-selection-in-this-conta",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "x-cancel-placement",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-dialog-planned",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-in",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-in-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-menu",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-out",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-out-tip",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
    MessageContract {
        message: "zoom-to-selection-shift-z",
        attribute: None,
        vars: &[],
        bool_vars: &[],
        elements: &[],
    },
];
#[allow(dead_code)]
static ALL_LANGS: [L10n; 2] = [
    // languages as an array
    L10n::De,
    L10n::En,
];

/// The languages that have translations available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum L10n {
    De,
    En,
}

impl Default for L10n {
    fn default() -> Self {
        Self::En
    }
}

impl FromStr for L10n {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "de" => Ok(Self::De),
            "en" => Ok(Self::En),
            _ => Err(format!("Unknown language: {}", s)),
        }
    }
}

impl Deref for L10n {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::De => "de",
            Self::En => "en",
        }
    }
}

impl AsRef<str> for L10n {
    fn as_ref(&self) -> &str {
        self
    }
}

impl Display for L10n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.deref())
    }
}

// Language helpers: not every app uses all of them, and an unused one is not a
// stale translation, so they don't produce dead-code warnings.
#[allow(dead_code)]
impl L10n {
    /// All languages with built-in translations.
    pub fn iter() -> Iter<'static, L10n> {
        ALL_LANGS.iter()
    }

    /// The language name as defined in the ftl message "language-name".
    pub fn language_name(&self) -> &'static str {
        match self {
            Self::De => "Deutsch",
            Self::En => "English",
        }
    }

    fn byte_range(&self) -> std::ops::Range<usize> {
        match self {
            Self::De => 0..35473,
            Self::En => 35473..69513,
        }
    }
    /// Load a L10nLanguage from the embedded data.
    pub fn load(&self) -> L10nLanguage {
        L10nLanguage::new(self, &LANG_DATA[self.byte_range()])
            .expect("fluent-typed: the embedded .ftl could not be loaded. This is a build-time bug; please report it.")
    }

    /// Load all languages (L10nLanguage) from the embedded data.
    pub fn load_all() -> L10nLanguageVec {
        L10nLanguageVec::load_without_isolation(
            LANG_DATA,
            Self::iter().map(|lang| (lang, lang.byte_range())),
        )
        .expect("fluent-typed: the embedded .ftl could not be loaded. This is a build-time bug; please report it.")
    }
}

/// A thin wrapper around the Fluent messages for one language.
///
/// It provides functions for each message that was found in
/// all the languages at build time.
#[derive(Debug)]
pub struct L10nLanguage(L10nBundle);

impl L10nLanguage {
    /// Load the L10n resources for the given language. The language
    /// has to be a valid LanguageIdentifier or otherwise
    /// an error is returned.
    ///
    /// The bytes are expected to be the contents of a .ftl file. An owned
    /// buffer (`Vec<u8>`, `String`) is moved in without copying; a borrowed
    /// one (`&[u8]`, `&str`) is copied once.
    #[allow(dead_code)]
    pub fn new(lang: impl AsRef<str>, bytes: impl Into<Vec<u8>>) -> Result<Self, L10nError> {
        Ok(Self(L10nBundle::new_without_isolation(lang, bytes)?))
    }

    /// Load external `.ftl` bytes — read from disk, downloaded, etc. — after
    /// validating them against the message contract compiled into this API.
    ///
    /// A translation where a message is missing or misspelled, uses a variable
    /// this API never passes, drops an `(Element)` marker, references a
    /// message or term that doesn't exist, or calls a function other than
    /// `NUMBER()` is rejected here with `L10nError::Validation` (listing every
    /// violation) instead of panicking later in an accessor.
    ///
    /// Use this to load a language that was not compiled in, or to hot-reload
    /// a translation under revision. Pass the buffer by value (e.g. straight
    /// from `std::fs::read`) to avoid a copy.
    #[allow(dead_code)]
    pub fn new_external(
        lang: impl AsRef<str>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, L10nError> {
        let bytes = bytes.into();
        validate_ftl(&bytes, MESSAGE_CONTRACTS)?;
        Self::new(lang, bytes)
    }

    pub fn msg_a_ruler_subdivision(&self) -> String {
        self.0.msg("a-ruler-subdivision", None).unwrap()
    }
    pub fn msg_a_shape_intersection(&self) -> String {
        self.0.msg("a-shape-intersection", None).unwrap()
    }
    pub fn msg_about_name(&self) -> String {
        self.0.msg("about-name", None).unwrap()
    }
    pub fn msg_about_tarahi(&self) -> String {
        self.0.msg("about-tarahi", None).unwrap()
    }
    pub fn msg_active(&self) -> String {
        self.0.msg("active", None).unwrap()
    }
    pub fn msg_activex_era_likely_cut_scope_cal(&self) -> String {
        self.0
            .msg("activex-era-likely-cut-scope-cal", None)
            .unwrap()
    }
    pub fn msg_add(&self) -> String {
        self.0.msg("add", None).unwrap()
    }
    pub fn msg_add_a_comment(&self) -> String {
        self.0.msg("add-a-comment", None).unwrap()
    }
    /// $next_ix (Number) - argument
    /// $hit (Number) - argument
    pub fn msg_add_connection_point_next_ix_on<F0: Into<FluentNumber>, F1: Into<FluentNumber>>(
        &self,
        next_ix: F0,
        hit: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("next_ix", next_ix.into());
        _args.set("hit", hit.into());
        self.0
            .msg("add-connection-point-next-ix-on", Some(_args))
            .unwrap()
    }
    pub fn msg_add_in_hosting_arrives_with_m8(&self) -> String {
        self.0.msg("add-in-hosting-arrives-with-m8", None).unwrap()
    }
    pub fn msg_add_in_manager(&self) -> String {
        self.0.msg("add-in-manager", None).unwrap()
    }
    pub fn msg_add_in_manager_m8(&self) -> String {
        self.0.msg("add-in-manager-m8", None).unwrap()
    }
    pub fn msg_add_ins(&self) -> String {
        self.0.msg("add-ins", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_add_name_prop_row_test<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0.msg("add-name-prop-row-test", Some(_args)).unwrap()
    }
    pub fn msg_add_to_new_container(&self) -> String {
        self.0.msg("add-to-new-container", None).unwrap()
    }
    pub fn msg_add_to_quick_shapes(&self) -> String {
        self.0.msg("add-to-quick-shapes", None).unwrap()
    }
    pub fn msg_add_to_underlying_container(&self) -> String {
        self.0.msg("add-to-underlying-container", None).unwrap()
    }
    pub fn msg_added_to_quick_shapes(&self) -> String {
        self.0.msg("added-to-quick-shapes", None).unwrap()
    }
    pub fn msg_align_against_the_page_instead_o(&self) -> String {
        self.0
            .msg("align-against-the-page-instead-o", None)
            .unwrap()
    }
    pub fn msg_align_bottom(&self) -> String {
        self.0.msg("align-bottom", None).unwrap()
    }
    pub fn msg_align_bottom_edges(&self) -> String {
        self.0.msg("align-bottom-edges", None).unwrap()
    }
    pub fn msg_align_bottom_kind(&self) -> String {
        self.0.msg("align-bottom-kind", None).unwrap()
    }
    pub fn msg_align_center(&self) -> String {
        self.0.msg("align-center", None).unwrap()
    }
    pub fn msg_align_center_planned(&self) -> String {
        self.0.msg("align-center-planned", None).unwrap()
    }
    pub fn msg_align_horizontal_middles(&self) -> String {
        self.0.msg("align-horizontal-middles", None).unwrap()
    }
    pub fn msg_align_left(&self) -> String {
        self.0.msg("align-left", None).unwrap()
    }
    pub fn msg_align_left_edges_first_selected(&self) -> String {
        self.0.msg("align-left-edges-first-selected", None).unwrap()
    }
    pub fn msg_align_left_planned(&self) -> String {
        self.0.msg("align-left-planned", None).unwrap()
    }
    pub fn msg_align_right(&self) -> String {
        self.0.msg("align-right", None).unwrap()
    }
    pub fn msg_align_right_edges(&self) -> String {
        self.0.msg("align-right-edges", None).unwrap()
    }
    pub fn msg_align_right_planned(&self) -> String {
        self.0.msg("align-right-planned", None).unwrap()
    }
    pub fn msg_align_top(&self) -> String {
        self.0.msg("align-top", None).unwrap()
    }
    pub fn msg_align_top_edges(&self) -> String {
        self.0.msg("align-top-edges", None).unwrap()
    }
    pub fn msg_align_top_kind(&self) -> String {
        self.0.msg("align-top-kind", None).unwrap()
    }
    pub fn msg_align_vertical_centers(&self) -> String {
        self.0.msg("align-vertical-centers", None).unwrap()
    }
    pub fn msg_aligning_to_the_first_selected_s(&self) -> String {
        self.0
            .msg("aligning-to-the-first-selected-s", None)
            .unwrap()
    }
    pub fn msg_aligning_to_the_page(&self) -> String {
        self.0.msg("aligning-to-the-page", None).unwrap()
    }
    pub fn msg_alignment_and_spacing_hints_whil(&self) -> String {
        self.0
            .msg("alignment-and-spacing-hints-whil", None)
            .unwrap()
    }
    pub fn msg_alignment_boxes(&self) -> String {
        self.0.msg("alignment-boxes", None).unwrap()
    }
    pub fn msg_all_changes_saved(&self) -> String {
        self.0.msg("all-changes-saved", None).unwrap()
    }
    /// $page_count (Number) - argument
    pub fn msg_all_pages_page_count<F0: Into<FluentNumber>>(&self, page_count: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("page_count", page_count.into());
        self.0.msg("all-pages-page-count", Some(_args)).unwrap()
    }
    pub fn msg_already_aligned(&self) -> String {
        self.0.msg("already-aligned", None).unwrap()
    }
    pub fn msg_applies_to_the_ribbon_menus_and(&self) -> String {
        self.0.msg("applies-to-the-ribbon-menus-and", None).unwrap()
    }
    pub fn msg_apply(&self) -> String {
        self.0.msg("apply", None).unwrap()
    }
    pub fn msg_apply_custom_size(&self) -> String {
        self.0.msg("apply-custom-size", None).unwrap()
    }
    pub fn msg_arrange(&self) -> String {
        self.0.msg("arrange", None).unwrap()
    }
    pub fn msg_arrange_all(&self) -> String {
        self.0.msg("arrange-all", None).unwrap()
    }
    pub fn msg_arrange_windows_planned(&self) -> String {
        self.0.msg("arrange-windows-planned", None).unwrap()
    }
    pub fn msg_assign_a_background_page_behind(&self) -> String {
        self.0.msg("assign-a-background-page-behind", None).unwrap()
    }
    pub fn msg_auto_layout_planned(&self) -> String {
        self.0.msg("auto-layout-planned", None).unwrap()
    }
    pub fn msg_auto_page_size_planned(&self) -> String {
        self.0.msg("auto-page-size-planned", None).unwrap()
    }
    pub fn msg_auto_size(&self) -> String {
        self.0.msg("auto-size", None).unwrap()
    }
    pub fn msg_autoconnect(&self) -> String {
        self.0.msg("autoconnect", None).unwrap()
    }
    pub fn msg_autoconnect_arrows_the_108_remai(&self) -> String {
        self.0
            .msg("autoconnect-arrows-the-108-remai", None)
            .unwrap()
    }
    pub fn msg_autoconnect_needs_a_2d_shape(&self) -> String {
        self.0.msg("autoconnect-needs-a-2d-shape", None).unwrap()
    }
    pub fn msg_autocorrect_options(&self) -> String {
        self.0.msg("autocorrect-options", None).unwrap()
    }
    pub fn msg_autocorrect_options_planned(&self) -> String {
        self.0.msg("autocorrect-options-planned", None).unwrap()
    }
    pub fn msg_automatic_resize(&self) -> String {
        self.0.msg("automatic-resize", None).unwrap()
    }
    pub fn msg_automatically_link(&self) -> String {
        self.0.msg("automatically-link", None).unwrap()
    }
    pub fn msg_background(&self) -> String {
        self.0.msg("background", None).unwrap()
    }
    pub fn msg_background_page_name<'a, F0: Into<FluentValue<'a>>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name);
        self.0.msg("background-page-name", Some(_args)).unwrap()
    }
    pub fn msg_backgrounds(&self) -> String {
        self.0.msg("backgrounds", None).unwrap()
    }
    pub fn msg_behavior(&self) -> String {
        self.0.msg("behavior", None).unwrap()
    }
    pub fn msg_blank_drawing(&self) -> String {
        self.0.msg("blank-drawing", None).unwrap()
    }
    pub fn msg_bold(&self) -> String {
        self.0.msg("bold", None).unwrap()
    }
    pub fn msg_bold_planned(&self) -> String {
        self.0.msg("bold-planned", None).unwrap()
    }
    pub fn msg_boolean_shape_operations_planned(&self) -> String {
        self.0
            .msg("boolean-shape-operations-planned", None)
            .unwrap()
    }
    pub fn msg_border_title(&self) -> String {
        self.0.msg("border-title", None).unwrap()
    }
    pub fn msg_borders_titles_planned(&self) -> String {
        self.0.msg("borders-titles-planned", None).unwrap()
    }
    pub fn msg_bring_forward(&self) -> String {
        self.0.msg("bring-forward", None).unwrap()
    }
    pub fn msg_bring_forward_planned(&self) -> String {
        self.0.msg("bring-forward-planned", None).unwrap()
    }
    pub fn msg_bring_to_front(&self) -> String {
        self.0.msg("bring-to-front", None).unwrap()
    }
    pub fn msg_browse(&self) -> String {
        self.0.msg("browse", None).unwrap()
    }
    pub fn msg_browse_for_an_existing_visio_pac(&self) -> String {
        self.0
            .msg("browse-for-an-existing-visio-pac", None)
            .unwrap()
    }
    pub fn msg_bullets(&self) -> String {
        self.0.msg("bullets", None).unwrap()
    }
    pub fn msg_bullets_planned(&self) -> String {
        self.0.msg("bullets-planned", None).unwrap()
    }
    pub fn msg_cad_drawing(&self) -> String {
        self.0.msg("cad-drawing", None).unwrap()
    }
    pub fn msg_cad_drawing_import_planned_later(&self) -> String {
        self.0
            .msg("cad-drawing-import-planned-later", None)
            .unwrap()
    }
    pub fn msg_calibri(&self) -> String {
        self.0.msg("calibri", None).unwrap()
    }
    pub fn msg_callout(&self) -> String {
        self.0.msg("callout", None).unwrap()
    }
    pub fn msg_callouts_planned(&self) -> String {
        self.0.msg("callouts-planned", None).unwrap()
    }
    pub fn msg_cancel(&self) -> String {
        self.0.msg("cancel", None).unwrap()
    }
    pub fn msg_center(&self) -> String {
        self.0.msg("center", None).unwrap()
    }
    pub fn msg_chart(&self) -> String {
        self.0.msg("chart", None).unwrap()
    }
    pub fn msg_charts_planned_later_milestone(&self) -> String {
        self.0.msg("charts-planned-later-milestone", None).unwrap()
    }
    pub fn msg_checking_the_clipboard_for_a_pic(&self) -> String {
        self.0
            .msg("checking-the-clipboard-for-a-pic", None)
            .unwrap()
    }
    pub fn msg_choose_the_display_language_for(&self) -> String {
        self.0.msg("choose-the-display-language-for", None).unwrap()
    }
    pub fn msg_classic_arrow(&self) -> String {
        self.0.msg("classic-arrow", None).unwrap()
    }
    pub fn msg_clear(&self) -> String {
        self.0.msg("clear", None).unwrap()
    }
    pub fn msg_clear_planned_later_milestone(&self) -> String {
        self.0.msg("clear-planned-later-milestone", None).unwrap()
    }
    pub fn msg_click_a_shape_to_add_a_connectio(&self) -> String {
        self.0
            .msg("click-a-shape-to-add-a-connectio", None)
            .unwrap()
    }
    pub fn msg_clipboard(&self) -> String {
        self.0.msg("clipboard", None).unwrap()
    }
    pub fn msg_clipboard_image_could_not_be_enc(&self) -> String {
        self.0
            .msg("clipboard-image-could-not-be-enc", None)
            .unwrap()
    }
    pub fn msg_clipboard_planned_for_a_later_mi(&self) -> String {
        self.0
            .msg("clipboard-planned-for-a-later-mi", None)
            .unwrap()
    }
    pub fn msg_clipboard_probe_failed(&self) -> String {
        self.0.msg("clipboard-probe-failed", None).unwrap()
    }
    pub fn msg_close(&self) -> String {
        self.0.msg("close", None).unwrap()
    }
    pub fn msg_close_group(&self) -> String {
        self.0.msg("close-group", None).unwrap()
    }
    pub fn msg_close_stencil(&self) -> String {
        self.0.msg("close-stencil", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_closed_switched_to_name<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0.msg("closed-switched-to-name", Some(_args)).unwrap()
    }
    /// $title (String) - argument
    pub fn msg_closed_title<F0: AsRef<str>>(&self, title: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("title", title.as_ref());
        self.0.msg("closed-title", Some(_args)).unwrap()
    }
    pub fn msg_code(&self) -> String {
        self.0.msg("code", None).unwrap()
    }
    pub fn msg_color(&self) -> String {
        self.0.msg("color", None).unwrap()
    }
    pub fn msg_colors(&self) -> String {
        self.0.msg("colors", None).unwrap()
    }
    pub fn msg_com_add_ins(&self) -> String {
        self.0.msg("com-add-ins", None).unwrap()
    }
    pub fn msg_comment_add(&self) -> String {
        self.0.msg("comment-add", None).unwrap()
    }
    pub fn msg_comment_add_inline(&self) -> String {
        self.0.msg("comment-add-inline", None).unwrap()
    }
    pub fn msg_comment_new(&self) -> String {
        self.0.msg("comment-new", None).unwrap()
    }
    pub fn msg_comments(&self) -> String {
        self.0.msg("comments", None).unwrap()
    }
    /// $target (Number) - argument
    /// $count (Number) - argument
    pub fn msg_comments_on_shape_target_count<F0: Into<FluentNumber>, F1: Into<FluentNumber>>(
        &self,
        target: F0,
        count: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("target", target.into());
        _args.set("count", count.into());
        self.0
            .msg("comments-on-shape-target-count", Some(_args))
            .unwrap()
    }
    pub fn msg_comments_planned(&self) -> String {
        self.0.msg("comments-planned", None).unwrap()
    }
    pub fn msg_configure_snap_and_glue_targets(&self) -> String {
        self.0.msg("configure-snap-and-glue-targets", None).unwrap()
    }
    pub fn msg_conn_point(&self) -> String {
        self.0.msg("conn-point", None).unwrap()
    }
    pub fn msg_conn_points(&self) -> String {
        self.0.msg("conn-points", None).unwrap()
    }
    /// $ix (Number) - argument
    pub fn msg_connection_point_ix_is_inherited<F0: Into<FluentNumber>>(&self, ix: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("ix", ix.into());
        self.0
            .msg("connection-point-ix-is-inherited", Some(_args))
            .unwrap()
    }
    /// $ix (Number) - argument
    pub fn msg_connection_point_ix_removed<F0: Into<FluentNumber>>(&self, ix: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("ix", ix.into());
        self.0
            .msg("connection-point-ix-removed", Some(_args))
            .unwrap()
    }
    /// $ix (Number) - argument
    pub fn msg_connection_point_ix_selected_del<F0: Into<FluentNumber>>(&self, ix: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("ix", ix.into());
        self.0
            .msg("connection-point-ix-selected-del", Some(_args))
            .unwrap()
    }
    pub fn msg_connection_point_next_ix_added_t<
        'a,
        F0: Into<FluentValue<'a>>,
        F1: Into<FluentValue<'a>>,
    >(
        &self,
        next_ix: F0,
        hit: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("next_ix", next_ix);
        _args.set("hit", hit);
        self.0
            .msg("connection-point-next-ix-added-t", Some(_args))
            .unwrap()
    }
    pub fn msg_connection_point_tool_off(&self) -> String {
        self.0.msg("connection-point-tool-off", None).unwrap()
    }
    pub fn msg_connector(&self) -> String {
        self.0.msg("connector", None).unwrap()
    }
    pub fn msg_connector_end_released(&self) -> String {
        self.0.msg("connector-end-released", None).unwrap()
    }
    pub fn msg_connector_is_glued_at_both_ends(&self) -> String {
        self.0.msg("connector-is-glued-at-both-ends", None).unwrap()
    }
    /// $reason (String) - argument
    pub fn msg_connector_move_refused_reason<F0: AsRef<str>>(&self, reason: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("reason", reason.as_ref());
        self.0
            .msg("connector-move-refused-reason", Some(_args))
            .unwrap()
    }
    /// $release_ends (Number) - argument
    pub fn msg_connector_moved_release_ends_glu<F0: Into<FluentNumber>>(
        &self,
        release_ends: F0,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("release_ends", release_ends.into());
        self.0
            .msg("connector-moved-release-ends-glu", Some(_args))
            .unwrap()
    }
    pub fn msg_connector_tool_drag_from_one_sha(&self) -> String {
        self.0
            .msg("connector-tool-drag-from-one-sha", None)
            .unwrap()
    }
    pub fn msg_connector_tool_status(&self) -> String {
        self.0.msg("connector-tool-status", None).unwrap()
    }
    pub fn msg_connector_tool_tip(&self) -> String {
        self.0.msg("connector-tool-tip", None).unwrap()
    }
    pub fn msg_connectors(&self) -> String {
        self.0.msg("connectors", None).unwrap()
    }
    pub fn msg_connpoint_tool_status(&self) -> String {
        self.0.msg("connpoint-tool-status", None).unwrap()
    }
    pub fn msg_container(&self) -> String {
        self.0.msg("container", None).unwrap()
    }
    pub fn msg_container_margins_planned(&self) -> String {
        self.0.msg("container-margins-planned", None).unwrap()
    }
    /// $style (String) - argument
    pub fn msg_container_style_style_applied<F0: AsRef<str>>(&self, style: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("style", style.as_ref());
        self.0
            .msg("container-style-style-applied", Some(_args))
            .unwrap()
    }
    /// $style (String) - argument
    pub fn msg_container_style_style_not_found<F0: AsRef<str>>(&self, style: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("style", style.as_ref());
        self.0
            .msg("container-style-style-not-found", Some(_args))
            .unwrap()
    }
    pub fn msg_container_styles(&self) -> String {
        self.0.msg("container-styles", None).unwrap()
    }
    pub fn msg_container_tools(&self) -> String {
        self.0.msg("container-tools", None).unwrap()
    }
    pub fn msg_containers(&self) -> String {
        self.0.msg("containers", None).unwrap()
    }
    pub fn msg_containers_planned(&self) -> String {
        self.0.msg("containers-planned", None).unwrap()
    }
    pub fn msg_containers_wrap_tip(&self) -> String {
        self.0.msg("containers-wrap-tip", None).unwrap()
    }
    pub fn msg_contgal_prompt(&self) -> String {
        self.0.msg("contgal-prompt", None).unwrap()
    }
    pub fn msg_controls(&self) -> String {
        self.0.msg("controls", None).unwrap()
    }
    pub fn msg_copy(&self) -> String {
        self.0.msg("copy", None).unwrap()
    }
    pub fn msg_copy_as_picture(&self) -> String {
        self.0.msg("copy-as-picture", None).unwrap()
    }
    /// $e (String) - argument
    pub fn msg_copy_as_picture_failed_e<F0: AsRef<str>>(&self, e: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("e", e.as_ref());
        self.0.msg("copy-as-picture-failed-e", Some(_args)).unwrap()
    }
    pub fn msg_copy_as_picture_menu(&self) -> String {
        self.0.msg("copy-as-picture-menu", None).unwrap()
    }
    pub fn msg_copy_picture(&self) -> String {
        self.0.msg("copy-picture", None).unwrap()
    }
    pub fn msg_copy_planned(&self) -> String {
        self.0.msg("copy-planned", None).unwrap()
    }
    /// $address (String) - argument
    /// $e (String) - argument
    pub fn msg_could_not_open_address_e<F0: AsRef<str>, F1: AsRef<str>>(
        &self,
        address: F0,
        e: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("address", address.as_ref());
        _args.set("e", e.as_ref());
        self.0.msg("could-not-open-address-e", Some(_args)).unwrap()
    }
    pub fn msg_create_a_background_page_planned(&self) -> String {
        self.0
            .msg("create-a-background-page-planned", None)
            .unwrap()
    }
    pub fn msg_create_a_stencil_planned(&self) -> String {
        self.0.msg("create-a-stencil-planned", None).unwrap()
    }
    pub fn msg_create_new_data_graphic(&self) -> String {
        self.0.msg("create-new-data-graphic", None).unwrap()
    }
    pub fn msg_cross_functional(&self) -> String {
        self.0.msg("cross-functional", None).unwrap()
    }
    pub fn msg_crow_s_foot(&self) -> String {
        self.0.msg("crow-s-foot", None).unwrap()
    }
    pub fn msg_crow_s_foot_tick(&self) -> String {
        self.0.msg("crow-s-foot-tick", None).unwrap()
    }
    pub fn msg_current(&self) -> String {
        self.0.msg("current", None).unwrap()
    }
    pub fn msg_current_page(&self) -> String {
        self.0.msg("current-page", None).unwrap()
    }
    /// $row_ix (Number) - argument
    pub fn msg_curve_control_customized_row_row<F0: Into<FluentNumber>>(
        &self,
        row_ix: F0,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("row_ix", row_ix.into());
        self.0
            .msg("curve-control-customized-row-row", Some(_args))
            .unwrap()
    }
    pub fn msg_curved_connector(&self) -> String {
        self.0.msg("curved-connector", None).unwrap()
    }
    pub fn msg_custom(&self) -> String {
        self.0.msg("custom", None).unwrap()
    }
    pub fn msg_custom_import(&self) -> String {
        self.0.msg("custom-import", None).unwrap()
    }
    pub fn msg_cut(&self) -> String {
        self.0.msg("cut", None).unwrap()
    }
    pub fn msg_cut_planned(&self) -> String {
        self.0.msg("cut-planned", None).unwrap()
    }
    pub fn msg_dash(&self) -> String {
        self.0.msg("dash", None).unwrap()
    }
    pub fn msg_dash_dot(&self) -> String {
        self.0.msg("dash-dot", None).unwrap()
    }
    pub fn msg_dash_dot_dot(&self) -> String {
        self.0.msg("dash-dot-dot", None).unwrap()
    }
    pub fn msg_data(&self) -> String {
        self.0.msg("data", None).unwrap()
    }
    pub fn msg_data_graphics(&self) -> String {
        self.0.msg("data-graphics", None).unwrap()
    }
    pub fn msg_data_graphics_m6(&self) -> String {
        self.0.msg("data-graphics-m6", None).unwrap()
    }
    pub fn msg_data_linking_m6(&self) -> String {
        self.0.msg("data-linking-m6", None).unwrap()
    }
    pub fn msg_date_time(&self) -> String {
        self.0.msg("date-time", None).unwrap()
    }
    pub fn msg_date_time_field_planned(&self) -> String {
        self.0.msg("date-time-field-planned", None).unwrap()
    }
    pub fn msg_decrease_indent(&self) -> String {
        self.0.msg("decrease-indent", None).unwrap()
    }
    pub fn msg_decrease_indent_planned(&self) -> String {
        self.0.msg("decrease-indent-planned", None).unwrap()
    }
    pub fn msg_define_shape_data(&self) -> String {
        self.0.msg("define-shape-data", None).unwrap()
    }
    pub fn msg_define_shape_data_m6(&self) -> String {
        self.0.msg("define-shape-data-m6", None).unwrap()
    }
    pub fn msg_delete(&self) -> String {
        self.0.msg("delete", None).unwrap()
    }
    pub fn msg_delete_comment(&self) -> String {
        self.0.msg("delete-comment", None).unwrap()
    }
    pub fn msg_delete_comment_planned(&self) -> String {
        self.0.msg("delete-comment-planned", None).unwrap()
    }
    pub fn msg_delete_page(&self) -> String {
        self.0.msg("delete-page", None).unwrap()
    }
    pub fn msg_design(&self) -> String {
        self.0.msg("design", None).unwrap()
    }
    pub fn msg_design_mode(&self) -> String {
        self.0.msg("design-mode", None).unwrap()
    }
    pub fn msg_design_mode_likely_cut_scope_cal(&self) -> String {
        self.0
            .msg("design-mode-likely-cut-scope-cal", None)
            .unwrap()
    }
    pub fn msg_developer(&self) -> String {
        self.0.msg("developer", None).unwrap()
    }
    pub fn msg_diagram_design(&self) -> String {
        self.0.msg("diagram-design", None).unwrap()
    }
    pub fn msg_diagram_parts(&self) -> String {
        self.0.msg("diagram-parts", None).unwrap()
    }
    pub fn msg_diamond_dot(&self) -> String {
        self.0.msg("diamond-dot", None).unwrap()
    }
    pub fn msg_diamond_ring(&self) -> String {
        self.0.msg("diamond-ring", None).unwrap()
    }
    pub fn msg_disband_container(&self) -> String {
        self.0.msg("disband-container", None).unwrap()
    }
    pub fn msg_discard_new(&self) -> String {
        self.0.msg("discard-new", None).unwrap()
    }
    pub fn msg_display_data_graphics(&self) -> String {
        self.0.msg("display-data-graphics", None).unwrap()
    }
    pub fn msg_display_language(&self) -> String {
        self.0.msg("display-language", None).unwrap()
    }
    pub fn msg_display_options(&self) -> String {
        self.0.msg("display-options", None).unwrap()
    }
    pub fn msg_dist_h(&self) -> String {
        self.0.msg("dist-h", None).unwrap()
    }
    pub fn msg_dist_v(&self) -> String {
        self.0.msg("dist-v", None).unwrap()
    }
    pub fn msg_distribute_horizontal_spacing(&self) -> String {
        self.0.msg("distribute-horizontal-spacing", None).unwrap()
    }
    pub fn msg_distribute_needs_three_or_more_s(&self) -> String {
        self.0
            .msg("distribute-needs-three-or-more-s", None)
            .unwrap()
    }
    pub fn msg_distribute_vertical_spacing(&self) -> String {
        self.0.msg("distribute-vertical-spacing", None).unwrap()
    }
    pub fn msg_document_closed(&self) -> String {
        self.0.msg("document-closed", None).unwrap()
    }
    pub fn msg_document_inspection_arrives_with(&self) -> String {
        self.0
            .msg("document-inspection-arrives-with", None)
            .unwrap()
    }
    pub fn msg_document_protection_arrives_with(&self) -> String {
        self.0
            .msg("document-protection-arrives-with", None)
            .unwrap()
    }
    pub fn msg_document_stencil(&self) -> String {
        self.0.msg("document-stencil", None).unwrap()
    }
    pub fn msg_document_stencil_planned(&self) -> String {
        self.0.msg("document-stencil-planned", None).unwrap()
    }
    pub fn msg_don_t_save(&self) -> String {
        self.0.msg("don-t-save", None).unwrap()
    }
    pub fn msg_dot(&self) -> String {
        self.0.msg("dot", None).unwrap()
    }
    pub fn msg_double_arrow_filled(&self) -> String {
        self.0.msg("double-arrow-filled", None).unwrap()
    }
    pub fn msg_double_arrow_open(&self) -> String {
        self.0.msg("double-arrow-open", None).unwrap()
    }
    pub fn msg_double_tick(&self) -> String {
        self.0.msg("double-tick", None).unwrap()
    }
    pub fn msg_double_tick_dot(&self) -> String {
        self.0.msg("double-tick-dot", None).unwrap()
    }
    pub fn msg_double_tick_ring(&self) -> String {
        self.0.msg("double-tick-ring", None).unwrap()
    }
    pub fn msg_drawing_aids(&self) -> String {
        self.0.msg("drawing-aids", None).unwrap()
    }
    pub fn msg_drawing_explorer(&self) -> String {
        self.0.msg("drawing-explorer", None).unwrap()
    }
    pub fn msg_drawing_explorer_planned(&self) -> String {
        self.0.msg("drawing-explorer-planned", None).unwrap()
    }
    pub fn msg_drawing_scale_planned(&self) -> String {
        self.0.msg("drawing-scale-planned", None).unwrap()
    }
    pub fn msg_drawings_you_open_will_appear_he(&self) -> String {
        self.0
            .msg("drawings-you-open-will-appear-he", None)
            .unwrap()
    }
    pub fn msg_duplicate(&self) -> String {
        self.0.msg("duplicate", None).unwrap()
    }
    pub fn msg_duplicate_page(&self) -> String {
        self.0.msg("duplicate-page", None).unwrap()
    }
    pub fn msg_dynamic_grid(&self) -> String {
        self.0.msg("dynamic-grid", None).unwrap()
    }
    pub fn msg_edit(&self) -> String {
        self.0.msg("edit", None).unwrap()
    }
    pub fn msg_edit_comment(&self) -> String {
        self.0.msg("edit-comment", None).unwrap()
    }
    pub fn msg_edit_data_graphic(&self) -> String {
        self.0.msg("edit-data-graphic", None).unwrap()
    }
    pub fn msg_edit_hyperlink(&self) -> String {
        self.0.msg("edit-hyperlink", None).unwrap()
    }
    pub fn msg_edit_stencil(&self) -> String {
        self.0.msg("edit-stencil", None).unwrap()
    }
    pub fn msg_edit_text(&self) -> String {
        self.0.msg("edit-text", None).unwrap()
    }
    pub fn msg_edit_the_selected_shape_s_hyperl(&self) -> String {
        self.0
            .msg("edit-the-selected-shape-s-hyperl", None)
            .unwrap()
    }
    pub fn msg_editing(&self) -> String {
        self.0.msg("editing", None).unwrap()
    }
    /// $shape_id (Number) - argument
    pub fn msg_editing_text_of_shape_shape_id<F0: Into<FluentNumber>>(
        &self,
        shape_id: F0,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("shape_id", shape_id.into());
        self.0
            .msg("editing-text-of-shape-shape-id", Some(_args))
            .unwrap()
    }
    pub fn msg_effects(&self) -> String {
        self.0.msg("effects", None).unwrap()
    }
    pub fn msg_ellipse_tool(&self) -> String {
        self.0.msg("ellipse-tool", None).unwrap()
    }
    pub fn msg_ellipse_tool_planned(&self) -> String {
        self.0.msg("ellipse-tool-planned", None).unwrap()
    }
    /// $reason (String) - argument
    pub fn msg_endpoint_drag_refused_reason<F0: AsRef<str>>(&self, reason: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("reason", reason.as_ref());
        self.0
            .msg("endpoint-drag-refused-reason", Some(_args))
            .unwrap()
    }
    /// $name (String) - argument
    pub fn msg_endpoint_moved_but_name_is_formu<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0
            .msg("endpoint-moved-but-name-is-formu", Some(_args))
            .unwrap()
    }
    pub fn msg_even_horizontal_spacing_3_shapes(&self) -> String {
        self.0
            .msg("even-horizontal-spacing-3-shapes", None)
            .unwrap()
    }
    pub fn msg_even_vertical_spacing_3_shapes(&self) -> String {
        self.0.msg("even-vertical-spacing-3-shapes", None).unwrap()
    }
    /// $f (String) - argument
    pub fn msg_eventdblclick_f_macro_and_naviga<F0: AsRef<str>>(&self, f: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("f", f.as_ref());
        self.0
            .msg("eventdblclick-f-macro-and-naviga", Some(_args))
            .unwrap()
    }
    pub fn msg_export_entry_desc(&self) -> String {
        self.0.msg("export-entry-desc", None).unwrap()
    }
    pub fn msg_export_failed_worker_lost(&self) -> String {
        self.0.msg("export-failed-worker-lost", None).unwrap()
    }
    pub fn msg_export_finished(&self) -> String {
        self.0.msg("export-finished", None).unwrap()
    }
    pub fn msg_export_page_sub(&self) -> String {
        self.0.msg("export-page-sub", None).unwrap()
    }
    pub fn msg_export_save_as_label(&self) -> String {
        self.0.msg("export-save-as-label", None).unwrap()
    }
    pub fn msg_export_start(&self) -> String {
        self.0.msg("export-start", None).unwrap()
    }
    pub fn msg_exporting(&self) -> String {
        self.0.msg("exporting", None).unwrap()
    }
    pub fn msg_external_data(&self) -> String {
        self.0.msg("external-data", None).unwrap()
    }
    pub fn msg_external_data_window(&self) -> String {
        self.0.msg("external-data-window", None).unwrap()
    }
    pub fn msg_external_data_window_m6(&self) -> String {
        self.0.msg("external-data-window-m6", None).unwrap()
    }
    pub fn msg_extracting_built_in_stencils_to<
        'a,
        F0: Into<FluentValue<'a>>,
        F1: Into<FluentValue<'a>>,
    >(
        &self,
        done: F0,
        total: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("done", done);
        _args.set("total", total);
        self.0
            .msg("extracting-built-in-stencils-to", Some(_args))
            .unwrap()
    }
    pub fn msg_fields_planned(&self) -> String {
        self.0.msg("fields-planned", None).unwrap()
    }
    pub fn msg_file(&self) -> String {
        self.0.msg("file", None).unwrap()
    }
    pub fn msg_files_you_open_will_appear_here(&self) -> String {
        self.0.msg("files-you-open-will-appear-here", None).unwrap()
    }
    pub fn msg_fill(&self) -> String {
        self.0.msg("fill", None).unwrap()
    }
    pub fn msg_filled_arrow_plain(&self) -> String {
        self.0.msg("filled-arrow-plain", None).unwrap()
    }
    pub fn msg_filled_circle(&self) -> String {
        self.0.msg("filled-circle", None).unwrap()
    }
    pub fn msg_filled_square(&self) -> String {
        self.0.msg("filled-square", None).unwrap()
    }
    pub fn msg_find(&self) -> String {
        self.0.msg("find", None).unwrap()
    }
    pub fn msg_find_planned_later_milestone(&self) -> String {
        self.0.msg("find-planned-later-milestone", None).unwrap()
    }
    pub fn msg_fine_dash(&self) -> String {
        self.0.msg("fine-dash", None).unwrap()
    }
    pub fn msg_fine_dot(&self) -> String {
        self.0.msg("fine-dot", None).unwrap()
    }
    pub fn msg_first_page(&self) -> String {
        self.0.msg("first-page", None).unwrap()
    }
    pub fn msg_fit_the_page_width_planned(&self) -> String {
        self.0.msg("fit-the-page-width-planned", None).unwrap()
    }
    pub fn msg_fit_the_whole_page_in_the_window(&self) -> String {
        self.0
            .msg("fit-the-whole-page-in-the-window", None)
            .unwrap()
    }
    pub fn msg_fit_to_contents(&self) -> String {
        self.0.msg("fit-to-contents", None).unwrap()
    }
    pub fn msg_flip_horizontal(&self) -> String {
        self.0.msg("flip-horizontal", None).unwrap()
    }
    pub fn msg_flip_vertical(&self) -> String {
        self.0.msg("flip-vertical", None).unwrap()
    }
    pub fn msg_floor_plan(&self) -> String {
        self.0.msg("floor-plan", None).unwrap()
    }
    pub fn msg_flowchart(&self) -> String {
        self.0.msg("flowchart", None).unwrap()
    }
    pub fn msg_font(&self) -> String {
        self.0.msg("font", None).unwrap()
    }
    pub fn msg_font_decrease(&self) -> String {
        self.0.msg("font-decrease", None).unwrap()
    }
    pub fn msg_font_decrease_tip(&self) -> String {
        self.0.msg("font-decrease-tip", None).unwrap()
    }
    pub fn msg_font_family_planned(&self) -> String {
        self.0.msg("font-family-planned", None).unwrap()
    }
    pub fn msg_font_increase(&self) -> String {
        self.0.msg("font-increase", None).unwrap()
    }
    pub fn msg_font_increase_tip(&self) -> String {
        self.0.msg("font-increase-tip", None).unwrap()
    }
    pub fn msg_font_size(&self) -> String {
        self.0.msg("font-size", None).unwrap()
    }
    pub fn msg_font_size_planned(&self) -> String {
        self.0.msg("font-size-planned", None).unwrap()
    }
    pub fn msg_foreground(&self) -> String {
        self.0.msg("foreground", None).unwrap()
    }
    pub fn msg_fork_ring(&self) -> String {
        self.0.msg("fork-ring", None).unwrap()
    }
    pub fn msg_format_painter(&self) -> String {
        self.0.msg("format-painter", None).unwrap()
    }
    pub fn msg_format_painter_cancelled(&self) -> String {
        self.0.msg("format-painter-cancelled", None).unwrap()
    }
    /// $sid (Number) - argument
    pub fn msg_format_painter_click_a_shape_to<F0: Into<FluentNumber>>(&self, sid: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("sid", sid.into());
        self.0
            .msg("format-painter-click-a-shape-to", Some(_args))
            .unwrap()
    }
    /// $sid (Number) - argument
    pub fn msg_format_painter_click_shapes_to_a<F0: Into<FluentNumber>>(&self, sid: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("sid", sid.into());
        self.0
            .msg("format-painter-click-shapes-to-a", Some(_args))
            .unwrap()
    }
    pub fn msg_format_shape(&self) -> String {
        self.0.msg("format-shape", None).unwrap()
    }
    pub fn msg_format_shape_fill(&self) -> String {
        self.0.msg("format-shape-fill", None).unwrap()
    }
    pub fn msg_format_shape_line(&self) -> String {
        self.0.msg("format-shape-line", None).unwrap()
    }
    pub fn msg_format_shape_menu(&self) -> String {
        self.0.msg("format-shape-menu", None).unwrap()
    }
    pub fn msg_format_shape_text(&self) -> String {
        self.0.msg("format-shape-text", None).unwrap()
    }
    pub fn msg_freeform(&self) -> String {
        self.0.msg("freeform", None).unwrap()
    }
    pub fn msg_freeform_drag_on_the_page_to_dra(&self) -> String {
        self.0
            .msg("freeform-drag-on-the-page-to-dra", None)
            .unwrap()
    }
    pub fn msg_freeform_off(&self) -> String {
        self.0.msg("freeform-off", None).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_freeform_shape_id<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("freeform-shape-id", Some(_args)).unwrap()
    }
    pub fn msg_freeform_stroke_too_short_to_dra(&self) -> String {
        self.0
            .msg("freeform-stroke-too-short-to-dra", None)
            .unwrap()
    }
    pub fn msg_glue(&self) -> String {
        self.0.msg("glue", None).unwrap()
    }
    pub fn msg_glue_to(&self) -> String {
        self.0.msg("glue-to", None).unwrap()
    }
    pub fn msg_go(&self) -> String {
        self.0.msg("go", None).unwrap()
    }
    pub fn msg_gradient_horiz(&self) -> String {
        self.0.msg("gradient-horiz", None).unwrap()
    }
    pub fn msg_gradient_radial(&self) -> String {
        self.0.msg("gradient-radial", None).unwrap()
    }
    pub fn msg_gradient_vert(&self) -> String {
        self.0.msg("gradient-vert", None).unwrap()
    }
    pub fn msg_grid(&self) -> String {
        self.0.msg("grid", None).unwrap()
    }
    pub fn msg_group(&self) -> String {
        self.0.msg("group", None).unwrap()
    }
    pub fn msg_group_needs_2_shapes_on_the_page(&self) -> String {
        self.0
            .msg("group-needs-2-shapes-on-the-page", None)
            .unwrap()
    }
    pub fn msg_group_the_selection_one_undo_ste(&self) -> String {
        self.0
            .msg("group-the-selection-one-undo-ste", None)
            .unwrap()
    }
    pub fn msg_grow_with_the_members_planned(&self) -> String {
        self.0.msg("grow-with-the-members-planned", None).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_guide_id<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("guide-id", Some(_args)).unwrap()
    }
    pub fn msg_guides(&self) -> String {
        self.0.msg("guides", None).unwrap()
    }
    pub fn msg_guides_planned_113_s_objects_exi(&self) -> String {
        self.0
            .msg("guides-planned-113-s-objects-exi", None)
            .unwrap()
    }
    pub fn msg_heading_style(&self) -> String {
        self.0.msg("heading-style", None).unwrap()
    }
    pub fn msg_heading_style_gallery_planned(&self) -> String {
        self.0.msg("heading-style-gallery-planned", None).unwrap()
    }
    pub fn msg_help_about(&self) -> String {
        self.0.msg("help-about", None).unwrap()
    }
    pub fn msg_high_192_dpi(&self) -> String {
        self.0.msg("high-192-dpi", None).unwrap()
    }
    pub fn msg_highlight_members(&self) -> String {
        self.0.msg("highlight-members", None).unwrap()
    }
    pub fn msg_home(&self) -> String {
        self.0.msg("home", None).unwrap()
    }
    pub fn msg_hyperlink(&self) -> String {
        self.0.msg("hyperlink", None).unwrap()
    }
    pub fn msg_hyperlink_menu(&self) -> String {
        self.0.msg("hyperlink-menu", None).unwrap()
    }
    pub fn msg_hyperlink_new(&self) -> String {
        self.0.msg("hyperlink-new", None).unwrap()
    }
    pub fn msg_hyperlink_pages(&self) -> String {
        self.0.msg("hyperlink-pages", None).unwrap()
    }
    pub fn msg_hyperlinks(&self) -> String {
        self.0.msg("hyperlinks", None).unwrap()
    }
    pub fn msg_illustrations(&self) -> String {
        self.0.msg("illustrations", None).unwrap()
    }
    pub fn msg_increase_indent(&self) -> String {
        self.0.msg("increase-indent", None).unwrap()
    }
    pub fn msg_increase_indent_planned(&self) -> String {
        self.0.msg("increase-indent-planned", None).unwrap()
    }
    pub fn msg_info(&self) -> String {
        self.0.msg("info", None).unwrap()
    }
    pub fn msg_inherited(&self) -> String {
        self.0.msg("inherited", None).unwrap()
    }
    pub fn msg_inherited_from_the_shape_s_maste(&self) -> String {
        self.0
            .msg("inherited-from-the-shape-s-maste", None)
            .unwrap()
    }
    pub fn msg_insert(&self) -> String {
        self.0.msg("insert", None).unwrap()
    }
    pub fn msg_insert_a_png_or_jpeg_picture_fro(&self) -> String {
        self.0
            .msg("insert-a-png-or-jpeg-picture-fro", None)
            .unwrap()
    }
    pub fn msg_insert_control(&self) -> String {
        self.0.msg("insert-control", None).unwrap()
    }
    pub fn msg_insert_page(&self) -> String {
        self.0.msg("insert-page", None).unwrap()
    }
    pub fn msg_insert_pages(&self) -> String {
        self.0.msg("insert-pages", None).unwrap()
    }
    pub fn msg_inspect_drawing(&self) -> String {
        self.0.msg("inspect-drawing", None).unwrap()
    }
    pub fn msg_inspect_the_selected_shape_s_cel(&self) -> String {
        self.0
            .msg("inspect-the-selected-shape-s-cel", None)
            .unwrap()
    }
    pub fn msg_italic(&self) -> String {
        self.0.msg("italic", None).unwrap()
    }
    pub fn msg_italic_planned(&self) -> String {
        self.0.msg("italic-planned", None).unwrap()
    }
    pub fn msg_jpg(&self) -> String {
        self.0.msg("jpg", None).unwrap()
    }
    pub fn msg_jump_size(&self) -> String {
        self.0.msg("jump-size", None).unwrap()
    }
    pub fn msg_justify(&self) -> String {
        self.0.msg("justify", None).unwrap()
    }
    pub fn msg_justify_planned(&self) -> String {
        self.0.msg("justify-planned", None).unwrap()
    }
    /// $label (String) - argument
    pub fn msg_label_a_choice_cannot_contain<F0: AsRef<str>>(&self, label: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("label", label.as_ref());
        self.0
            .msg("label-a-choice-cannot-contain", Some(_args))
            .unwrap()
    }
    /// $label (String) - argument
    pub fn msg_label_dialog_planned_for_a_later<F0: AsRef<str>>(&self, label: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("label", label.as_ref());
        self.0
            .msg("label-dialog-planned-for-a-later", Some(_args))
            .unwrap()
    }
    /// $label (String) - argument
    pub fn msg_label_not_a_number<F0: AsRef<str>>(&self, label: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("label", label.as_ref());
        self.0.msg("label-not-a-number", Some(_args)).unwrap()
    }
    pub fn msg_landscape(&self) -> String {
        self.0.msg("landscape", None).unwrap()
    }
    pub fn msg_language(&self) -> String {
        self.0.msg("language", None).unwrap()
    }
    pub fn msg_language_menu(&self) -> String {
        self.0.msg("language-menu", None).unwrap()
    }
    #[allow(unused)]
    pub fn msg_language_name(&self) -> String {
        self.0.msg("language-name", None).unwrap()
    }
    pub fn msg_language_settings_planned(&self) -> String {
        self.0.msg("language-settings-planned", None).unwrap()
    }
    pub fn msg_lasso_select(&self) -> String {
        self.0.msg("lasso-select", None).unwrap()
    }
    pub fn msg_lasso_select_drag_a_freehand_tra(&self) -> String {
        self.0
            .msg("lasso-select-drag-a-freehand-tra", None)
            .unwrap()
    }
    pub fn msg_lasso_select_off(&self) -> String {
        self.0.msg("lasso-select-off", None).unwrap()
    }
    /// $n (Number) - argument
    pub fn msg_lasso_selected_n_shape_s<F0: Into<FluentNumber>>(&self, n: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("n", n.into());
        self.0.msg("lasso-selected-n-shape-s", Some(_args)).unwrap()
    }
    pub fn msg_last_page(&self) -> String {
        self.0.msg("last-page", None).unwrap()
    }
    pub fn msg_layer(&self) -> String {
        self.0.msg("layer", None).unwrap()
    }
    pub fn msg_layout(&self) -> String {
        self.0.msg("layout", None).unwrap()
    }
    pub fn msg_layout_options(&self) -> String {
        self.0.msg("layout-options", None).unwrap()
    }
    pub fn msg_layout_options_planned(&self) -> String {
        self.0.msg("layout-options-planned", None).unwrap()
    }
    pub fn msg_layout_size(&self) -> String {
        self.0.msg("layout-size", None).unwrap()
    }
    pub fn msg_layout_size_planned(&self) -> String {
        self.0.msg("layout-size-planned", None).unwrap()
    }
    pub fn msg_layout_spacing_planned(&self) -> String {
        self.0.msg("layout-spacing-planned", None).unwrap()
    }
    pub fn msg_legend(&self) -> String {
        self.0.msg("legend", None).unwrap()
    }
    pub fn msg_legends_m6_visio_paints_legend_o(&self) -> String {
        self.0
            .msg("legends-m6-visio-paints-legend-o", None)
            .unwrap()
    }
    pub fn msg_line(&self) -> String {
        self.0.msg("line", None).unwrap()
    }
    pub fn msg_line_color_label(&self) -> String {
        self.0.msg("line-color-label", None).unwrap()
    }
    pub fn msg_line_jumps(&self) -> String {
        self.0.msg("line-jumps", None).unwrap()
    }
    pub fn msg_line_jumps_hidden(&self) -> String {
        self.0.msg("line-jumps-hidden", None).unwrap()
    }
    pub fn msg_line_jumps_shown(&self) -> String {
        self.0.msg("line-jumps-shown", None).unwrap()
    }
    /// $reason (String) - argument
    pub fn msg_line_move_refused_reason<F0: AsRef<str>>(&self, reason: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("reason", reason.as_ref());
        self.0.msg("line-move-refused-reason", Some(_args)).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_line_moved_but_name_is_formula_d<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0
            .msg("line-moved-but-name-is-formula-d", Some(_args))
            .unwrap()
    }
    pub fn msg_line_tool(&self) -> String {
        self.0.msg("line-tool", None).unwrap()
    }
    pub fn msg_line_tool_planned(&self) -> String {
        self.0.msg("line-tool-planned", None).unwrap()
    }
    pub fn msg_line_weight(&self) -> String {
        self.0.msg("line-weight", None).unwrap()
    }
    pub fn msg_link_data_to_shapes(&self) -> String {
        self.0.msg("link-data-to-shapes", None).unwrap()
    }
    pub fn msg_links(&self) -> String {
        self.0.msg("links", None).unwrap()
    }
    pub fn msg_location(&self) -> String {
        self.0.msg("location", None).unwrap()
    }
    pub fn msg_lock_container(&self) -> String {
        self.0.msg("lock-container", None).unwrap()
    }
    /// $lock (String) - argument
    pub fn msg_lock_is_set_protection_section<F0: AsRef<str>>(&self, lock: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("lock", lock.as_ref());
        self.0
            .msg("lock-is-set-protection-section", Some(_args))
            .unwrap()
    }
    pub fn msg_lock_the_container_against_membe(&self) -> String {
        self.0
            .msg("lock-the-container-against-membe", None)
            .unwrap()
    }
    pub fn msg_long_arrow(&self) -> String {
        self.0.msg("long-arrow", None).unwrap()
    }
    pub fn msg_long_dash(&self) -> String {
        self.0.msg("long-dash", None).unwrap()
    }
    pub fn msg_macro_security(&self) -> String {
        self.0.msg("macro-security", None).unwrap()
    }
    pub fn msg_macro_security_m7(&self) -> String {
        self.0.msg("macro-security-m7", None).unwrap()
    }
    pub fn msg_macros(&self) -> String {
        self.0.msg("macros", None).unwrap()
    }
    pub fn msg_macros_arrive_with_m7_vba_vsdm_m(&self) -> String {
        self.0
            .msg("macros-arrive-with-m7-vba-vsdm-m", None)
            .unwrap()
    }
    pub fn msg_manage_reports(&self) -> String {
        self.0.msg("manage-reports", None).unwrap()
    }
    pub fn msg_margins(&self) -> String {
        self.0.msg("margins", None).unwrap()
    }
    pub fn msg_markup(&self) -> String {
        self.0.msg("markup", None).unwrap()
    }
    pub fn msg_markup_tracking_planned_no_marku(&self) -> String {
        self.0
            .msg("markup-tracking-planned-no-marku", None)
            .unwrap()
    }
    pub fn msg_master(&self) -> String {
        self.0.msg("master", None).unwrap()
    }
    /// $mid (Number) - argument
    pub fn msg_master_mid<F0: Into<FluentNumber>>(&self, mid: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("mid", mid.into());
        self.0.msg("master-mid", Some(_args)).unwrap()
    }
    pub fn msg_member_highlight_off(&self) -> String {
        self.0.msg("member-highlight-off", None).unwrap()
    }
    pub fn msg_member_highlight_on(&self) -> String {
        self.0.msg("member-highlight-on", None).unwrap()
    }
    pub fn msg_membership(&self) -> String {
        self.0.msg("membership", None).unwrap()
    }
    pub fn msg_metric_stencils(&self) -> String {
        self.0.msg("metric-stencils", None).unwrap()
    }
    pub fn msg_middle(&self) -> String {
        self.0.msg("middle", None).unwrap()
    }
    pub fn msg_mm_in(&self) -> String {
        self.0.msg("mm-in", None).unwrap()
    }
    pub fn msg_more_shapes(&self) -> String {
        self.0.msg("more-shapes", None).unwrap()
    }
    pub fn msg_move_container_and_members(&self) -> String {
        self.0.msg("move-container-and-members", None).unwrap()
    }
    pub fn msg_move_down(&self) -> String {
        self.0.msg("move-down", None).unwrap()
    }
    /// $shape_id (Number) - argument
    pub fn msg_move_line_endpoint_shape_shape_i<F0: Into<FluentNumber>>(
        &self,
        shape_id: F0,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("shape_id", shape_id.into());
        self.0
            .msg("move-line-endpoint-shape-shape-i", Some(_args))
            .unwrap()
    }
    pub fn msg_move_shapes(&self) -> String {
        self.0.msg("move-shapes", None).unwrap()
    }
    pub fn msg_move_up(&self) -> String {
        self.0.msg("move-up", None).unwrap()
    }
    pub fn msg_name(&self) -> String {
        self.0.msg("name", None).unwrap()
    }
    pub fn msg_name_page_id<'a, F0: Into<FluentValue<'a>>, F1: Into<FluentValue<'a>>>(
        &self,
        name: F0,
        id: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name);
        _args.set("id", id);
        self.0.msg("name-page-id", Some(_args)).unwrap()
    }
    pub fn msg_name_u_master_id<'a, F0: Into<FluentValue<'a>>, F1: Into<FluentValue<'a>>>(
        &self,
        name_u: F0,
        id: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name_u", name_u);
        _args.set("id", id);
        self.0.msg("name-u-master-id", Some(_args)).unwrap()
    }
    pub fn msg_network_diagram(&self) -> String {
        self.0.msg("network-diagram", None).unwrap()
    }
    pub fn msg_new_background_page(&self) -> String {
        self.0.msg("new-background-page", None).unwrap()
    }
    pub fn msg_new_blank_drawing(&self) -> String {
        self.0.msg("new-blank-drawing", None).unwrap()
    }
    pub fn msg_new_blank_drawing_unsaved(&self) -> String {
        self.0.msg("new-blank-drawing-unsaved", None).unwrap()
    }
    pub fn msg_new_choice(&self) -> String {
        self.0.msg("new-choice", None).unwrap()
    }
    pub fn msg_new_page(&self) -> String {
        self.0.msg("new-page", None).unwrap()
    }
    pub fn msg_new_stencil(&self) -> String {
        self.0.msg("new-stencil", None).unwrap()
    }
    pub fn msg_new_window(&self) -> String {
        self.0.msg("new-window", None).unwrap()
    }
    pub fn msg_new_window_planned_single_window(&self) -> String {
        self.0
            .msg("new-window-planned-single-window", None)
            .unwrap()
    }
    pub fn msg_next_comment(&self) -> String {
        self.0.msg("next-comment", None).unwrap()
    }
    pub fn msg_next_comment_planned(&self) -> String {
        self.0.msg("next-comment-planned", None).unwrap()
    }
    pub fn msg_next_page(&self) -> String {
        self.0.msg("next-page", None).unwrap()
    }
    pub fn msg_no_background(&self) -> String {
        self.0.msg("no-background", None).unwrap()
    }
    pub fn msg_no_background_pages_right_click(&self) -> String {
        self.0.msg("no-background-pages-right-click", None).unwrap()
    }
    pub fn msg_no_comments_on_this_shape_yet(&self) -> String {
        self.0.msg("no-comments-on-this-shape-yet", None).unwrap()
    }
    pub fn msg_no_document_open(&self) -> String {
        self.0.msg("no-document-open", None).unwrap()
    }
    pub fn msg_no_hyperlinks_on_this_shape(&self) -> String {
        self.0.msg("no-hyperlinks-on-this-shape", None).unwrap()
    }
    /// $key (String) - argument
    pub fn msg_no_keytip_key<F0: AsRef<str>>(&self, key: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("key", key.as_ref());
        self.0.msg("no-keytip-key", Some(_args)).unwrap()
    }
    pub fn msg_no_matching_shapes(&self) -> String {
        self.0.msg("no-matching-shapes", None).unwrap()
    }
    pub fn msg_no_picture_on_the_clipboard(&self) -> String {
        self.0.msg("no-picture-on-the-clipboard", None).unwrap()
    }
    pub fn msg_no_shapes(&self) -> String {
        self.0.msg("no-shapes", None).unwrap()
    }
    pub fn msg_none(&self) -> String {
        self.0.msg("none", None).unwrap()
    }
    pub fn msg_nothing_on_this_page_matches_the(&self) -> String {
        self.0
            .msg("nothing-on-this-page-matches-the", None)
            .unwrap()
    }
    pub fn msg_nothing_selected_to_copy(&self) -> String {
        self.0.msg("nothing-selected-to-copy", None).unwrap()
    }
    pub fn msg_nothing_to_redo(&self) -> String {
        self.0.msg("nothing-to-redo", None).unwrap()
    }
    pub fn msg_nothing_to_undo(&self) -> String {
        self.0.msg("nothing-to-undo", None).unwrap()
    }
    pub fn msg_nudge_selection(&self) -> String {
        self.0.msg("nudge-selection", None).unwrap()
    }
    pub fn msg_numbering(&self) -> String {
        self.0.msg("numbering", None).unwrap()
    }
    pub fn msg_numbering_planned(&self) -> String {
        self.0.msg("numbering-planned", None).unwrap()
    }
    pub fn msg_nurbs_control_customized_the_e_f(&self) -> String {
        self.0
            .msg("nurbs-control-customized-the-e-f", None)
            .unwrap()
    }
    pub fn msg_ok(&self) -> String {
        self.0.msg("ok", None).unwrap()
    }
    pub fn msg_online_pictures(&self) -> String {
        self.0.msg("online-pictures", None).unwrap()
    }
    pub fn msg_online_pictures_planned(&self) -> String {
        self.0.msg("online-pictures-planned", None).unwrap()
    }
    pub fn msg_only_png_and_jpeg_pictures_are_s(&self) -> String {
        self.0
            .msg("only-png-and-jpeg-pictures-are-s", None)
            .unwrap()
    }
    pub fn msg_open_2_prong(&self) -> String {
        self.0.msg("open-2-prong", None).unwrap()
    }
    pub fn msg_open_a_visio_file(&self) -> String {
        self.0.msg("open-a-visio-file", None).unwrap()
    }
    pub fn msg_open_barbed(&self) -> String {
        self.0.msg("open-barbed", None).unwrap()
    }
    pub fn msg_open_circle(&self) -> String {
        self.0.msg("open-circle", None).unwrap()
    }
    pub fn msg_open_diamond(&self) -> String {
        self.0.msg("open-diamond", None).unwrap()
    }
    pub fn msg_open_double_chevron_large(&self) -> String {
        self.0.msg("open-double-chevron-large", None).unwrap()
    }
    pub fn msg_open_existing_stencil(&self) -> String {
        self.0.msg("open-existing-stencil", None).unwrap()
    }
    /// $e (String) - argument
    pub fn msg_open_failed_e<F0: AsRef<str>>(&self, e: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("e", e.as_ref());
        self.0.msg("open-failed-e", Some(_args)).unwrap()
    }
    pub fn msg_open_failed_the_load_worker_died(&self) -> String {
        self.0
            .msg("open-failed-the-load-worker-died", None)
            .unwrap()
    }
    pub fn msg_open_file_cta(&self) -> String {
        self.0.msg("open-file-cta", None).unwrap()
    }
    pub fn msg_open_file_title(&self) -> String {
        self.0.msg("open-file-title", None).unwrap()
    }
    pub fn msg_open_fork(&self) -> String {
        self.0.msg("open-fork", None).unwrap()
    }
    /// $group_label (String) - argument
    pub fn msg_open_group_label<F0: AsRef<str>>(&self, group_label: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("group_label", group_label.as_ref());
        self.0.msg("open-group-label", Some(_args)).unwrap()
    }
    pub fn msg_open_square(&self) -> String {
        self.0.msg("open-square", None).unwrap()
    }
    pub fn msg_open_triangle_convex_base(&self) -> String {
        self.0.msg("open-triangle-convex-base", None).unwrap()
    }
    pub fn msg_open_triangle_large(&self) -> String {
        self.0.msg("open-triangle-large", None).unwrap()
    }
    pub fn msg_open_triangle_medium(&self) -> String {
        self.0.msg("open-triangle-medium", None).unwrap()
    }
    pub fn msg_open_triangle_medium_large(&self) -> String {
        self.0.msg("open-triangle-medium-large", None).unwrap()
    }
    pub fn msg_open_triangle_small(&self) -> String {
        self.0.msg("open-triangle-small", None).unwrap()
    }
    pub fn msg_open_v_tall(&self) -> String {
        self.0.msg("open-v-tall", None).unwrap()
    }
    pub fn msg_open_vsdx_vstx_vssx_ctrl_o(&self) -> String {
        self.0.msg("open-vsdx-vstx-vssx-ctrl-o", None).unwrap()
    }
    pub fn msg_open_wide(&self) -> String {
        self.0.msg("open-wide", None).unwrap()
    }
    /// $stem (String) - argument
    pub fn msg_opened_stencil_stem<F0: AsRef<str>>(&self, stem: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("stem", stem.as_ref());
        self.0.msg("opened-stencil-stem", Some(_args)).unwrap()
    }
    pub fn msg_operations(&self) -> String {
        self.0.msg("operations", None).unwrap()
    }
    pub fn msg_options(&self) -> String {
        self.0.msg("options", None).unwrap()
    }
    pub fn msg_options_arrive_with_a_later_mile(&self) -> String {
        self.0
            .msg("options-arrive-with-a-later-mile", None)
            .unwrap()
    }
    pub fn msg_org_chart(&self) -> String {
        self.0.msg("org-chart", None).unwrap()
    }
    pub fn msg_orientation(&self) -> String {
        self.0.msg("orientation", None).unwrap()
    }
    pub fn msg_output_file(&self) -> String {
        self.0.msg("output-file", None).unwrap()
    }
    pub fn msg_page_breaks(&self) -> String {
        self.0.msg("page-breaks", None).unwrap()
    }
    pub fn msg_page_breaks_planned(&self) -> String {
        self.0.msg("page-breaks-planned", None).unwrap()
    }
    pub fn msg_page_name(&self) -> String {
        self.0.msg("page-name", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_page_name_is_a_foreground_page_a<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0
            .msg("page-name-is-a-foreground-page-a", Some(_args))
            .unwrap()
    }
    /// $name (String) - argument
    pub fn msg_page_name_marked_as_a_background<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0
            .msg("page-name-marked-as-a-background", Some(_args))
            .unwrap()
    }
    pub fn msg_page_number(&self) -> String {
        self.0.msg("page-number", None).unwrap()
    }
    pub fn msg_page_number_field_planned(&self) -> String {
        self.0.msg("page-number-field-planned", None).unwrap()
    }
    pub fn msg_page_setup(&self) -> String {
        self.0.msg("page-setup", None).unwrap()
    }
    pub fn msg_page_setup_menu(&self) -> String {
        self.0.msg("page-setup-menu", None).unwrap()
    }
    pub fn msg_page_size(&self) -> String {
        self.0.msg("page-size", None).unwrap()
    }
    pub fn msg_page_size_iso_ansi_presets_or_cu(&self) -> String {
        self.0
            .msg("page-size-iso-ansi-presets-or-cu", None)
            .unwrap()
    }
    pub fn msg_page_width(&self) -> String {
        self.0.msg("page-width", None).unwrap()
    }
    /// $to (Number) - argument
    pub fn msg_paint_formatting_onto_shape_to<F0: Into<FluentNumber>>(&self, to: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("to", to.into());
        self.0
            .msg("paint-formatting-onto-shape-to", Some(_args))
            .unwrap()
    }
    /// $n (Number) - argument
    /// $to (Number) - argument
    /// $n (Number) - argument
    /// $to (Number) - argument
    pub fn msg_painter_cells<F0: Into<FluentNumber>, F1: Into<FluentNumber>>(
        &self,
        n: F0,
        to: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("n", n.into());
        _args.set("to", to.into());
        self.0.msg("painter-cells", Some(_args)).unwrap()
    }
    /// $n (Number) - argument
    /// $to (Number) - argument
    pub fn msg_painter_cells_sticky<F0: Into<FluentNumber>, F1: Into<FluentNumber>>(
        &self,
        n: F0,
        to: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("n", n.into());
        _args.set("to", to.into());
        self.0.msg("painter-cells-sticky", Some(_args)).unwrap()
    }
    pub fn msg_paragraph(&self) -> String {
        self.0.msg("paragraph", None).unwrap()
    }
    pub fn msg_paste(&self) -> String {
        self.0.msg("paste", None).unwrap()
    }
    pub fn msg_pick_an_output_file_first(&self) -> String {
        self.0.msg("pick-an-output-file-first", None).unwrap()
    }
    pub fn msg_pictures(&self) -> String {
        self.0.msg("pictures", None).unwrap()
    }
    pub fn msg_pointer_tool(&self) -> String {
        self.0.msg("pointer-tool", None).unwrap()
    }
    pub fn msg_pointer_tool_select_and_drag_esc(&self) -> String {
        self.0
            .msg("pointer-tool-select-and-drag-esc", None)
            .unwrap()
    }
    pub fn msg_portrait(&self) -> String {
        self.0.msg("portrait", None).unwrap()
    }
    pub fn msg_presentation_mode(&self) -> String {
        self.0.msg("presentation-mode", None).unwrap()
    }
    pub fn msg_presentation_mode_planned(&self) -> String {
        self.0.msg("presentation-mode-planned", None).unwrap()
    }
    pub fn msg_preset_sizes(&self) -> String {
        self.0.msg("preset-sizes", None).unwrap()
    }
    pub fn msg_previous_comment(&self) -> String {
        self.0.msg("previous-comment", None).unwrap()
    }
    pub fn msg_previous_comment_planned(&self) -> String {
        self.0.msg("previous-comment-planned", None).unwrap()
    }
    pub fn msg_previous_page(&self) -> String {
        self.0.msg("previous-page", None).unwrap()
    }
    pub fn msg_print_384_dpi(&self) -> String {
        self.0.msg("print-384-dpi", None).unwrap()
    }
    pub fn msg_print_stub(&self) -> String {
        self.0.msg("print-stub", None).unwrap()
    }
    pub fn msg_print_the_current_drawing(&self) -> String {
        self.0.msg("print-the-current-drawing", None).unwrap()
    }
    pub fn msg_printing_arrives_with_m6(&self) -> String {
        self.0.msg("printing-arrives-with-m6", None).unwrap()
    }
    pub fn msg_proofing(&self) -> String {
        self.0.msg("proofing", None).unwrap()
    }
    pub fn msg_protect_drawing(&self) -> String {
        self.0.msg("protect-drawing", None).unwrap()
    }
    pub fn msg_protection(&self) -> String {
        self.0.msg("protection", None).unwrap()
    }
    pub fn msg_quick_import(&self) -> String {
        self.0.msg("quick-import", None).unwrap()
    }
    pub fn msg_quick_shapes(&self) -> String {
        self.0.msg("quick-shapes", None).unwrap()
    }
    pub fn msg_quick_styles(&self) -> String {
        self.0.msg("quick-styles", None).unwrap()
    }
    pub fn msg_rail_export(&self) -> String {
        self.0.msg("rail-export", None).unwrap()
    }
    pub fn msg_rail_new(&self) -> String {
        self.0.msg("rail-new", None).unwrap()
    }
    pub fn msg_rail_print(&self) -> String {
        self.0.msg("rail-print", None).unwrap()
    }
    pub fn msg_re_layout_page(&self) -> String {
        self.0.msg("re-layout-page", None).unwrap()
    }
    pub fn msg_recent(&self) -> String {
        self.0.msg("recent", None).unwrap()
    }
    pub fn msg_rectangle_tool(&self) -> String {
        self.0.msg("rectangle-tool", None).unwrap()
    }
    pub fn msg_rectangle_tool_planned(&self) -> String {
        self.0.msg("rectangle-tool-planned", None).unwrap()
    }
    pub fn msg_redo_ctrl_y(&self) -> String {
        self.0.msg("redo-ctrl-y", None).unwrap()
    }
    /// $l (String) - argument
    pub fn msg_redo_l<F0: AsRef<str>>(&self, l: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("l", l.as_ref());
        self.0.msg("redo-l", Some(_args)).unwrap()
    }
    pub fn msg_refresh_all(&self) -> String {
        self.0.msg("refresh-all", None).unwrap()
    }
    pub fn msg_release_the_members_and_delete_t(&self) -> String {
        self.0
            .msg("release-the-members-and-delete-t", None)
            .unwrap()
    }
    pub fn msg_remove(&self) -> String {
        self.0.msg("remove", None).unwrap()
    }
    pub fn msg_remove_background_mark(&self) -> String {
        self.0.msg("remove-background-mark", None).unwrap()
    }
    /// $ix (Number) - argument
    pub fn msg_remove_connection_point_ix<F0: Into<FluentNumber>>(&self, ix: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("ix", ix.into());
        self.0
            .msg("remove-connection-point-ix", Some(_args))
            .unwrap()
    }
    pub fn msg_remove_from_quick_shapes(&self) -> String {
        self.0.msg("remove-from-quick-shapes", None).unwrap()
    }
    pub fn msg_removed_from_quick_shapes(&self) -> String {
        self.0.msg("removed-from-quick-shapes", None).unwrap()
    }
    pub fn msg_rename_page(&self) -> String {
        self.0.msg("rename-page", None).unwrap()
    }
    pub fn msg_reorder_pages(&self) -> String {
        self.0.msg("reorder-pages", None).unwrap()
    }
    pub fn msg_reorder_pages_menu(&self) -> String {
        self.0.msg("reorder-pages-menu", None).unwrap()
    }
    pub fn msg_replace(&self) -> String {
        self.0.msg("replace", None).unwrap()
    }
    pub fn msg_replace_planned_later_milestone(&self) -> String {
        self.0.msg("replace-planned-later-milestone", None).unwrap()
    }
    pub fn msg_reply(&self) -> String {
        self.0.msg("reply", None).unwrap()
    }
    pub fn msg_report_definitions_planned(&self) -> String {
        self.0.msg("report-definitions-planned", None).unwrap()
    }
    pub fn msg_reports(&self) -> String {
        self.0.msg("reports", None).unwrap()
    }
    pub fn msg_resize_refused_shape_is_protecte(&self) -> String {
        self.0
            .msg("resize-refused-shape-is-protecte", None)
            .unwrap()
    }
    pub fn msg_resolution(&self) -> String {
        self.0.msg("resolution", None).unwrap()
    }
    pub fn msg_resolution_dpi(&self) -> String {
        self.0.msg("resolution-dpi", None).unwrap()
    }
    pub fn msg_review(&self) -> String {
        self.0.msg("review", None).unwrap()
    }
    pub fn msg_review_new_comment(&self) -> String {
        self.0.msg("review-new-comment", None).unwrap()
    }
    pub fn msg_reviewing_pane(&self) -> String {
        self.0.msg("reviewing-pane", None).unwrap()
    }
    pub fn msg_reviewing_pane_planned(&self) -> String {
        self.0.msg("reviewing-pane-planned", None).unwrap()
    }
    pub fn msg_right_angle_connector(&self) -> String {
        self.0.msg("right-angle-connector", None).unwrap()
    }
    pub fn msg_ring(&self) -> String {
        self.0.msg("ring", None).unwrap()
    }
    pub fn msg_ring_tick(&self) -> String {
        self.0.msg("ring-tick", None).unwrap()
    }
    pub fn msg_rotate_left(&self) -> String {
        self.0.msg("rotate-left", None).unwrap()
    }
    pub fn msg_rotate_left_90(&self) -> String {
        self.0.msg("rotate-left-90", None).unwrap()
    }
    pub fn msg_rotate_right(&self) -> String {
        self.0.msg("rotate-right", None).unwrap()
    }
    pub fn msg_rotate_right_90(&self) -> String {
        self.0.msg("rotate-right-90", None).unwrap()
    }
    pub fn msg_rotate_shapes(&self) -> String {
        self.0.msg("rotate-shapes", None).unwrap()
    }
    pub fn msg_rotate_the_selection_90_clockwis(&self) -> String {
        self.0
            .msg("rotate-the-selection-90-clockwis", None)
            .unwrap()
    }
    pub fn msg_rotate_the_selection_90_counter(&self) -> String {
        self.0.msg("rotate-the-selection-90-counter", None).unwrap()
    }
    pub fn msg_rounding(&self) -> String {
        self.0.msg("rounding", None).unwrap()
    }
    pub fn msg_ruler_subdivisions(&self) -> String {
        self.0.msg("ruler-subdivisions", None).unwrap()
    }
    pub fn msg_rulers(&self) -> String {
        self.0.msg("rulers", None).unwrap()
    }
    pub fn msg_save(&self) -> String {
        self.0.msg("save", None).unwrap()
    }
    pub fn msg_save_as(&self) -> String {
        self.0.msg("save-as", None).unwrap()
    }
    pub fn msg_save_changes(&self) -> String {
        self.0.msg("save-changes", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_save_changes_to_name<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0.msg("save-changes-to-name", Some(_args)).unwrap()
    }
    pub fn msg_save_ctrl_s(&self) -> String {
        self.0.msg("save-ctrl-s", None).unwrap()
    }
    pub fn msg_save_new(&self) -> String {
        self.0.msg("save-new", None).unwrap()
    }
    pub fn msg_save_stencil(&self) -> String {
        self.0.msg("save-stencil", None).unwrap()
    }
    pub fn msg_save_the_stencil_planned(&self) -> String {
        self.0.msg("save-the-stencil-planned", None).unwrap()
    }
    pub fn msg_scale(&self) -> String {
        self.0.msg("scale", None).unwrap()
    }
    pub fn msg_screen_96_dpi(&self) -> String {
        self.0.msg("screen-96-dpi", None).unwrap()
    }
    pub fn msg_search_shapes(&self) -> String {
        self.0.msg("search-shapes", None).unwrap()
    }
    pub fn msg_search_templates(&self) -> String {
        self.0.msg("search-templates", None).unwrap()
    }
    pub fn msg_select(&self) -> String {
        self.0.msg("select", None).unwrap()
    }
    pub fn msg_select_a_container_to_disband(&self) -> String {
        self.0.msg("select-a-container-to-disband", None).unwrap()
    }
    pub fn msg_select_a_format(&self) -> String {
        self.0.msg("select-a-format", None).unwrap()
    }
    pub fn msg_select_a_group_to_ungroup(&self) -> String {
        self.0.msg("select-a-group-to-ungroup", None).unwrap()
    }
    pub fn msg_select_a_shape_to_copy_formattin(&self) -> String {
        self.0
            .msg("select-a-shape-to-copy-formattin", None)
            .unwrap()
    }
    pub fn msg_select_a_shape_to_edit_its_hyper(&self) -> String {
        self.0
            .msg("select-a-shape-to-edit-its-hyper", None)
            .unwrap()
    }
    pub fn msg_select_all(&self) -> String {
        self.0.msg("select-all", None).unwrap()
    }
    pub fn msg_select_by(&self) -> String {
        self.0.msg("select-by", None).unwrap()
    }
    pub fn msg_select_by_type(&self) -> String {
        self.0.msg("select-by-type", None).unwrap()
    }
    pub fn msg_select_by_type_menu(&self) -> String {
        self.0.msg("select-by-type-menu", None).unwrap()
    }
    pub fn msg_select_by_type_pick_a_criterion(&self) -> String {
        self.0.msg("select-by-type-pick-a-criterion", None).unwrap()
    }
    pub fn msg_select_contents(&self) -> String {
        self.0.msg("select-contents", None).unwrap()
    }
    pub fn msg_select_every_shape_on_the_page_c(&self) -> String {
        self.0
            .msg("select-every-shape-on-the-page-c", None)
            .unwrap()
    }
    pub fn msg_select_shapes_by_type_master_or(&self) -> String {
        self.0.msg("select-shapes-by-type-master-or", None).unwrap()
    }
    pub fn msg_select_the_container_s_members_p(&self) -> String {
        self.0
            .msg("select-the-container-s-members-p", None)
            .unwrap()
    }
    pub fn msg_select_the_shapes_to_contain_fir(&self) -> String {
        self.0
            .msg("select-the-shapes-to-contain-fir", None)
            .unwrap()
    }
    pub fn msg_select_two_or_more_shapes_to_ali(&self) -> String {
        self.0
            .msg("select-two-or-more-shapes-to-ali", None)
            .unwrap()
    }
    pub fn msg_selected_all_shapes(&self) -> String {
        self.0.msg("selected-all-shapes", None).unwrap()
    }
    /// $shape_id (Number) - argument
    pub fn msg_selected_shape_shape_id<F0: Into<FluentNumber>>(&self, shape_id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("shape_id", shape_id.into());
        self.0.msg("selected-shape-shape-id", Some(_args)).unwrap()
    }
    pub fn msg_selection(&self) -> String {
        self.0.msg("selection", None).unwrap()
    }
    pub fn msg_selection_pane(&self) -> String {
        self.0.msg("selection-pane", None).unwrap()
    }
    pub fn msg_selection_pane_planned_112_shell(&self) -> String {
        self.0
            .msg("selection-pane-planned-112-shell", None)
            .unwrap()
    }
    pub fn msg_send_backward(&self) -> String {
        self.0.msg("send-backward", None).unwrap()
    }
    pub fn msg_send_backward_planned(&self) -> String {
        self.0.msg("send-backward-planned", None).unwrap()
    }
    pub fn msg_send_the_drawing_to_a_printer_or(&self) -> String {
        self.0
            .msg("send-the-drawing-to-a-printer-or", None)
            .unwrap()
    }
    pub fn msg_send_to_back(&self) -> String {
        self.0.msg("send-to-back", None).unwrap()
    }
    pub fn msg_set_as_background(&self) -> String {
        self.0.msg("set-as-background", None).unwrap()
    }
    pub fn msg_set_connector_route_style(&self) -> String {
        self.0.msg("set-connector-route-style", None).unwrap()
    }
    pub fn msg_set_page_size(&self) -> String {
        self.0.msg("set-page-size", None).unwrap()
    }
    pub fn msg_set_proofing_language(&self) -> String {
        self.0.msg("set-proofing-language", None).unwrap()
    }
    pub fn msg_set_proofing_language_planned(&self) -> String {
        self.0.msg("set-proofing-language-planned", None).unwrap()
    }
    pub fn msg_shape_behavior_planned(&self) -> String {
        self.0.msg("shape-behavior-planned", None).unwrap()
    }
    pub fn msg_shape_data(&self) -> String {
        self.0.msg("shape-data", None).unwrap()
    }
    pub fn msg_shape_data_menu(&self) -> String {
        self.0.msg("shape-data-menu", None).unwrap()
    }
    pub fn msg_shape_data_window(&self) -> String {
        self.0.msg("shape-data-window", None).unwrap()
    }
    pub fn msg_shape_data_window_m6(&self) -> String {
        self.0.msg("shape-data-window-m6", None).unwrap()
    }
    pub fn msg_shape_design(&self) -> String {
        self.0.msg("shape-design", None).unwrap()
    }
    /// $dup_id (Number) - argument
    /// $sid (Number) - argument
    pub fn msg_shape_dup_id_connected_to_sid_au<F0: Into<FluentNumber>, F1: Into<FluentNumber>>(
        &self,
        dup_id: F0,
        sid: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("dup_id", dup_id.into());
        _args.set("sid", sid.into());
        self.0
            .msg("shape-dup-id-connected-to-sid-au", Some(_args))
            .unwrap()
    }
    pub fn msg_shape_effects_planned(&self) -> String {
        self.0.msg("shape-effects-planned", None).unwrap()
    }
    pub fn msg_shape_extensions(&self) -> String {
        self.0.msg("shape-extensions", None).unwrap()
    }
    pub fn msg_shape_fill_color(&self) -> String {
        self.0.msg("shape-fill-color", None).unwrap()
    }
    pub fn msg_shape_handles(&self) -> String {
        self.0.msg("shape-handles", None).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_shape_id_s_alignment_box<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("shape-id-s-alignment-box", Some(_args)).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_shape_id_s_connection_point<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0
            .msg("shape-id-s-connection-point", Some(_args))
            .unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_shape_id_s_extension<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("shape-id-s-extension", Some(_args)).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_shape_id_s_handle<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("shape-id-s-handle", Some(_args)).unwrap()
    }
    /// $id (Number) - argument
    pub fn msg_shape_id_s_vertex<F0: Into<FluentNumber>>(&self, id: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("id", id.into());
        self.0.msg("shape-id-s-vertex", Some(_args)).unwrap()
    }
    pub fn msg_shape_intersections(&self) -> String {
        self.0.msg("shape-intersections", None).unwrap()
    }
    pub fn msg_shape_protection_planned(&self) -> String {
        self.0.msg("shape-protection-planned", None).unwrap()
    }
    pub fn msg_shape_report(&self) -> String {
        self.0.msg("shape-report", None).unwrap()
    }
    pub fn msg_shape_reports_planned(&self) -> String {
        self.0.msg("shape-reports-planned", None).unwrap()
    }
    /// $sid (Number) - argument
    pub fn msg_shape_sid<F0: Into<FluentNumber>>(&self, sid: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("sid", sid.into());
        self.0.msg("shape-sid", Some(_args)).unwrap()
    }
    pub fn msg_shape_styles(&self) -> String {
        self.0.msg("shape-styles", None).unwrap()
    }
    pub fn msg_shape_text(&self) -> String {
        self.0.msg("shape-text", None).unwrap()
    }
    pub fn msg_shape_type(&self) -> String {
        self.0.msg("shape-type", None).unwrap()
    }
    pub fn msg_shape_vertices(&self) -> String {
        self.0.msg("shape-vertices", None).unwrap()
    }
    pub fn msg_shapes(&self) -> String {
        self.0.msg("shapes", None).unwrap()
    }
    pub fn msg_shapesheet(&self) -> String {
        self.0.msg("shapesheet", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_shapesheet_name<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0.msg("shapesheet-name", Some(_args)).unwrap()
    }
    pub fn msg_share(&self) -> String {
        self.0.msg("share", None).unwrap()
    }
    pub fn msg_share_a_copy(&self) -> String {
        self.0.msg("share-a-copy", None).unwrap()
    }
    pub fn msg_share_a_copy_of_the_drawing(&self) -> String {
        self.0.msg("share-a-copy-of-the-drawing", None).unwrap()
    }
    pub fn msg_share_a_copy_of_the_drawing_with(&self) -> String {
        self.0
            .msg("share-a-copy-of-the-drawing-with", None)
            .unwrap()
    }
    pub fn msg_sharing_arrives_with_m6(&self) -> String {
        self.0.msg("sharing-arrives-with-m6", None).unwrap()
    }
    pub fn msg_short_dash(&self) -> String {
        self.0.msg("short-dash", None).unwrap()
    }
    pub fn msg_show(&self) -> String {
        self.0.msg("show", None).unwrap()
    }
    pub fn msg_show_connection_points_planned_m(&self) -> String {
        self.0
            .msg("show-connection-points-planned-m", None)
            .unwrap()
    }
    pub fn msg_show_document_stencil(&self) -> String {
        self.0.msg("show-document-stencil", None).unwrap()
    }
    pub fn msg_show_hide(&self) -> String {
        self.0.msg("show-hide", None).unwrap()
    }
    pub fn msg_show_hide_comments(&self) -> String {
        self.0.msg("show-hide-comments", None).unwrap()
    }
    pub fn msg_show_hide_comments_planned_view(&self) -> String {
        self.0.msg("show-hide-comments-planned-view", None).unwrap()
    }
    pub fn msg_show_hide_data_graphics_on_the_p(&self) -> String {
        self.0
            .msg("show-hide-data-graphics-on-the-p", None)
            .unwrap()
    }
    pub fn msg_show_line_jumps(&self) -> String {
        self.0.msg("show-line-jumps", None).unwrap()
    }
    pub fn msg_show_markup(&self) -> String {
        self.0.msg("show-markup", None).unwrap()
    }
    pub fn msg_show_markup_planned(&self) -> String {
        self.0.msg("show-markup-planned", None).unwrap()
    }
    pub fn msg_show_shapesheet(&self) -> String {
        self.0.msg("show-shapesheet", None).unwrap()
    }
    pub fn msg_size(&self) -> String {
        self.0.msg("size", None).unwrap()
    }
    pub fn msg_size_cell_is_formula_driven_f<
        'a,
        F0: Into<FluentValue<'a>>,
        F1: Into<FluentValue<'a>>,
    >(
        &self,
        size_cell: F0,
        f: F1,
    ) -> String {
        let mut _args = FluentArgs::new();
        _args.set("size_cell", size_cell);
        _args.set("f", f);
        self.0
            .msg("size-cell-is-formula-driven-f", Some(_args))
            .unwrap()
    }
    pub fn msg_size_pt(&self) -> String {
        self.0.msg("size-pt", None).unwrap()
    }
    pub fn msg_size_the_container_to_its_member(&self) -> String {
        self.0
            .msg("size-the-container-to-its-member", None)
            .unwrap()
    }
    pub fn msg_slanted_tick(&self) -> String {
        self.0.msg("slanted-tick", None).unwrap()
    }
    pub fn msg_small_arrow(&self) -> String {
        self.0.msg("small-arrow", None).unwrap()
    }
    pub fn msg_snap(&self) -> String {
        self.0.msg("snap", None).unwrap()
    }
    pub fn msg_snap_conn_points(&self) -> String {
        self.0.msg("snap-conn-points", None).unwrap()
    }
    pub fn msg_snap_dynamic_grid(&self) -> String {
        self.0.msg("snap-dynamic-grid", None).unwrap()
    }
    pub fn msg_snap_glue(&self) -> String {
        self.0.msg("snap-glue", None).unwrap()
    }
    pub fn msg_snap_to(&self) -> String {
        self.0.msg("snap-to", None).unwrap()
    }
    pub fn msg_solid(&self) -> String {
        self.0.msg("solid", None).unwrap()
    }
    pub fn msg_spacing(&self) -> String {
        self.0.msg("spacing", None).unwrap()
    }
    pub fn msg_spell_check_planned(&self) -> String {
        self.0.msg("spell-check-planned", None).unwrap()
    }
    pub fn msg_spelling(&self) -> String {
        self.0.msg("spelling", None).unwrap()
    }
    pub fn msg_splash_wordmark(&self) -> String {
        self.0.msg("splash-wordmark", None).unwrap()
    }
    pub fn msg_split(&self) -> String {
        self.0.msg("split", None).unwrap()
    }
    pub fn msg_split_view_planned(&self) -> String {
        self.0.msg("split-view-planned", None).unwrap()
    }
    pub fn msg_stamp_tool(&self) -> String {
        self.0.msg("stamp-tool", None).unwrap()
    }
    pub fn msg_stamp_tool_planned(&self) -> String {
        self.0.msg("stamp-tool-planned", None).unwrap()
    }
    pub fn msg_start_an_empty_drawing(&self) -> String {
        self.0.msg("start-an-empty-drawing", None).unwrap()
    }
    pub fn msg_start_an_empty_drawing_on_a_lett(&self) -> String {
        self.0
            .msg("start-an-empty-drawing-on-a-lett", None)
            .unwrap()
    }
    pub fn msg_start_an_empty_us_letter_drawing(&self) -> String {
        self.0
            .msg("start-an-empty-us-letter-drawing", None)
            .unwrap()
    }
    pub fn msg_status(&self) -> String {
        self.0.msg("status", None).unwrap()
    }
    pub fn msg_stealth_dart(&self) -> String {
        self.0.msg("stealth-dart", None).unwrap()
    }
    pub fn msg_stencil(&self) -> String {
        self.0.msg("stencil", None).unwrap()
    }
    pub fn msg_stencil_editing_planned_scope_ca(&self) -> String {
        self.0
            .msg("stencil-editing-planned-scope-ca", None)
            .unwrap()
    }
    pub fn msg_stencil_fallback(&self) -> String {
        self.0.msg("stencil-fallback", None).unwrap()
    }
    pub fn msg_stencil_settings(&self) -> String {
        self.0.msg("stencil-settings", None).unwrap()
    }
    pub fn msg_stencil_settings_planned(&self) -> String {
        self.0.msg("stencil-settings-planned", None).unwrap()
    }
    /// $stem (String) - argument
    pub fn msg_stencil_stem_is_already_open_exp<F0: AsRef<str>>(&self, stem: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("stem", stem.as_ref());
        self.0
            .msg("stencil-stem-is-already-open-exp", Some(_args))
            .unwrap()
    }
    pub fn msg_straight_connector(&self) -> String {
        self.0.msg("straight-connector", None).unwrap()
    }
    pub fn msg_strikethrough(&self) -> String {
        self.0.msg("strikethrough", None).unwrap()
    }
    pub fn msg_strikethrough_planned(&self) -> String {
        self.0.msg("strikethrough-planned", None).unwrap()
    }
    pub fn msg_style(&self) -> String {
        self.0.msg("style", None).unwrap()
    }
    pub fn msg_style_gallery_planned(&self) -> String {
        self.0.msg("style-gallery-planned", None).unwrap()
    }
    pub fn msg_sub_address_page(&self) -> String {
        self.0.msg("sub-address-page", None).unwrap()
    }
    pub fn msg_suggested_flowchart_network_floo(&self) -> String {
        self.0
            .msg("suggested-flowchart-network-floo", None)
            .unwrap()
    }
    /// $n (Number) - argument
    pub fn msg_svg_copied_n_bytes_vector_paste<F0: Into<FluentNumber>>(&self, n: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("n", n.into());
        self.0
            .msg("svg-copied-n-bytes-vector-paste", Some(_args))
            .unwrap()
    }
    pub fn msg_swap_portrait_landscape_swaps_wi(&self) -> String {
        self.0
            .msg("swap-portrait-landscape-swaps-wi", None)
            .unwrap()
    }
    pub fn msg_switch_documents_the_tab_strip_s(&self) -> String {
        self.0
            .msg("switch-documents-the-tab-strip-s", None)
            .unwrap()
    }
    pub fn msg_switch_windows(&self) -> String {
        self.0.msg("switch-windows", None).unwrap()
    }
    /// $name (String) - argument
    pub fn msg_switched_to_name<F0: AsRef<str>>(&self, name: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("name", name.as_ref());
        self.0.msg("switched-to-name", Some(_args)).unwrap()
    }
    pub fn msg_symbol(&self) -> String {
        self.0.msg("symbol", None).unwrap()
    }
    pub fn msg_symbol_planned(&self) -> String {
        self.0.msg("symbol-planned", None).unwrap()
    }
    /// $version (String) - argument
    pub fn msg_tarahi_v_cargo_pkg_version_clean<F0: AsRef<str>>(&self, version: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("version", version.as_ref());
        self.0
            .msg("tarahi-v-cargo-pkg-version-clean", Some(_args))
            .unwrap()
    }
    pub fn msg_task_panes(&self) -> String {
        self.0.msg("task-panes", None).unwrap()
    }
    pub fn msg_task_panes_shapes_pan_zoom_plann(&self) -> String {
        self.0
            .msg("task-panes-shapes-pan-zoom-plann", None)
            .unwrap()
    }
    pub fn msg_tell_me(&self) -> String {
        self.0.msg("tell-me", None).unwrap()
    }
    pub fn msg_tell_me_planned_for_a_later_mile(&self) -> String {
        self.0
            .msg("tell-me-planned-for-a-later-mile", None)
            .unwrap()
    }
    pub fn msg_tell_me_search_arrives_with_a_la(&self) -> String {
        self.0
            .msg("tell-me-search-arrives-with-a-la", None)
            .unwrap()
    }
    pub fn msg_tell_me_what_you_want_to_do(&self) -> String {
        self.0.msg("tell-me-what-you-want-to-do", None).unwrap()
    }
    pub fn msg_template_gallery_arrives_with_th(&self) -> String {
        self.0
            .msg("template-gallery-arrives-with-th", None)
            .unwrap()
    }
    pub fn msg_text(&self) -> String {
        self.0.msg("text", None).unwrap()
    }
    pub fn msg_text_align_left(&self) -> String {
        self.0.msg("text-align-left", None).unwrap()
    }
    pub fn msg_text_align_right(&self) -> String {
        self.0.msg("text-align-right", None).unwrap()
    }
    pub fn msg_text_box(&self) -> String {
        self.0.msg("text-box", None).unwrap()
    }
    pub fn msg_text_color(&self) -> String {
        self.0.msg("text-color", None).unwrap()
    }
    pub fn msg_text_color_planned(&self) -> String {
        self.0.msg("text-color-planned", None).unwrap()
    }
    pub fn msg_text_field(&self) -> String {
        self.0.msg("text-field", None).unwrap()
    }
    pub fn msg_text_formatting_arrives_with_a_l(&self) -> String {
        self.0
            .msg("text-formatting-arrives-with-a-l", None)
            .unwrap()
    }
    pub fn msg_text_shapes_planned(&self) -> String {
        self.0.msg("text-shapes-planned", None).unwrap()
    }
    pub fn msg_text_tool(&self) -> String {
        self.0.msg("text-tool", None).unwrap()
    }
    pub fn msg_text_tool_planned_double_click_e(&self) -> String {
        self.0
            .msg("text-tool-planned-double-click-e", None)
            .unwrap()
    }
    pub fn msg_the_current_drawing_has_unsaved(&self) -> String {
        self.0.msg("the-current-drawing-has-unsaved", None).unwrap()
    }
    pub fn msg_the_grid(&self) -> String {
        self.0.msg("the-grid", None).unwrap()
    }
    pub fn msg_the_last_page_cannot_be_deleted(&self) -> String {
        self.0.msg("the-last-page-cannot-be-deleted", None).unwrap()
    }
    pub fn msg_the_link_has_no_address(&self) -> String {
        self.0.msg("the-link-has-no-address", None).unwrap()
    }
    pub fn msg_the_picture_could_not_be_decoded(&self) -> String {
        self.0
            .msg("the-picture-could-not-be-decoded", None)
            .unwrap()
    }
    pub fn msg_the_picture_lands_on_the_clipboa(&self) -> String {
        self.0
            .msg("the-picture-lands-on-the-clipboa", None)
            .unwrap()
    }
    pub fn msg_the_svg_markup_lands_on_the_clip(&self) -> String {
        self.0
            .msg("the-svg-markup-lands-on-the-clip", None)
            .unwrap()
    }
    pub fn msg_theme_variants_planned(&self) -> String {
        self.0.msg("theme-variants-planned", None).unwrap()
    }
    pub fn msg_themes(&self) -> String {
        self.0.msg("themes", None).unwrap()
    }
    pub fn msg_themes_planned_gallery_folds_bro(&self) -> String {
        self.0
            .msg("themes-planned-gallery-folds-bro", None)
            .unwrap()
    }
    pub fn msg_thesaurus(&self) -> String {
        self.0.msg("thesaurus", None).unwrap()
    }
    pub fn msg_thesaurus_planned(&self) -> String {
        self.0.msg("thesaurus-planned", None).unwrap()
    }
    pub fn msg_this_page_is_itself_a_background(&self) -> String {
        self.0
            .msg("this-page-is-itself-a-background", None)
            .unwrap()
    }
    pub fn msg_this_page_renders_behind(&self) -> String {
        self.0.msg("this-page-renders-behind", None).unwrap()
    }
    pub fn msg_tick(&self) -> String {
        self.0.msg("tick", None).unwrap()
    }
    pub fn msg_tick_chevron(&self) -> String {
        self.0.msg("tick-chevron", None).unwrap()
    }
    pub fn msg_tick_dot(&self) -> String {
        self.0.msg("tick-dot", None).unwrap()
    }
    pub fn msg_tick_double_chevron(&self) -> String {
        self.0.msg("tick-double-chevron", None).unwrap()
    }
    pub fn msg_tick_ring(&self) -> String {
        self.0.msg("tick-ring", None).unwrap()
    }
    pub fn msg_tint_the_container_s_members_whi(&self) -> String {
        self.0
            .msg("tint-the-container-s-members-whi", None)
            .unwrap()
    }
    pub fn msg_title(&self) -> String {
        self.0.msg("title", None).unwrap()
    }
    pub fn msg_title_planned(&self) -> String {
        self.0.msg("title-planned", None).unwrap()
    }
    pub fn msg_to_page(&self) -> String {
        self.0.msg("to-page", None).unwrap()
    }
    pub fn msg_toggle_display_units_model_stays(&self) -> String {
        self.0
            .msg("toggle-display-units-model-stays", None)
            .unwrap()
    }
    pub fn msg_toggle_member_highlight(&self) -> String {
        self.0.msg("toggle-member-highlight", None).unwrap()
    }
    pub fn msg_tools(&self) -> String {
        self.0.msg("tools", None).unwrap()
    }
    pub fn msg_track_markup(&self) -> String {
        self.0.msg("track-markup", None).unwrap()
    }
    pub fn msg_translate(&self) -> String {
        self.0.msg("translate", None).unwrap()
    }
    pub fn msg_translate_planned(&self) -> String {
        self.0.msg("translate-planned", None).unwrap()
    }
    pub fn msg_transparent_background(&self) -> String {
        self.0.msg("transparent-background", None).unwrap()
    }
    pub fn msg_triple_tick(&self) -> String {
        self.0.msg("triple-tick", None).unwrap()
    }
    pub fn msg_triple_tick_dot(&self) -> String {
        self.0.msg("triple-tick-dot", None).unwrap()
    }
    pub fn msg_triple_tick_ring(&self) -> String {
        self.0.msg("triple-tick-ring", None).unwrap()
    }
    pub fn msg_underline(&self) -> String {
        self.0.msg("underline", None).unwrap()
    }
    pub fn msg_underline_planned(&self) -> String {
        self.0.msg("underline-planned", None).unwrap()
    }
    pub fn msg_undo_ctrl_z(&self) -> String {
        self.0.msg("undo-ctrl-z", None).unwrap()
    }
    /// $l (String) - argument
    pub fn msg_undo_l<F0: AsRef<str>>(&self, l: F0) -> String {
        let mut _args = FluentArgs::new();
        _args.set("l", l.as_ref());
        self.0.msg("undo-l", Some(_args)).unwrap()
    }
    pub fn msg_ungroup(&self) -> String {
        self.0.msg("ungroup", None).unwrap()
    }
    pub fn msg_ungroup_the_selected_group_s(&self) -> String {
        self.0.msg("ungroup-the-selected-group-s", None).unwrap()
    }
    pub fn msg_unsaved_changes(&self) -> String {
        self.0.msg("unsaved-changes", None).unwrap()
    }
    pub fn msg_untitled(&self) -> String {
        self.0.msg("untitled", None).unwrap()
    }
    pub fn msg_use_relative_paths_for_hyperlink(&self) -> String {
        self.0
            .msg("use-relative-paths-for-hyperlink", None)
            .unwrap()
    }
    pub fn msg_value(&self) -> String {
        self.0.msg("value", None).unwrap()
    }
    pub fn msg_variant_colors_planned(&self) -> String {
        self.0.msg("variant-colors-planned", None).unwrap()
    }
    pub fn msg_variant_connectors_planned(&self) -> String {
        self.0.msg("variant-connectors-planned", None).unwrap()
    }
    pub fn msg_variant_effects_planned(&self) -> String {
        self.0.msg("variant-effects-planned", None).unwrap()
    }
    pub fn msg_variants(&self) -> String {
        self.0.msg("variants", None).unwrap()
    }
    pub fn msg_vba_arrives_with_m7_vsdm(&self) -> String {
        self.0.msg("vba-arrives-with-m7-vsdm", None).unwrap()
    }
    pub fn msg_vector_svg(&self) -> String {
        self.0.msg("vector-svg", None).unwrap()
    }
    pub fn msg_view(&self) -> String {
        self.0.msg("view", None).unwrap()
    }
    pub fn msg_views(&self) -> String {
        self.0.msg("views", None).unwrap()
    }
    pub fn msg_visio_drawing(&self) -> String {
        self.0.msg("visio-drawing", None).unwrap()
    }
    pub fn msg_visio_stencil(&self) -> String {
        self.0.msg("visio-stencil", None).unwrap()
    }
    pub fn msg_visio_vsdx_family(&self) -> String {
        self.0.msg("visio-vsdx-family", None).unwrap()
    }
    pub fn msg_visual_aids(&self) -> String {
        self.0.msg("visual-aids", None).unwrap()
    }
    pub fn msg_visual_basic(&self) -> String {
        self.0.msg("visual-basic", None).unwrap()
    }
    pub fn msg_weight(&self) -> String {
        self.0.msg("weight", None).unwrap()
    }
    pub fn msg_whole_page(&self) -> String {
        self.0.msg("whole-page", None).unwrap()
    }
    pub fn msg_wide_arrow(&self) -> String {
        self.0.msg("wide-arrow", None).unwrap()
    }
    pub fn msg_window(&self) -> String {
        self.0.msg("window", None).unwrap()
    }
    pub fn msg_word_count(&self) -> String {
        self.0.msg("word-count", None).unwrap()
    }
    pub fn msg_word_count_planned(&self) -> String {
        self.0.msg("word-count-planned", None).unwrap()
    }
    pub fn msg_wrap_the_selection_in_this_conta(&self) -> String {
        self.0
            .msg("wrap-the-selection-in-this-conta", None)
            .unwrap()
    }
    pub fn msg_x_cancel_placement(&self) -> String {
        self.0.msg("x-cancel-placement", None).unwrap()
    }
    pub fn msg_zoom(&self) -> String {
        self.0.msg("zoom", None).unwrap()
    }
    pub fn msg_zoom_dialog_planned(&self) -> String {
        self.0.msg("zoom-dialog-planned", None).unwrap()
    }
    pub fn msg_zoom_in(&self) -> String {
        self.0.msg("zoom-in", None).unwrap()
    }
    pub fn msg_zoom_in_tip(&self) -> String {
        self.0.msg("zoom-in-tip", None).unwrap()
    }
    pub fn msg_zoom_menu(&self) -> String {
        self.0.msg("zoom-menu", None).unwrap()
    }
    pub fn msg_zoom_out(&self) -> String {
        self.0.msg("zoom-out", None).unwrap()
    }
    pub fn msg_zoom_out_tip(&self) -> String {
        self.0.msg("zoom-out-tip", None).unwrap()
    }
    pub fn msg_zoom_to_selection_shift_z(&self) -> String {
        self.0.msg("zoom-to-selection-shift-z", None).unwrap()
    }
}
