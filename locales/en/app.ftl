### Tarahi UI strings — English. The source of truth for rendering.
###
### IDs are stable, hand-curated kebab-case names (a mechanical pass
### derived them from the pre-Fluent English keys, 2026-11-11). The
### runtime looks messages up by the ID at the call site (`t!("id")`);
### the completeness gate in crates/dc-ui/tests/i18n.rs checks that
### every routed ID exists in this file AND de.ftl, and that neither
### file carries an unreferenced message.
a-ruler-subdivision = a ruler subdivision
a-shape-intersection = a shape intersection
about-name = Tarahi
about-tarahi = About Tarahi
# $what (String) - the action name
# $why (String) - the deferral reason
action-deferred = { $what } deferred: { $why }
# $what (String) - the action name
action-display-only = { $what } is display-only
active = Active
activex-era-likely-cut-scope-cal = ActiveX-era — likely cut (scope call on #252)
add = Add
add-a-comment = Add a comment
# $next_ix (Number) - argument
# $hit (Number) - argument
add-connection-point-next-ix-on = add connection point { $next_ix } on shape { $hit }
add-in-hosting-arrives-with-m8 = Add-in hosting arrives with M8
add-in-manager = Add-in Manager
add-in-manager-m8 = Add-in manager — M8
add-ins = Add-ins
# $name (String) - argument
add-name-prop-row-test = add { $name } prop row (test)
add-to-new-container = Add to New Container
add-to-quick-shapes = Add to Quick Shapes
add-to-underlying-container = Add to Underlying Container
added-to-quick-shapes = added to Quick Shapes
align-against-the-page-instead-o = Align against the page instead of the primary selection
align-bottom = Bottom
align-bottom-edges = Align bottom edges
align-bottom-kind = bottom
align-center = Align Center
align-center-planned = Align center — planned
align-horizontal-middles = Align horizontal middles
align-left = Left
align-left-edges-first-selected = Align left edges (first-selected or page)
align-left-planned = Align left — planned
align-right = Right
align-right-edges = Align right edges
align-right-planned = Align right — planned
align-top = Top
align-top-edges = Align top edges
align-top-kind = top
align-vertical-centers = Align vertical centers
align-word-bottom = bottom
align-word-centers = centers
align-word-left = left
align-word-middles = middles
align-word-right = right
align-word-top = top
aligning-to-the-first-selected-s = aligning to the first-selected shape
aligning-to-the-page = aligning to the page
alignment-and-spacing-hints-whil = Alignment and spacing hints while dragging
alignment-boxes = Alignment boxes
all-changes-saved = All changes saved
# $page_count (Number) - argument
all-pages-page-count = All pages ({ $page_count })
already-aligned = already aligned
applies-to-the-ribbon-menus-and = Applies to the ribbon, menus, and dialogs. Drawing text is unaffected.
apply = Apply
apply-custom-size = Apply custom size
arrange = Arrange
arrange-all = Arrange All
arrange-windows-planned = Arrange windows — planned
assign-a-background-page-behind = Assign a background page behind this one
auto-layout-planned = Auto layout — planned
auto-page-size-planned = Auto page size — planned
auto-size = Auto Size
autoconnect = AutoConnect
autoconnect-arrows-the-108-remai = AutoConnect arrows — the #108 remainder
autoconnect-needs-a-2d-shape = AutoConnect needs a 2D shape
autocorrect-options = AutoCorrect Options
autocorrect-options-planned = AutoCorrect options — planned
automatic-resize = Automatic Resize
automatically-link = Automatically Link
background = Background
background-cleared = background cleared
background-page-name = background page: { $name }
backgrounds = Backgrounds
behavior = Behavior
blank-drawing = Blank drawing
bold = Bold
bold-planned = Bold — planned
boolean-shape-operations-planned = Boolean shape operations — planned
border-title = Border & Title
borders-titles-planned = Borders & titles — planned
bring-forward = Bring Forward
bring-forward-planned = Bring forward — planned
bring-to-front = Bring to Front
browse = Browse…
browse-for-an-existing-visio-pac = Browse for an existing Visio package.
# $ok (Number) - extracted count
# $failed (Number) - failed count
builtin-extracted-summary = Built-in stencils: { $ok } extracted, { $failed } failed — embedded copies in use
# $n (Number) - extracted file count
builtin-extracted-to = Extracted { $n } built-in stencil files to My Shapes/Built-ins
# $title (String) - stencil title
builtin-stencil-failed = built-in stencil failed to load: { $title }
bullets = Bullets
bullets-planned = Bullets — planned
cad-drawing = CAD Drawing
cad-drawing-import-planned-later = CAD drawing import — planned (later milestone)
calibri = Calibri
callout = Callout
callouts-planned = Callouts — planned
cancel = Cancel
# $name (String) - cell name
# $f (String) - the guarding formula
cell-is-formula-driven = { $name } is formula-driven ({ $f })
center = Center
chart = Chart
charts-planned-later-milestone = Charts — planned (later milestone)
checking-the-clipboard-for-a-pic = checking the clipboard for a picture…
choose-the-display-language-for = Choose the display language for the ribbon, menus, and dialogs
classic-arrow = Classic Arrow
clear = Clear
clear-planned-later-milestone = Clear — planned (later milestone)
click-a-shape-to-add-a-connectio = click a shape to add a connection point
clipboard = Clipboard
clipboard-image-could-not-be-enc = clipboard image could not be encoded
clipboard-planned-for-a-later-mi = Clipboard — planned for a later milestone
clipboard-probe-failed = clipboard probe failed
close = Close
close-group = × Close Group
close-stencil = Close Stencil...
# $name (String) - argument
closed-switched-to-name = closed; switched to { $name }
# $title (String) - argument
closed-title = closed { $title }
code = Code
color = Color
color-fill-bkgnd-window = Fill Background
color-fill-window = Fill Color
color-line-window = Line Color
color-text-window = Text Color
colors = Colors
com-add-ins = COM Add-Ins
comment-add = Add Comment
comment-add-inline = add comment
comment-new = New Comment
comments = Comments
# $target (Number) - argument
# $count (Number) - argument
comments-on-shape-target-count = Comments on shape { $target } ({ $count })
comments-planned = Comments — planned
configure-snap-and-glue-targets = Configure snap and glue targets
conn-point = Conn Point
conn-points = Connection Points
# $ix (Number) - argument
connection-point-ix-is-inherited = connection point { $ix } is inherited from the master — edit it there
# $ix (Number) - argument
connection-point-ix-removed = connection point { $ix } removed
# $ix (Number) - argument
connection-point-ix-selected-del = connection point { $ix } selected (Delete removes it)
connection-point-next-ix-added-t = connection point { $next_ix } added to shape { $hit }
connection-point-tool-off = connection point tool off
connector = Connector
connector-drawn-free-ends = connector drawn (free ends)
connector-end-released = connector end released
connector-glued-at-both-ends = connector glued at both ends
connector-glued-at-one-end = connector glued at one end
# $id (Number) - shape id
connector-glued-shape = connector glued to shape { $id }
# $id (Number) - shape id
connector-glued-shape-point = connector glued to the connection point of shape { $id }
connector-is-glued-at-both-ends = connector is glued at both ends — drag past the snap radius to release, or drag an endpoint or its shapes
# $reason (String) - argument
connector-move-refused-reason = connector move refused: { $reason }
# $release_ends (Number) - argument
connector-moved-release-ends-glu = connector moved — { $release_ends } glued end(s) released past the snap radius
# $id (Number) - shape id
connector-stays-glued = connector stays glued to shape { $id }
connector-tool-drag-from-one-sha = Connector Tool: drag from one shape to another (Esc to finish)
connector-tool-status = Connector tool — draw a connector between shapes or points (Esc to finish)
connector-tool-tip = Connector tool — draw a connector between shapes or points
connectors = Connectors
connpoint-tool-status = Connection Point tool: click a shape to add a connection point
container = Container
container-flyout =   Container  ›
container-margins-planned = Container margins — planned
# $style (String) - argument
container-style-style-applied = container style { $style } applied
# $style (String) - argument
container-style-style-not-found = container style { $style } not found
container-styles = Container Styles
container-tools = Container Tools
containers = Containers…
containers-planned = Containers — planned
containers-wrap-tip = Wrap the selection in a container style
contgal-prompt = Wrap the selection in a container style:
controls = Controls
copy = Copy
copy-as-picture = Copy as Picture
# $e (String) - argument
copy-as-picture-failed-e = copy as picture failed: { $e }
copy-as-picture-menu = Copy as Picture…
copy-picture = Copy Picture
copy-planned = Copy — planned
# $address (String) - argument
# $e (String) - argument
could-not-open-address-e = could not open { $address }: { $e }
# $path (String) - file path
# $e (String) - error text
could-not-read-path = could not read { $path }: { $e }
create-a-background-page-planned = Create a background page — planned (Design ▸ Backgrounds assigns today)
create-a-stencil-planned = Create a stencil — planned
create-new-data-graphic = Create New Data Graphic
cross-functional = Cross-functional
crow-s-foot = Crow's Foot
crow-s-foot-tick = Crow's Foot + Tick
current = Current:
current-page = Current page
# $row_ix (Number) - argument
curve-control-customized-row-row = curve control customized — row { $row_ix }'s route formula replaced with the dragged value
curved-connector = Curved Connector
custom = Custom
custom-import = Custom Import
cut = Cut
cut-planned = Cut — planned
dash = Dash
dash-dot = Dash-dot
dash-dot-dot = Dash-dot-dot
data = Data
data-graphics = Data Graphics
data-graphics-m6 = Data graphics (M6)
data-linking-m6 = Data linking (M6)
date-time = Date & Time
date-time-field-planned = Date & time field — planned
decrease-indent = Decrease Indent
decrease-indent-planned = Decrease indent — planned
# $entry (String) - default stencil entry
# $e (String) - error text
default-stencil-failed = default stencil { $entry } failed to load: { $e }
define-shape-data = Define Shape Data
define-shape-data-m6 = Define shape data (M6)
delete = Delete
delete-comment = Delete Comment
delete-comment-planned = Delete comment — planned
delete-page = Delete Page
design = Design
design-mode = Design Mode
design-mode-likely-cut-scope-cal = Design mode — likely cut (scope call on #252)
developer = Developer
diagram-design = DIAGRAM · DESIGN
diagram-parts = Diagram Parts
diamond-dot = Diamond + Dot
diamond-ring = Diamond + Ring
disband-container = Disband Container
discard-new = Discard & New
display-data-graphics = Display Data Graphics
display-language = Display language:
display-options = Display Options
dist-h = Dist H
dist-v = Dist V
distribute-horizontal-spacing = distribute horizontal spacing
distribute-needs-three-or-more-s = distribute needs three or more shapes
distribute-vertical-spacing = distribute vertical spacing
document-closed = document closed
document-inspection-arrives-with = Document inspection arrives with a later milestone
document-masters = Document Masters
document-protection-arrives-with = Document protection arrives with a later milestone
document-stencil = Document Stencil
document-stencil-planned = Document stencil — planned
don-t-save = Don't Save
dot = Dot
double-arrow-filled = Double Arrow (Filled)
double-arrow-open = Double Arrow (Open)
double-tick = Double Tick
double-tick-dot = Double Tick + Dot
double-tick-ring = Double Tick + Ring
drawing-aids = Drawing aids
drawing-explorer = Drawing Explorer
drawing-explorer-planned = Drawing explorer — planned
# $n (Number) - untitled document number
drawing-n = Drawing{ $n }
drawing-scale-planned = Drawing scale — planned
drawings-you-open-will-appear-he = Drawings you open will appear here.
duplicate = Duplicate
duplicate-page = Duplicate Page
dynamic-grid = Dynamic Grid
edit = Edit
edit-comment = Edit comment
edit-data-graphic = Edit Data Graphic
edit-hyperlink = edit hyperlink
edit-stencil = Edit Stencil
edit-text = Edit Text…
edit-the-selected-shape-s-hyperl = Edit the selected shape's hyperlinks (Ctrl+K)
editing = Editing
# $shape_id (Number) - argument
editing-text-of-shape-shape-id = editing text of shape { $shape_id }
effects = Effects
ellipse-tool = Ellipse Tool
ellipse-tool-planned = Ellipse tool — planned
endpoint-begin = begin
# $reason (String) - argument
endpoint-drag-refused-reason = endpoint drag refused: { $reason }
endpoint-end = end
# $hit (String) - localized snap target text
endpoint-glued-to = endpoint glued to { $hit }
# $name (String) - argument
endpoint-moved-but-name-is-formu = endpoint moved, but { $name } is formula-driven and kept
even-horizontal-spacing-3-shapes = Even horizontal spacing (3+ shapes)
even-vertical-spacing-3-shapes = Even vertical spacing (3+ shapes)
# $f (String) - argument
eventdblclick-f-macro-and-naviga = EventDblClick '{ $f }' — macro and navigation actions arrive with a later milestone
export-entry-desc = Save the drawing in another format
export-failed-e = Export failed: { $e }
export-failed-worker-lost = Export failed: worker lost
export-finished = Export finished.
export-page-sub = Save the drawing in another format.
export-pdf-hint = Document with selectable text (all pages in one file)
export-png-hint = Raster image at the chosen density
export-save-as-label = Save as
export-start = Export…
export-svg-hint = Vector graphics — the renderer's own format
# $n (Number) - files written
exported-files =
    { $n ->
        [one] Exported { $n } file.
       *[other] Exported { $n } files.
    }
exporting = Exporting…
external-data = External Data
external-data-window = External Data Window
external-data-window-m6 = External data window (M6)
extracting-built-in-stencils-to = Extracting built-in stencils to My Shapes ({ $done }/{ $total })…
fields-planned = Fields — planned
file = File
files-you-open-will-appear-here = Files you open will appear here.
fill = Fill
filled-arrow-plain = Filled Arrow (Plain)
filled-circle = Filled Circle
filled-square = Filled Square
filter-pdf = PDF (document)
filter-png = PNG (raster image)
filter-svg = SVG (vector graphics)
find = Find
find-planned-later-milestone = Find — planned (later milestone)
fine-dash = Fine Dash
fine-dot = Fine Dot
first-page = First page
fit-the-page-width-planned = Fit the page width — planned
fit-the-whole-page-in-the-window = Fit the whole page in the window
fit-to-contents = Fit to Contents
flip-horizontal = Flip Horizontal
flip-vertical = Flip Vertical
floor-plan = Floor Plan
flowchart = Flowchart
font = Font
font-decrease = Decrease Font Size
font-decrease-tip = Decrease font size
font-family-planned = Font family — planned
font-increase = Increase Font Size
font-increase-tip = Increase font size
font-size = font size
font-size-planned = Font size — planned
foreground = Foreground
fork-ring = Fork + Ring
format-painter = Format Painter
format-painter-cancelled = format painter cancelled
# $sid (Number) - argument
format-painter-click-a-shape-to = format painter: click a shape to apply shape { $sid }'s formatting
# $sid (Number) - argument
format-painter-click-shapes-to-a = format painter: click shapes to apply shape { $sid }'s formatting (Esc to finish)
format-shape = Format Shape
format-shape-fill = Format Shape Fill…
format-shape-line = Format Shape Line…
format-shape-menu = Format Shape…
format-shape-text = Format Shape Text…
freeform = Freeform
freeform-drag-on-the-page-to-dra = Freeform: drag on the page to draw freehand geometry (Esc to finish)
freeform-off = freeform off
# $id (Number) - argument
freeform-shape-id = freeform shape { $id }
# $segs (Number) - argument
freeform-shape-created-segs-segm =
    { $segs ->
        [one] freeform shape created (one segment — drag the vertex handles to edit)
       *[other] freeform shape created ({ $segs } segments — drag the vertex handles to edit)
    }
freeform-stroke-too-short-to-dra = freeform stroke too short to draw
glue = Glue
glue-to = Glue to:
go = Go
gradient-horiz = Gradient horiz.
gradient-radial = Gradient radial
gradient-vert = Gradient vert.
grid = Grid
group = Group
# $names (String) - joined page/group names
group-breadcrumb = Page › { $names }
group-flyout =   Group  ›
group-needs-2-shapes-on-the-page = Group needs 2+ shapes on the page
group-the-selection-one-undo-ste = Group the selection (one undo step; nests into an existing group)
grow-with-the-members-planned = Grow with the members — planned
# $v (String) - guide coordinate
guide-added-horizontal = horizontal guide at { $v } added
# $v (String) - guide coordinate pair
guide-added-point = guide point at { $v } added
# $v (String) - guide coordinate
guide-added-vertical = vertical guide at { $v } added
# $id (Number) - argument
guide-id = guide { $id }
guides = Guides
guides-planned-113-s-objects-exi = Guides — planned (#113's objects exist; the toggle is planned)
heading-style = Heading Style
heading-style-gallery-planned = Heading style gallery — planned
height = Height
# $g (String) - guard formula
height-not-resized-guard = height not resized: { $g } (width only)
help-about = Help / About
high-192-dpi = High (192 dpi)
highlight-members = Highlight Members
hl-address = Address
hl-description = Description
hl-subaddress = Sub-address
home = Home
hyperlink = Hyperlink
hyperlink-menu = Hyperlink…
hyperlink-new = New…
hyperlink-pages = Pages…
hyperlinks = Hyperlinks
illustrations = Illustrations
increase-indent = Increase Indent
increase-indent-planned = Increase indent — planned
info = Info
inherited = inherited
inherited-from-the-shape-s-maste = Inherited from the shape's master — edit it to override it on this shape
insert = Insert
insert-a-png-or-jpeg-picture-fro = Insert a PNG or JPEG picture from a file
insert-control = Insert Control
insert-page = Insert Page…
insert-pages = Pages
inspect-drawing = Inspect Drawing…
inspect-the-selected-shape-s-cel = Inspect the selected shape's cells
italic = Italic
italic-planned = Italic — planned
jump-size = Jump size
justify = Justify
justify-planned = Justify — planned
# $key (String) - keytip key
keytip-unavailable = { $key } is unavailable
# $label (String) - argument
label-a-choice-cannot-contain = { $label }: a choice cannot contain ';'
# $label (String) - argument
label-dialog-planned-for-a-later = { $label } dialog — planned for a later milestone
# $label (String) - argument
label-not-a-number = { $label }: not a number
landscape = Landscape
language = Language
language-menu = Language…
language-name = English
language-settings-planned = Language settings — planned
lasso-select = Lasso Select
lasso-select-drag-a-freehand-tra = Lasso Select: drag a freehand trail around shapes (Esc to finish)
lasso-select-off = lasso select off
# $n (Number) - argument
lasso-selected-n-shape-s =
    { $n ->
        [one] lasso selected one shape
       *[other] lasso selected { $n } shapes
    }
last-page = Last page
layer = Layer
# $id (Number) - layer id
# $e (String) - error text
layer-composite-failed = layer { $id }: { $e }
# $ix (Number) - layer row index
layer-ix-fallback = layer { $ix }
layout = Layout
layout-options = Layout Options
layout-options-planned = Layout options — planned
layout-size = Layout Size
layout-size-planned = Layout size — planned
layout-spacing-planned = Layout spacing — planned
legend = Legend
legends-m6-visio-paints-legend-o = Legends (M6; Visio paints Legend on Insert, not Data)
line = Line
line-color-label = Line color
# $factor (Number) - jump factor
line-jump-factor-set = line jump size ×{ $factor }
line-jumps = Line jumps
line-jumps-hidden = line jumps hidden
line-jumps-shown = line jumps shown
# $reason (String) - argument
line-move-refused-reason = line move refused: { $reason }
# $name (String) - argument
line-moved-but-name-is-formula-d = line moved, but { $name } is formula-driven and kept
line-tool = Line Tool
line-tool-planned = Line tool — planned
line-weight = Line weight
link-data-to-shapes = Link Data to Shapes
links = Links
location = Location
lock-container = Lock Container
# $lock (String) - argument
lock-is-set-protection-section = { $lock } is set (Protection section)
lock-the-container-against-membe = Lock the container against membership edits — planned
long-arrow = Long Arrow
long-dash = Long Dash
macro-security = Macro Security
macro-security-m7 = Macro security — M7
macros = Macros
macros-arrive-with-m7-vba-vsdm-m = macros arrive with M7 (VBA/.vsdm milestone)
manage-reports = Manage Reports
margins = Margins
markup = Markup
markup-tracking-planned-no-marku = Markup tracking — planned (no markup model yet)
master = Master
# $id (Number) - master id
master-armed-click-place = master { $id } armed — click the canvas to place
# $mid (Number) - argument
master-mid = master { $mid }
member-highlight-off = member highlight off
member-highlight-on = member highlight on
membership = Membership
metric-stencils = Metric Stencils
middle = Middle
mm-in = mm/in
more-shapes = More Shapes
move-container-and-members = move container and members
move-down = Move Down
# $shape_id (Number) - argument
move-line-endpoint-shape-shape-i = move line endpoint (shape { $shape_id })
move-shapes = move shapes
move-up = Move Up
name = Name
name-page-id = { $name } (page { $id })
name-u-master-id = { $name_u } (master { $id })
network-diagram = Network Diagram
new-background-page = New Background Page
new-blank-drawing = New blank drawing
new-blank-drawing-unsaved = new blank drawing (unsaved)
new-choice = new choice
new-page = New Page
new-stencil = New Stencil
new-window = New Window
new-window-planned-single-window = New window — planned (single-window today)
next-comment = Next Comment
next-comment-planned = Next comment — planned
next-page = Next page
no-background = No background
no-background-pages-right-click = No background pages. Right-click a page tab and pick "Set as Background".
no-comments-on-this-shape-yet = No comments on this shape yet.
no-document-open = No document open
no-formula = No Formula
no-hyperlinks-on-this-shape = No hyperlinks on this shape.
# $key (String) - argument
no-keytip-key = no keytip { $key }
no-matching-shapes = No matching shapes.
no-picture-on-the-clipboard = no picture on the clipboard
no-shapes = No shapes.
# $q (String) - search text
no-templates-match = No templates match “{ $q }”.
none = None
not-saved-yet = Not saved yet
nothing-on-this-page-matches-the = (nothing on this page matches the criterion)
nothing-selected-to-copy = nothing selected to copy
nothing-to-redo = nothing to redo
nothing-to-undo = nothing to undo
nudge-selection = nudge selection
numbering = Numbering
numbering-planned = Numbering — planned
nurbs-control-customized-the-e-f = NURBS control customized — the E formula's control point moved
ok = OK
online-pictures = Online Pictures
online-pictures-planned = Online pictures — planned
only-png-and-jpeg-pictures-are-s = only PNG and JPEG pictures are supported
open-2-prong = Open 2-Prong
open-a-visio-file = Open a Visio file…
open-barbed = Open Barbed
open-circle = Open Circle
open-diamond = Open Diamond
open-double-chevron-large = Open Double Chevron (Large)
open-existing-stencil = Open Existing Stencil...
# $e (String) - argument
open-failed-e = open failed: { $e }
# $path (String) - argument
open-failed-no-pages = open failed: { $path } has no pages
open-failed-the-load-worker-died = open failed: the load worker died
open-file-cta = Open a file…
open-file-title = Open a file
open-fork = Open Fork
# $group_label (String) - argument
open-group-label = Open { $group_label }
open-square = Open Square
# $e (String) - error text
open-stencil-failed-e = open stencil failed: { $e }
open-triangle-convex-base = Open Triangle (Convex Base)
open-triangle-large = Open Triangle (Large)
open-triangle-medium = Open Triangle (Medium)
open-triangle-medium-large = Open Triangle (Medium-Large)
open-triangle-small = Open Triangle (Small)
open-v-tall = Open V (Tall)
open-vsdx-vstx-vssx-ctrl-o = Open .vsdx / .vstx / .vssx (Ctrl+O)
open-wide = Open Wide
# $address (String) - hyperlink address
opened-address = opened { $address }
# $title (String) - stencil title
opened-builtin-metric = opened { $title } (metric)
# $title (String) - stencil title
opened-builtin-myshapes = opened { $title } (My Shapes)
# $name (String) - group name
opened-group = Opened group { $name } — Esc or Close Group to return
# $path (String) - file path
opened-path = opened { $path }
# $stem (String) - argument
opened-stencil-stem = opened stencil { $stem }
# $name (String) - file name
opening-file = Opening { $name }…
operations = Operations
options = Options
options-arrive-with-a-later-mile = Options arrive with a later milestone
org-chart = Org Chart
orientation = Orientation
output-file = Output file
page-breaks = Page Breaks
page-breaks-planned = Page breaks — planned
page-name = Page name
# $name (String) - argument
page-name-is-a-foreground-page-a = page { $name } is a foreground page again
# $name (String) - argument
page-name-marked-as-a-background = page { $name } marked as a background
# $sub (String) - sub-address page name
page-not-found = page { $sub } not found
page-number = Page Number
page-number-field-planned = Page-number field — planned
page-setup = Page Setup
page-setup-menu = Page Setup…
page-size = Page Size
page-size-iso-ansi-presets-or-cu = Page size — ISO/ANSI presets or custom
# $w (String) - width value
# $h (String) - height value
page-size-set = page size { $w } × { $h }
# $n (Number) - page number, 1-based
# $id (Number) - page id
page-tab-tooltip = page { $n } (id { $id })
page-width = Page Width
# $to (Number) - argument
paint-formatting-onto-shape-to = paint formatting onto shape { $to }
# $n (Number) - argument
# $to (Number) - argument
# $n (Number) - argument
# $to (Number) - argument
painter-cells = { $n } formatting cells copied to shape { $to }
# $n (Number) - argument
# $to (Number) - argument
painter-cells-sticky = { $n } formatting cells copied to shape { $to } (click more shapes, Esc to finish)
paragraph = Paragraph
paste = Paste
pick-an-output-file-first = Pick an output file first
# $w (Number) - pixel width
# $h (Number) - pixel height
# $factor (Number) - zoom factor
picture-copied = picture copied ({ $w }×{ $h } px at { $factor }×)
# $w (Number) - pixel width
# $h (Number) - pixel height
# $w_in (Number) - width in inches
# $h_in (Number) - height in inches
picture-inserted = picture inserted ({ $w }×{ $h } px at { $w_in }×{ $h_in } in)
pictures = Pictures
pointer-tool = Pointer Tool
pointer-tool-select-and-drag-esc = Pointer tool — select and drag (Esc from the Connector Tool)
portrait = Portrait
presentation-mode = Presentation Mode
presentation-mode-planned = Presentation mode — planned
preset-sizes = Preset sizes
previous-comment = Previous Comment
previous-comment-planned = Previous comment — planned
previous-page = Previous page
print-384-dpi = Print (384 dpi)
print-stub = Print…
print-the-current-drawing = Print the current drawing
printing-arrives-with-m6 = Printing arrives with M6
proofing = Proofing
protect-drawing = Protect Drawing…
protection = Protection
quick-import = Quick Import
quick-shapes = Quick Shapes
quick-styles = Quick Styles
rail-export = Export
rail-new = New
rail-print = Print
re-layout-page = Re-layout Page
recent = Recent
rectangle-tool = Rectangle Tool
rectangle-tool-planned = Rectangle tool — planned
redo-ctrl-y = Redo (Ctrl+Y)
# $l (String) - argument
redo-l = redo: { $l }
refresh-all = Refresh All
release-the-members-and-delete-t = Release the members and delete the container (one undo step)
remove = Remove
remove-background-mark = Remove Background Mark
# $ix (Number) - argument
remove-connection-point-ix = remove connection point { $ix }
remove-from-quick-shapes = Remove from Quick Shapes
removed-from-quick-shapes = removed from Quick Shapes
rename-page = Rename Page
rendering-scrim = Rendering…
reorder-pages = Reorder Pages
reorder-pages-menu = Reorder Pages…
replace = Replace
replace-planned-later-milestone = Replace — planned (later milestone)
reply = Reply
report-definitions-planned = Report definitions — planned
reports = Reports
resize-refused-shape-is-protecte = resize refused: shape is protected
resolution = Resolution
resolution-dpi = Resolution (dpi):
review = Review
review-new-comment = New comment
reviewing-pane = Reviewing Pane
reviewing-pane-planned = Reviewing pane — planned
right-angle-connector = Right-Angle Connector
ring = Ring
ring-tick = Ring + Tick
rotate-left = Rotate Left
rotate-left-90 = Rotate Left 90°
rotate-right = Rotate Right
rotate-right-90 = Rotate Right 90°
rotate-shapes = rotate shapes
rotate-the-selection-90-clockwis = Rotate the selection 90° clockwise
rotate-the-selection-90-counter = Rotate the selection 90° counter-clockwise
rounding = Rounding
rtl-text-menu = Right-to-Left Text
rtl-text-off = text set left-to-right
rtl-text-on = text set right-to-left
ruler-subdivisions = Ruler subdivisions
rulers = Rulers
save = Save
save-as = Save As
save-changes = Save changes?
# $name (String) - argument
save-changes-to-name = Save changes to { $name }?
save-ctrl-s = Save (Ctrl+S)
# $e (String) - error text
save-failed-e = save failed: { $e }
save-new = Save & New
save-stencil = Save Stencil
save-the-stencil-planned = Save the stencil — planned
# $path (String) - file path
saved-path = saved { $path }
scale = Scale
screen-96-dpi = Screen (96 dpi)
search-results = Search Results
search-shapes = Search Shapes
search-templates = Search Templates
select = Select
select-a-container-to-disband = Select a container to disband
select-a-format = Select a format
select-a-group-to-ungroup = Select a group to ungroup
select-a-shape-to-copy-formattin = select a shape to copy formatting from first
select-a-shape-to-edit-its-hyper = Select a shape to edit its hyperlinks
select-all = Select All
select-by = Select by
select-by-type = Select by Type
select-by-type-menu = Select by Type…
select-by-type-pick-a-criterion = select by type: pick a criterion and value
select-contents = Select Contents
select-every-shape-on-the-page-c = Select every shape on the page (Ctrl+A)
select-shapes-by-type-master-or = Select shapes by type, master, or layer
select-the-container-s-members-p = Select the container's members — planned
select-the-shapes-to-contain-fir = select the shapes to contain first
select-two-or-more-shapes-to-ali = select two or more shapes to align
# $label (String) - the value label
# $n (Number) - matching shape count
select-value-n-shapes = { $label } ({ $n ->
    [one] one shape
   *[other] { $n } shapes
})
selected-all-shapes = selected all shapes
# $n (Number) - selected shape count
# $crit (String) - criterion label
selected-n-by-criterion =
    { $n ->
        [one] selected one shape by { $crit }
       *[other] selected { $n } shapes by { $crit }
    }
# $shape_id (Number) - argument
selected-shape-shape-id = selected shape { $shape_id }
selection = Selection
selection-pane = Selection Pane
selection-pane-planned-112-shell = Selection pane — planned (#112 shell depth)
send-backward = Send Backward
send-backward-planned = Send backward — planned
send-the-drawing-to-a-printer-or = Send the drawing to a printer or save it as a PDF.
send-to-back = Send to Back
set-as-background = Set as Background
set-connector-route-style = set connector route style
set-page-size = set page size
set-proofing-language = Set Proofing Language
set-proofing-language-planned = Set proofing language — planned
shape-behavior-planned = Shape behavior — planned
shape-data = Shape Data
shape-data-menu = Shape Data…
shape-data-window = Shape Data Window
shape-data-window-m6 = Shape data window (M6)
shape-design = Shape Design
# $dup_id (Number) - argument
# $sid (Number) - argument
shape-dup-id-connected-to-sid-au = shape { $dup_id } connected to { $sid } (AutoConnect)
shape-effects-planned = Shape effects — planned
shape-extensions = Shape extensions
shape-fill-color = Shape fill color
shape-handles = Shape handles
# $id (Number) - argument
shape-id-s-alignment-box = shape { $id }'s alignment box
# $id (Number) - argument
shape-id-s-connection-point = shape { $id }'s connection point
# $id (Number) - argument
shape-id-s-extension = shape { $id }'s extension
# $id (Number) - argument
shape-id-s-handle = shape { $id }'s handle
# $id (Number) - argument
shape-id-s-vertex = shape { $id }'s vertex
shape-intersections = Shape intersections
shape-protection-planned = Shape protection — planned
shape-report = Shape Report
shape-reports-planned = Shape reports — planned
# $sid (Number) - argument
shape-sid = shape { $sid }
shape-styles = Shape Styles
shape-text = Shape text
shape-type = Shape type
shape-vertices = Shape vertices
shapes = Shapes
shapesheet = ShapeSheet
# $name (String) - argument
shapesheet-name = ShapeSheet — { $name }
share = Share
share-a-copy = Share a Copy…
share-a-copy-of-the-drawing = Share a copy of the drawing
share-a-copy-of-the-drawing-with = Share a copy of the drawing with other people.
sharing-arrives-with-m6 = Sharing arrives with M6
short-dash = Short Dash
show = Show
show-connection-points-planned-m = Show connection points — planned (moved from Show; Visio lists it here)
show-document-stencil = Show Document Stencil
show-hide = Show/Hide
show-hide-comments = Show/Hide Comments
show-hide-comments-planned-view = Show/hide comments — planned (View's toggle is wired today)
show-hide-data-graphics-on-the-p = Show/hide data graphics on the page (M6)
show-line-jumps = Show line jumps
show-markup = Show Markup
show-markup-planned = Show markup — planned
show-shapesheet = Show ShapeSheet
size = Size
size-cell-is-formula-driven-f = { $size_cell } is formula-driven ({ $f })
size-pt = Size (pt)
size-the-container-to-its-member = Size the container to its members — planned
slanted-tick = Slanted Tick
small-arrow = Small Arrow
snap = Snap
snap-conn-points = Connection points
snap-dynamic-grid = Dynamic grid
snap-glue = Snap & Glue
snap-to = Snap to:
snapglue-note = Settings are stored in the drawing. Guides as targets arrive with #113; connector glue with #108.
solid = Solid
spacing = Spacing
spell-check-planned = Spell check — planned
spelling = Spelling
splash-wordmark = tarahi
split = Split
split-view-planned = Split view — planned
ss-section-1d-endpoints = 1-D Endpoints
ss-section-connection-points = Connection Points
ss-section-control-points = Control Points
ss-section-fill-gradient-stops = Fill Gradient Stops
# $ix (Number) - geometry section index
ss-section-geometry-n = Geometry { $ix }
ss-section-layer-membership = Layer Membership
ss-section-line-gradient-stops = Line Gradient Stops
ss-section-page-layout = Page Layout
ss-section-page-properties = Page Properties
ss-section-print-setup = Print Setup
ss-section-ruler-grid = Ruler & Grid
ss-section-shape-data = Shape Data
ss-section-smart-tags = Smart Tags
ss-section-text-fields = Text Fields
ss-section-text-transform = Text Transform
ss-section-user-defined-cells = User-defined Cells
ss-shape-not-found = shape not found
stamp-tool = Stamp Tool
stamp-tool-planned = Stamp tool — planned
start-an-empty-drawing = Start an empty drawing
start-an-empty-drawing-on-a-lett = Start an empty drawing on a letter-size page.
start-an-empty-us-letter-drawing = Start an empty US-Letter drawing (Ctrl+N)
status = Status
stealth-dart = Stealth Dart
stencil = Stencil
stencil-editing-planned-scope-ca = Stencil editing — planned (scope call on #252)
stencil-fallback = stencil
stencil-settings = Stencil Settings
stencil-settings-planned = Stencil settings — planned
# $stem (String) - argument
stencil-stem-is-already-open-exp = stencil { $stem } is already open — expanded
straight-connector = Straight Connector
strikethrough = Strikethrough
strikethrough-planned = Strikethrough — planned
style = Style
style-gallery-planned = Style gallery — planned
sub-address-page = Sub-address page
suggested-flowchart-network-floo = Suggested: flowchart · network · floor plan
# $n (Number) - argument
svg-copied-n-bytes-vector-paste = SVG copied ({ $n } bytes, vector — paste into editors that accept SVG text)
swap-portrait-landscape-swaps-wi = Swap portrait/landscape (swaps width and height)
switch-documents-the-tab-strip-s = Switch documents — the tab strip's Window menu does this today (#192)
switch-windows = Switch Windows
# $name (String) - argument
switched-to-name = switched to { $name }
symbol = Symbol
symbol-planned = Symbol — planned
# $version (String) - argument
tarahi-v-cargo-pkg-version-clean = Tarahi v{ $version } — cleanroom Visio-compatible diagramming
task-panes = Task Panes
task-panes-shapes-pan-zoom-plann = Task panes (Shapes, Pan & Zoom…) — planned
tell-me = Tell Me
tell-me-planned-for-a-later-mile = Tell Me — planned for a later milestone
tell-me-search-arrives-with-a-la = Tell Me search arrives with a later milestone
tell-me-what-you-want-to-do = Tell me what you want to do
template-gallery-arrives-with-th = Template gallery arrives with the M5 template flow
text = Text
text-align-left = Align Left
text-align-right = Align Right
text-box = Text Box
text-color = Text Color
text-color-tip = Text color of the selection
text-field = Text Field
text-formatting-arrives-with-a-l = Text formatting arrives with a later milestone
text-shapes-planned = Text shapes — planned
text-tool = Text Tool
text-tool-planned-double-click-e = Text tool — planned (double-click edits text today)
the-current-drawing-has-unsaved = The current drawing has unsaved changes.
the-drawing = the drawing
the-grid = the grid
the-last-page-cannot-be-deleted = the last page cannot be deleted
the-link-has-no-address = the link has no address
the-picture-could-not-be-decoded = the picture could not be decoded
the-picture-lands-on-the-clipboa = The picture lands on the clipboard as a PNG bitmap.
the-svg-markup-lands-on-the-clip = The SVG markup lands on the clipboard as vector text.
theme-variants-planned = Theme variants — planned
themes = Themes
themes-planned-gallery-folds-bro = Themes — planned (gallery folds Browse for Themes, Theme Colors/Effects/Connectors)
thesaurus = Thesaurus
thesaurus-planned = Thesaurus — planned
this-page-is-itself-a-background = (this page is itself a background page)
this-page-renders-behind = This page renders behind:
tick = Tick
tick-chevron = Tick + Chevron
tick-dot = Tick + Dot
tick-double-chevron = Tick + Double Chevron
tick-ring = Tick + Ring
tint-the-container-s-members-whi = Tint the container's members while it is selected
title = Title
title-planned = Title — planned
to-page = To Page
toggle-display-units-model-stays = Toggle display units (model stays inches)
toggle-member-highlight = toggle member highlight
tools = Tools
track-markup = Track Markup
translate = Translate
translate-planned = Translate — planned
transparent-background = Transparent background
triple-tick = Triple Tick
triple-tick-dot = Triple Tick + Dot
triple-tick-ring = Triple Tick + Ring
underline = Underline
underline-planned = Underline — planned
# $name (String) - property name
# $v (String) - the added choice
undo-add-choice = add { $name } choice { $v }
# $id (Number) - shape id
undo-add-connector = add connector { $id }
# $n (Number) - shape count
# $c (String) - container name
undo-add-n-to-container =
    { $n ->
        [one] add one shape to container { $c }
       *[other] add { $n } shapes to container { $c }
    }
# $what (String) - the aligned edge word
undo-align-what = align { $what }
# $style (String) - container style master NameU
undo-apply-container-style = apply container style { $style }
# $id (Number) - shape id
undo-autoconnect = AutoConnect shape { $id }
undo-ctrl-z = Undo (Ctrl+Z)
# $n (Number) - shape count
undo-delete-n-shapes =
    { $n ->
        [one] delete one shape
       *[other] delete { $n } shapes
    }
# $id (Number) - shape id
undo-delete-shape = delete shape { $id }
# $n (Number) - released member count
undo-disband-container =
    { $n ->
        [one] disband container (one member released)
       *[other] disband container ({ $n } members released)
    }
# $sid (Number) - connector shape id
# $end (String) - localized endpoint word
undo-glue-connector = glue connector { $sid } { $end }
# $l (String) - argument
undo-l = undo: { $l }
# $id (Number) - connector id
undo-move-connector = move connector { $id }
# $end (String) - localized endpoint word
undo-move-endpoint = move { $end } endpoint
# $id (Number) - guide id
undo-move-guide = move guide { $id }
# $id (Number) - shape id
undo-move-line-shape = move line shape { $id }
# $id (Number) - shape id
undo-move-shape = move shape { $id }
# $n (Number) - vertex row index
undo-move-vertex = move vertex { $n }
# $id (Number) - shape id
undo-resize-shape = resize shape { $id }
# $name (String) - cell name
undo-set-cell = set { $name }
# $section (String) - section name
# $cell (String) - cell name
undo-set-section-cell = set { $section }.{ $cell }
# $n (Number) - group count
undo-ungroup-n =
    { $n ->
        [one] ungroup one group
       *[other] ungroup { $n } groups
    }
ungroup = Ungroup
ungroup-the-selected-group-s = Ungroup the selected group(s)
unsaved-changes = Unsaved changes
untitled = Untitled
use-relative-paths-for-hyperlink = Use relative paths for hyperlinks
value = Value:
variant-colors-planned = Variant colors — planned
variant-connectors-planned = Variant connectors — planned
variant-effects-planned = Variant effects — planned
variants = Variants
vba-arrives-with-m7-vsdm = VBA arrives with M7 (.vsdm)
vector-svg = Vector (SVG)
# $cell (String) - the formula-driven cell description
vertex-drag-refused = vertex drag refused: { $cell } is formula-driven — edit the formula to change it
view = View
views = Views
visio-drawing = Visio drawing
visio-stencil = Visio stencil
visio-vsdx-family = Visio (VSDX family)
visual-aids = Visual Aids
visual-basic = Visual Basic
weight = Weight
# $sub (String) - sub-address page name
went-to-page = went to page { $sub }
whole-page = Whole Page
wide-arrow = Wide Arrow
width = Width
# $g (String) - guard formula
width-not-resized-guard = width not resized: { $g } (height only)
window = Window
# $n (Number) - window count
window-n = Window { $n }
word-count = Word Count
word-count-planned = Word count — planned
worker-lost = worker lost
wrap-the-selection-in-this-conta = Wrap the selection in this container style
x-cancel-placement = x cancel placement
zoom = Zoom
zoom-dialog-planned = Zoom dialog — planned
zoom-in = Zoom In
zoom-in-tip = Zoom in
zoom-menu = Zoom…
zoom-out = Zoom Out
zoom-out-tip = Zoom out
zoom-to-selection-shift-z = Zoom to selection (Shift+Z)
