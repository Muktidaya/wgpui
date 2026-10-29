use image::{AnimationDecoder, GenericImageView};
use std::{io::Cursor, sync::Arc};
use wgpui::{DevicePixels, Size, SvgRenderer};

#[test]
fn raster_examples_preserve_alpha_colors_and_animation() -> anyhow::Result<()> {
    let png = image::load_from_memory_with_format(
        include_bytes!("../examples/legacy/image/geometric-demo.png"),
        image::ImageFormat::Png,
    )?;
    assert_eq!(png.dimensions(), (512, 512));
    let rgba = png.to_rgba8();
    assert_eq!(rgba.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(rgba.get_pixel(100, 100).0, [24, 35, 55, 220]);
    assert_eq!(rgba.get_pixel(200, 200).0, [40, 110, 200, 255]);

    let gif = include_bytes!("../examples/legacy/image/geometric-animation.gif");
    let frames = image::codecs::gif::GifDecoder::new(Cursor::new(gif))?
        .into_frames()
        .collect_frames()?;
    assert_eq!(frames.len(), 25);
    for frame in &frames {
        assert_eq!(frame.buffer().dimensions(), (500, 500));
        assert_eq!(frame.delay().numer_denom_ms(), (40, 1));
        assert_eq!(frame.buffer().get_pixel(0, 0).0[3], 0);
    }
    assert_ne!(frames[0].buffer(), frames[24].buffer());
    Ok(())
}

#[test]
fn svg_examples_preserve_mask_color_and_dragon_rendering() -> anyhow::Result<()> {
    let renderer = SvgRenderer::new(Arc::new(()));
    let arrow = renderer.render_single_frame(
        include_bytes!("../examples/legacy/image/circle-arrow-demo.svg"),
        1.0,
        false,
    )?;
    assert_eq!(arrow.size(0), Size::new(DevicePixels(32), DevicePixels(32)));
    let pixels = arrow.as_bytes(0).expect("arrow frame");
    assert!(pixels.chunks_exact(4).any(|p| p[3] > 0));
    assert!(pixels.chunks_exact(4).any(|p| p[3] == 0));

    let colors = renderer.render_single_frame(
        include_bytes!("../examples/legacy/image/colored-shapes-demo.svg"),
        1.0,
        false,
    )?;
    assert_eq!(
        colors.size(0),
        Size::new(DevicePixels(960), DevicePixels(960))
    );
    let pixels = colors.as_bytes(0).expect("color frame");
    assert!(pixels.chunks_exact(4).any(|p| p == [40, 110, 200, 255]));
    assert!(pixels.chunks_exact(4).any(|p| p == [220, 150, 40, 255]));
    assert_eq!(&pixels[..4], &[0, 0, 0, 0]);

    let dragon = renderer.render_single_frame(
        include_bytes!("../examples/legacy/svg/dragon.svg"),
        1.0,
        false,
    )?;
    assert!(
        dragon
            .as_bytes(0)
            .expect("dragon frame")
            .chunks_exact(4)
            .any(|p| p[3] > 0)
    );
    Ok(())
}
