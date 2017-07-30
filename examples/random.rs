extern crate atlas;
extern crate framing;
extern crate png_framing;
extern crate rand;

use atlas::Atlas;
use framing::{Rgba, Function, Image};
use png_framing::Png;
use rand::distributions::{Range, IndependentSample};

/// Sorts the data before passing it in - this is almost essential.
const SORT: bool = true;
/// The number of images to add to the atlas.
const FRAMES: usize = 5000;
/// Prints rectangle information that can be pasted into
/// [this demo](http://codeincomplete.com/posts/bin-packing/demo/).
const PRINT_RECTS: bool = false;

fn main() {
    let mut rng = rand::thread_rng();
    let between = Range::new(1, 17);

    let mut frames = Vec::with_capacity(FRAMES);
    let mut used = 0;

    for _ in 0..FRAMES {
        let (r, g, b) = rand::random::<(u8, u8, u8)>();
        let color = Rgba(r, g, b, 255);
        let (w, h) = (
            between.ind_sample(&mut rng) * 16,
            between.ind_sample(&mut rng) * 16
        );

        used += w * h;
        if PRINT_RECTS {
            println!("{}x{}", w, h);
        }

        frames.push(Function::new(w, h, move |_, _| color));
    }

    if SORT {
        println!("Sorting...");

        frames.sort_by_key(|frame| -({
            // Sort by longest-side in descending order.
            let (width, height) = (frame.width(), frame.height());
            if width < height { width } else { height }
        } as isize));

        // frames.sort_by_key(|frame| -(
        //     // Sort by area in descending order.
        //     (frame.width() * frame.height()) as isize
        // ));
    }

    println!("Adding frames to atlas...");

    let mut atlas = Atlas::new(Rgba(0, 0, 0, 0));
    for frame in frames {
        atlas.add(frame);
    }

    let total = atlas.width() * atlas.height();
    let efficiency = used as f64 / total as f64;

    println!("Generated with {}% efficiency!", efficiency * 100.0);
    Png::new(atlas).save("output.png").unwrap();
    println!("Atlas saved to `output.png`!");
}
