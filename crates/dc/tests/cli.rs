//! CLI contract tests (#97 §6 items 1–3): the `dc` binary's observable
//! behavior — exit codes, stdout/stderr split, files written, and the
//! flag wiring between clap and dc-core — which no other layer can see
//! (the audit found this crate had zero tests of any kind). Fixtures
//! are built in code through dc-core and saved as real packages, so no
//! Microsoft content is involved.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn dc_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_dc").unwrap().into()
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(dc_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("spawn dc")
}

fn ok(out: &Output) -> &str {
    assert!(
        out.status.success(),
        "dc failed: {}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    std::str::from_utf8(&out.stdout).unwrap()
}

fn fails(out: &Output) -> &str {
    assert!(
        !out.status.success(),
        "dc unexpectedly succeeded: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    std::str::from_utf8(&out.stderr).unwrap()
}

/// The blank starter document (one Letter page, starter masters) — the
/// same base every surface opens.
fn blank_doc(dir: &Path, name: &str) -> PathBuf {
    let mut ed = dc_core::Editor::new_blank();
    let path = dir.join(name);
    ed.save_as(&path).unwrap();
    path
}

/// Blank + a second page (distinct name) + one shape with text on
/// page 1 — exercises the multi-page and edit surfaces.
fn shape_doc(dir: &Path, name: &str) -> PathBuf {
    let mut ed = dc_core::Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let shape_id = dc_core::commands::next_shape_id(&ed.doc, page);
    let mut shape = vsdx::model::Shape {
        id: shape_id,
        type_: Some("Shape".into()),
        ..vsdx::model::Shape::default()
    };
    shape.sheet.set_cell("PinX", vsdx::model::Cell::value("2"));
    shape.sheet.set_cell("PinY", vsdx::model::Cell::value("3"));
    ed.apply(dc_core::AddShape {
        page,
        parent: None,
        shape,
    })
    .unwrap();
    ed.apply(dc_core::commands::SetText::new(
        dc_core::Scope::Shape { page, shape: shape_id },
        "hello cli",
    ))
    .unwrap();
    ed.apply(dc_core::commands::AddPage::new("Second")).unwrap();
    let path = dir.join(name);
    ed.save_as(&path).unwrap();
    path
}

fn arg(p: &Path) -> &str {
    p.to_str().unwrap()
}

// ---------------------------------------------------------------------------
// render (item 1): the multi-page contract the help text always
// promised ("page N appends -N for multi-page docs") and the code
// silently violated by rendering only page 1.
// ---------------------------------------------------------------------------

#[test]
fn render_single_page_document_writes_the_plain_out_path() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = blank_doc(tmp.path(), "blank.vsdx");
    let out = run(tmp.path(), &["render", arg(&doc), "-o", "out.svg"]);
    let stdout = ok(&out);
    assert!(stdout.contains("wrote out.svg"), "stdout: {stdout}");
    let svg = std::fs::read_to_string(tmp.path().join("out.svg")).unwrap();
    assert!(svg.starts_with("<svg"), "not an SVG: {}", &svg[..40.min(svg.len())]);
}

#[test]
fn render_multi_page_document_writes_every_page_with_numbered_suffixes() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = shape_doc(tmp.path(), "two.vsdx");
    let out = run(tmp.path(), &["render", arg(&doc), "-o", "out.svg"]);
    let stdout = ok(&out);
    let p1 = std::fs::read_to_string(tmp.path().join("out-1.svg")).unwrap();
    let p2 = std::fs::read_to_string(tmp.path().join("out-2.svg")).unwrap();
    assert!(stdout.contains("out-1.svg") && stdout.contains("out-2.svg"), "stdout: {stdout}");
    assert!(!tmp.path().join("out.svg").exists(), "the un-numbered path must not be written");
    assert!(p1.contains("hello cli"), "page 1 carries the shape text");
    assert!(!p2.contains("hello cli"), "page 2 is the other page");
}

#[test]
fn render_page_flag_selects_one_page_and_rejects_out_of_range() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = shape_doc(tmp.path(), "two.vsdx");
    let out = run(tmp.path(), &["render", arg(&doc), "-o", "only.svg", "--page", "2"]);
    ok(&out);
    assert!(tmp.path().join("only.svg").exists());
    assert!(!tmp.path().join("only-2.svg").exists(), "--page keeps the plain path");
    let out = run(tmp.path(), &["render", arg(&doc), "--page", "9"]);
    let stderr = fails(&out);
    assert!(stderr.contains("out of range"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// roundtrip --verify (item 3): the PASS path runs end-to-end through
// the binary and its output re-opens. (The verify-FAILURE legs compare
// writer output against its own input; without a fault-injection seam
// there is no deterministic way to make a correct writer fail, so the
// failure-reporting contract is covered by the error paths below.)
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_verify_passes_and_the_output_reopens() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = shape_doc(tmp.path(), "two.vsdx");
    let out = run(tmp.path(), &["roundtrip", arg(&doc), "-o", "rt.vsdx", "--verify"]);
    let stdout = ok(&out);
    assert!(stdout.contains("wrote rt.vsdx"), "stdout: {stdout}");
    let reopened = vsdx::parse::parse(&tmp.path().join("rt.vsdx")).unwrap();
    assert_eq!(reopened.pages.len(), 2, "both pages survive");
}

// ---------------------------------------------------------------------------
// edit wiring (item 2): each mutation family round-trips through the
// flags → dc-core → package, and malformed input exits non-zero with a
// stderr message (never a silent drop).
// ---------------------------------------------------------------------------

#[test]
fn edit_flags_roundtrip_every_mutation_family() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = shape_doc(tmp.path(), "in.vsdx");
    let page = "page:1";
    let shape = "page:1/shape:1";

    // cell
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "cell.vsdx", "--cell", shape, "PinX", "4.5",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("cell.vsdx")).unwrap();
    assert_eq!(
        d.pages[0].shapes[0].sheet.cell("PinX").and_then(|c| c.numeric()),
        Some(4.5)
    );

    // formula (the ShapeSheet-edit escape hatch)
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "formula.vsdx",
        "--formula", shape, "Width", "GUARD(2 in)",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("formula.vsdx")).unwrap();
    assert_eq!(
        d.pages[0].shapes[0].sheet.cell("Width").and_then(|c| c.f.clone()),
        Some("GUARD(2 in)".into())
    );

    // text
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "text.vsdx", "--text", shape, "replaced text",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("text.vsdx")).unwrap();
    assert!(format!("{:?}", d.pages[0].shapes[0].sheet.text_raw).contains("replaced text"));

    // pages: add + rename (rename proves the add's ID round-trips)
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "pages.vsdx",
        "--add-page", "Third", "--rename-page", "1", "Front",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("pages.vsdx")).unwrap();
    assert_eq!(d.pages.len(), 3);
    assert_eq!(d.pages[0].name, "Front");

    // layers
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "layers.vsdx", "--add-layer", "1", "Annotations",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("layers.vsdx")).unwrap();
    assert!(dc_core::layers::layer_rows(&d, d.pages[0].id)
        .unwrap()
        .iter()
        .any(|r| dc_core::layers::layer_name(r) == Some("Annotations")));

    // connect (page owner, glue shape 1 Begin → shape 3 End; shape 3 is
    // the placeholder added in the same invocation)
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "conn.vsdx", "--add-shape", "1",
        "--connect", "page:1", "1", "Begin", "3", "3", "End", "3",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("conn.vsdx")).unwrap();
    let conns = dc_core::connects::ConnectsOwner::Page(d.pages[0].id)
        .read_connects(&d)
        .unwrap();
    assert_eq!(conns.list.len(), 1, "the connect round-tripped");

    // undo: two edits, one --undo → only the first survives
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "undone.vsdx",
        "--cell", shape, "PinX", "4.5", "--cell", shape, "PinY", "6.5", "--undo",
    ]);
    ok(&out);
    let d = vsdx::parse::parse(&tmp.path().join("undone.vsdx")).unwrap();
    assert_eq!(
        d.pages[0].shapes[0].sheet.cell("PinX").and_then(|c| c.numeric()),
        Some(4.5),
        "the earlier edit survives the undo"
    );
    assert_eq!(
        d.pages[0].shapes[0].sheet.cell("PinY").and_then(|c| c.numeric()),
        Some(3.0),
        "the undone edit reverts to the authored value"
    );

    // history prints and succeeds without mutating
    let out = run(tmp.path(), &["edit", arg(&doc), "-o", "hist.vsdx", "--history"]);
    let stdout = ok(&out);
    assert!(!stdout.trim().is_empty(), "history lists: {stdout}");
}

#[test]
fn edit_rejects_malformed_input_on_stderr_with_a_nonzero_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = shape_doc(tmp.path(), "in.vsdx");
    // Non-numeric page ID in --rename-page.
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "bad.vsdx", "--rename-page", "page:notanumber", "X",
    ]);
    let stderr = fails(&out);
    assert!(!stderr.is_empty(), "an explanation belongs on stderr");
    // Missing input file.
    let out = run(tmp.path(), &["edit", "missing.vsdx"]);
    let stderr = fails(&out);
    assert!(stderr.contains("missing.vsdx"), "stderr: {stderr}");
    // Output into a nonexistent directory fails rather than pretending.
    let out = run(tmp.path(), &[
        "edit", arg(&doc), "-o", "no/such/dir/out.vsdx",
        "--cell", "page:1/shape:1", "PinX", "1",
    ]);
    let stderr = fails(&out);
    assert!(!stderr.is_empty());
}

// ---------------------------------------------------------------------------
// info/inspect: the read-only subcommands succeed on a real package
// and report missing files on stderr.
// ---------------------------------------------------------------------------

#[test]
fn info_and_inspect_read_packages_and_report_missing_files() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = blank_doc(tmp.path(), "blank.vsdx");
    let out = run(tmp.path(), &["info", arg(&doc)]);
    let stdout = ok(&out);
    assert!(stdout.contains("visio/document.xml"), "part inventory: {stdout}");
    let out = run(tmp.path(), &["inspect", arg(&doc)]);
    let stdout = ok(&out);
    assert!(!stdout.trim().is_empty(), "inspect prints the model");
    let out = run(tmp.path(), &["info", "nope.vsdx"]);
    let stderr = fails(&out);
    assert!(stderr.contains("nope.vsdx"), "stderr: {stderr}");
}
