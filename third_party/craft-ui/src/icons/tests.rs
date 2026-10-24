use super::*;
use crate::testing::TestResult;

const ICONS: &[(&str, &[u8])] = &[
    ("box", br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="4" width="16" height="16"/></svg>"#),
    ("special", br##"<svg stroke-width="1" fill="#123456" stroke-width='2'/>"##),
];

fn set(color: &'static str, width: Option<&'static str>, prefix: &'static str) -> SvgIconSet {
    SvgIconSet::new(
        ICONS,
        SvgStyle {
            current_color: color,
            stroke_width: width,
        },
        prefix,
        "box",
    )
}

/// The (uri, shared bytes) an icon `ImageSource` decomposes into.
type IconParts = (String, Arc<[u8]>);

fn parts(source: ImageSource<'static>) -> Result<IconParts, Box<dyn std::error::Error>> {
    match source {
        ImageSource::Bytes {
            uri,
            bytes: Bytes::Shared(bytes),
        } => Ok((uri.to_string(), bytes)),
        _ => Err("expected shared SVG bytes".into()),
    }
}

#[test]
fn app_styles_preserve_their_own_strokes_and_white_spelling() -> TestResult {
    let photo = set("#ffffff", Some("1.75"), "bytes://icons/");
    let vector = set("white", Some("1.6"), "bytes://icon/");
    for (set, white, width) in [(&photo, "#ffffff", "1.75"), (&vector, "white", "1.6")] {
        let (_, bytes) = parts(set.source("box"))?;
        let text = std::str::from_utf8(&bytes)?;
        assert!(text.contains(&format!("stroke=\"{white}\"")));
        assert!(text.contains(&format!("stroke-width=\"{width}\"")));
        assert!(!text.contains("currentColor"));
        let (_, special) = parts(set.source("special"))?;
        assert_eq!(&*special, ICONS[1].1);
    }
    let (photo_uri, _) = parts(photo.source("box"))?;
    assert_eq!(photo_uri, "bytes://icons/box.svg");
    let (vector_uri, _) = parts(vector.source("box"))?;
    assert_eq!(vector_uri, "bytes://icon/box.svg");
    Ok(())
}

#[test]
fn stroke_replacement_is_optional() -> TestResult {
    let (_, bytes) = parts(set("white", None, "bytes://plain/").source("box"))?;
    assert!(std::str::from_utf8(&bytes)?.contains("stroke-width=\"2\""));
    Ok(())
}

#[test]
fn fallback_shares_bytes_but_preserves_requested_cache_key() -> TestResult {
    let icons = set("white", None, "bytes://test/");
    let (uri, missing) = parts(icons.source("missing"))?;
    let (_, known) = parts(icons.source("box"))?;
    assert_eq!(uri, "bytes://test/missing.svg");
    assert!(Arc::ptr_eq(&known, &missing));
    let (_, again) = parts(icons.source("box"))?;
    assert!(Arc::ptr_eq(&known, &again));
    assert!(!icons.contains("missing"));
    assert_eq!(icons.canonical_name("missing"), None);
    assert_eq!(icons.canonical_name("box"), Some("box"));
    Ok(())
}

#[test]
fn absent_fallback_is_an_empty_source_not_a_panic() -> TestResult {
    let icons = SvgIconSet::new(
        &[],
        SvgStyle {
            current_color: "white",
            stroke_width: None,
        },
        "bytes://empty/",
        "absent",
    );
    let (uri, bytes) = parts(icons.source("unknown"))?;
    assert_eq!(uri, "bytes://empty/unknown.svg");
    assert!(bytes.is_empty());
    Ok(())
}

#[test]
fn duplicate_names_keep_the_last_entry_and_invalid_utf8_is_lossy() -> TestResult {
    let icons = SvgIconSet::new(
        &[("a", b"first"), ("a", b"last\xff")],
        SvgStyle {
            current_color: "white",
            stroke_width: None,
        },
        "bytes://duplicates/",
        "a",
    );
    let (_, bytes) = parts(icons.source("a"))?;
    assert_eq!(std::str::from_utf8(&bytes)?, "last\u{fffd}");
    Ok(())
}

#[test]
fn egui_svg_loader_rasterizes_both_styles_at_multiple_pixel_sizes() -> TestResult {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    for (prefix, width) in [("bytes://photo/", "1.75"), ("bytes://vector/", "1.6")] {
        let icons = set("white", Some(width), prefix);
        for size in [16, 24, 48] {
            let (uri, bytes) = parts(icons.source("box"))?;
            ctx.include_bytes(uri.clone(), Bytes::Shared(bytes));
            let result = ctx.try_load_image(
                &uri,
                egui::load::SizeHint::Size {
                    width: size,
                    height: size,
                    maintain_aspect_ratio: true,
                },
            )?;
            let egui::load::ImagePoll::Ready { image } = result else {
                return Err("embedded SVG did not load synchronously".into());
            };
            assert_eq!(image.size, [size as usize, size as usize]);
            assert!(image.pixels.iter().any(|pixel| pixel.a() != 0));
        }
    }
    Ok(())
}
