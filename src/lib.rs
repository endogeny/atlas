#![warn(missing_docs)]

//! Putting textures together, hopefully without wasting too much space.

extern crate framing;

use framing::{AsBytes, Image, Function, ChunkyFrame};
use std::mem;

// TODO(quadrupleslap): Current algorithm sucks, write a better one.

/// Stores images, and automatically stitches them together.
///
/// While it's definitely okay to add the images in any order, to get any decent
/// space efficiency it's necessary to at least sort-of sort the frames in
/// terms of decreasing size. Particularly good orders are by `width * height`
/// and by `max(width, height)`, both in descending order.
pub struct Atlas<T> {
    image: Option<Box<Image<Pixel = T> + Sync>>,
    blank: T,
    rects: Vec<Rect>
}

impl<T> Atlas<T> {
    /// Create a new, empty atlas.
    ///
    /// The blank pixel will be used to represent the space that exists between
    /// images, in the almost certain case that 100% space utilization is not
    /// achieved.
    pub fn new(blank: T) -> Self {
        Atlas {
            image: None,
            blank,
            rects: Vec::new()
        }
    }

    /// Adds an image to the atlas, placing it appropriately.
    ///
    /// The return value is the location of the image within the atlas. If the
    /// image is of zero size, then the coordinates `(0, 0)` will be returned,
    /// which you most likely won't need to special-case, since it is
    /// *technically* valid.
    pub fn add<U>(&mut self, image: U) -> (usize, usize)
    where
        T: Clone + Sync + 'static,
        U: Into<Box<Image<Pixel = T> + Sync>>
    {
        let image = image.into();
        let mut current = None;
        mem::swap(&mut self.image, &mut current);

        let (w, h) = (image.width(), image.height());

        if w == 0 || h == 0 {
            return (0, 0);
        }

        if let Some(current) = current {
            let (cur_width, cur_height) = (current.width(), current.height());

            let result = self.rects.iter()
                .enumerate()
                .filter(|&(_, rect)| w <= rect.w && h <= rect.h)
                .min_by_key(|&(_, rect)| {
                    let (dw, dh) = (rect.w - w, rect.h - h);
                    if dh < dw { dh } else { dw }
                })
                .map(|(i, rect)| (i, rect.clone()));

            if let Some((i, rect)) = result {
                self.rects.remove(i);

                if rect.w != w {
                    self.rects.push(Rect {
                        x: rect.x + w,
                        y: rect.y,
                        w: rect.w - w,
                        h: h
                    });
                }

                if rect.h != h {
                    self.rects.push(Rect {
                        x: rect.x,
                        y: rect.y + h,
                        w: w,
                        h: rect.h - h
                    });
                }

                if rect.w != w && rect.h != h {
                    self.rects.push(Rect {
                        x: rect.x + w,
                        y: rect.y + h,
                        w: rect.w - w,
                        h: rect.h - h
                    });
                }

                self.image = Some(Box::new(Function::new(
                    cur_width, cur_height,
                    {
                        let rect = rect.clone();
                        move |x, y| unsafe {
                            if rect.x <= x && rect.y <= y {
                                let (x, y) = (x - rect.x, y - rect.y);
                                if x < w && y < h {
                                    return image.pixel(x, y);
                                }
                            }

                            current.pixel(x, y)
                        }
                    }
                )));

                (rect.x, rect.y)
            } else {
                if cur_height <= cur_width {
                    if cur_width > w {
                        self.rects.push(Rect {
                            x: w,
                            y: cur_height,
                            w: cur_width - w,
                            h: h
                        });
                    } else if cur_width < w {
                        self.rects.push(Rect {
                            x: cur_width,
                            y: 0,
                            w: w - cur_width,
                            h: cur_height
                        });
                    }

                    self.image = Some(Box::new(Function::new(
                        cur_width, cur_height + h,
                        {
                            let blank = self.blank.clone();
                            move |x, y| unsafe {
                                if y < cur_height {
                                    current.pixel(x, y)
                                } else if x < w {
                                    image.pixel(x, y - cur_height)
                                } else {
                                    blank.clone()
                                }
                            }
                        }
                    )));

                    (0, cur_height)
                } else {
                    if cur_height > h {
                        self.rects.push(Rect {
                            x: cur_width,
                            y: h,
                            w: w,
                            h: cur_height - h
                        });
                    } else if cur_height < h {
                        self.rects.push(Rect {
                            x: 0,
                            y: cur_height,
                            w: cur_width,
                            h: h - cur_height
                        });
                    }

                    self.image = Some(Box::new(Function::new(
                        cur_width + w, cur_height,
                        {
                            let blank = self.blank.clone();
                            move |x, y| unsafe {
                                if x < cur_width {
                                    current.pixel(x, y)
                                } else if y < h {
                                    image.pixel(x - cur_width, y)
                                } else {
                                    blank.clone()
                                }
                            }
                        }
                    )));

                    (cur_width, 0)
                }
            }
        } else {
            self.image = Some(image.into());
            (0, 0)
        }
    }

    /// Internally converts the backing image into a byte-buffer.
    ///
    /// This should drastically improve the speed of accessing an image, but
    /// isn't exactly *cheap*, so do it after adding all your images.
    pub fn collapse(&mut self) where T: AsBytes + Sync + 'static {
        let mut image = None;
        mem::swap(&mut self.image, &mut image);

        if let Some(image) = image {
            self.image = Some(Box::new(ChunkyFrame::new(image)));
        }
    }
}

impl<T> Into<ChunkyFrame<T>> for Atlas<T> where T: AsBytes {
    fn into(self) -> ChunkyFrame<T> {
        if let Some(ref image) = self.image {
            ChunkyFrame::new(image)
        } else {
            ChunkyFrame::from_bytes(0, 0, Vec::new().into())
        }
    }
}

impl<T> Image for Atlas<T> {
    type Pixel = T;

    fn width(&self) -> usize {
        if let Some(ref image) = self.image {
            image.width()
        } else {
            0
        }
    }

    fn height(&self) -> usize {
        if let Some(ref image) = self.image {
            image.height()
        } else {
            0
        }
    }

    unsafe fn pixel(&self, x: usize, y: usize) -> Self::Pixel {
        self.image.as_ref().unwrap().pixel(x, y)
    }
}

#[derive(Clone)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize
}
