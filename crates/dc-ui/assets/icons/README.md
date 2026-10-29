# Vendored icon set — Pictogrammers Material Design Icons (MDI)

Source: @mdi/svg 7.4.47 via cdn.jsdelivr.net (files `svg/<name>.svg`),
retrieved 2026-10-18. Upstream: https://pictogrammers.com/library/mdi/
License: Apache-2.0 (see LICENSE-APACHE-2.0).

File names map 1:1 to the `Icon` enum asset stems in
`crates/dc-ui/src/ribbon.rs` (`Icon::asset`). MDI names per file:

| file | MDI icon |
|---|---|
| save | content-save |
| undo | undo |
| redo | redo |
| open | folder-open-outline |
| new | file-document-outline |
| paste | content-paste |
| cut | content-cut |
| copy | content-copy |
| painter | format-paint |
| fonta | format-text |
| bold | format-bold |
| italic | format-italic |
| underline | format-underline |
| bucket | format-color-fill |
| pen | draw |
| bringfront | arrange-bring-forward |
| sendback | arrange-send-backward |
| align | format-align-left |
| rotate | rotate-right |
| group | group |
| container | vector-square |
| callout, comment | comment-text-outline |
| field | code-json |
| spell | spellcheck |
| datagraphic | chart-bar |
| link | link-variant |
| refresh | refresh |
| swimlane | view-column-outline |
| check | check |
| picture | image |
| onlinepic | web |
| orientation | swap-horizontal-bold |
| pointer | cursor-default-outline |
| textbox | text-box-outline |
| newpage | file-document-plus-outline |
| theme | palette |
| fit | fit-to-screen |
| zoomin | magnify-plus |
| zoomout | magnify-minus |
| grid | grid |
| rulers | ruler-square |
| shapes | shape |
| macro | script-text-outline |
| shapesheet | table-edit |
| units | swap-horizontal |
| delete | trash-can-outline |
| lightbulb | lightbulb (Tell Me search box, 2026-10-22) |
| magnet | magnet (Snap & Glue, 2026-10-29) |
| connector | vector-line (Connector Tool, 2026-10-29) |
| format-vertical-align-top | format-vertical-align-top (#110, 2026-10-29) |
| format-vertical-align-center | format-vertical-align-center (#110, 2026-10-29) |
| format-vertical-align-bottom | format-vertical-align-bottom (#110, 2026-10-29) |
| arrow-expand-horizontal | arrow-expand-horizontal (#110, 2026-10-29) |
| arrow-expand-vertical | arrow-expand-vertical (#110, 2026-10-29) |

Posters note: MDI is an open-source set, not Microsoft artwork; the
cleanroom rule (no Microsoft assets) still holds. Fluent UI icons were
considered and rejected as default for being Microsoft artwork (their
MIT license would permit use, but the project posture avoids shipping
Microsoft bytes).
