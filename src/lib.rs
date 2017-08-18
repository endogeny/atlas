#![warn(missing_docs)]

//! Putting textures together, hopefully without wasting too much space.

extern crate framing;

use framing::{AsBytes, Image, Chunky};
use std::{mem, ptr};

/// Stores images, and automatically stitches them together.
///
/// While it's definitely okay to add the images in any order, to get any decent
/// space efficiency it's necessary to at least sort-of sort the frames in
/// terms of decreasing size. Particularly good orders are by `width * height`
/// and by `max(width, height)`, both in descending order.
pub struct Atlas<T> {
    bytes: Vec<u8>,
    scratch: Vec<u8>,
    width: usize,
    height: usize,
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
            bytes: Vec::new(),
            scratch: Vec::new(),
            width: 0,
            height: 0,
            blank: blank,
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
        T: AsBytes + Clone + Sync + 'static,
        U: Image<Pixel = T> + Sync
    {
        let (w, h) = (image.width(), image.height());

        if w == 0 || h == 0 {
            return (0, 0);
        }

        if self.width == 0 || self.height == 0 {
            self.bytes.reserve(T::width() * w * h);
            self.width = w;
            self.height = h;

            for (_, _, pixel) in framing::iter(&image) {
                self.bytes.extend_from_slice(T::Bytes::from(pixel).as_ref())
            }

            return (0, 0);
        }

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

            // The image fits!
            for y in 0..h {
            for x in 0..w {
                let i = T::width() * (self.width * (rect.y + y) + (rect.x + x));
                let p = T::Bytes::from(unsafe {
                    image.pixel(x, y)
                });

                unsafe {
                    ptr::copy_nonoverlapping(
                        p.as_ref().as_ptr(),
                        self.bytes.as_mut_ptr().offset(i as isize),
                        T::width()
                    )
                }
            }}

            (rect.x, rect.y)
        } else {
            // The image doesn't fit.

            if self.height <= self.width {
                // Our atlas is wider than it is tall, so the image is put at
                // the bottom of the atlas, to make it more square.

                if self.width > w {
                    self.rects.push(Rect {
                        x: w,
                        y: self.height,
                        w: self.width - w,
                        h: h
                    });
                } else if self.width < w {
                    self.rects.push(Rect {
                        x: self.width,
                        y: 0,
                        w: w - self.width,
                        h: self.height
                    });
                }

                if w <= self.width {
                    // The image is already wide enough.

                    self.bytes.reserve(T::width() * self.width * h);

                    for y in self.height..(self.height + h) {
                        for x in 0..w {
                            let pixel = T::Bytes::from(unsafe {
                                image.pixel(x, y)
                            });
                            self.bytes.extend_from_slice(pixel.as_ref());
                        }
                        for _ in w..self.width {
                            let pixel = T::Bytes::from(self.blank.clone());
                            self.bytes.extend_from_slice(pixel.as_ref());
                        }
                    }

                    self.height = self.height + h;
                } else {
                    // We need to make the image wider.

                    let cap = T::width() * (self.height + h) * w;
                    self.scratch.clear();
                    self.scratch.reserve(cap);

                    for chunk in self.bytes.chunks(T::width() * self.width) {
                        self.scratch.extend_from_slice(chunk);
                        for _ in self.width..w {
                            let pixel = T::Bytes::from(self.blank.clone());
                            self.scratch.extend_from_slice(pixel.as_ref());
                        }
                    }

                    for y in 0..h {
                        for x in 0..w {
                            let pixel = T::Bytes::from(unsafe {
                                image.pixel(x, y)
                            });
                            self.scratch.extend_from_slice(pixel.as_ref());
                        }
                    }

                    mem::swap(&mut self.bytes, &mut self.scratch);
                    self.width = w;
                    self.height = self.height + h;
                }

                (0, self.height)
            } else {
                // Our atlas is taller than it is wide, so the image is put to
                // the right of the atlas, to make it more square.

                if self.height > h {
                    self.rects.push(Rect {
                        x: self.width,
                        y: h,
                        w: w,
                        h: self.height - h
                    });
                } else if self.height < h {
                    self.rects.push(Rect {
                        x: 0,
                        y: self.height,
                        w: self.width,
                        h: h - self.height
                    });
                }

                let new_height = if self.height <= h { h } else { self.height };
                let new_width = self.width + w;

                let cap = T::width() * new_width * new_height;
                self.scratch.clear();
                self.scratch.reserve(cap);

                for (y, chunk) in
                    self.bytes
                        .chunks(T::width() * self.width)
                        .enumerate()
                {
                    self.scratch.extend_from_slice(chunk);
                    if y < h {
                        for x in 0..w {
                            let pixel = T::Bytes::from(unsafe {
                                image.pixel(x, y)
                            });
                            self.scratch.extend_from_slice(pixel.as_ref());
                        }
                    } else {
                        for _ in 0..w {
                            let pixel = T::Bytes::from(self.blank.clone());
                            self.scratch.extend_from_slice(pixel.as_ref());
                        }
                    }
                }

                for y in self.height..h {
                    for _ in 0..self.width {
                        let pixel = T::Bytes::from(self.blank.clone());
                        self.scratch.extend_from_slice(pixel.as_ref());
                    }
                    for x in 0..w {
                        let pixel = T::Bytes::from(unsafe {
                            image.pixel(x, y)
                        });
                        self.scratch.extend_from_slice(pixel.as_ref());
                    }
                }

                mem::swap(&mut self.bytes, &mut self.scratch);
                self.width = new_width;
                self.height = new_height;

                (self.width, 0)
            }
        }
    }
}

impl<T> Into<Chunky<T>> for Atlas<T> where T: AsBytes {
    fn into(self) -> Chunky<T> {
        Chunky::from_bytes(self.width, self.height, self.bytes)
    }
}

impl<T> Image for Atlas<T> where T: AsBytes {
    type Pixel = T;

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    unsafe fn pixel(&self, x: usize, y: usize) -> Self::Pixel {
        let off = T::width() * (y * self.width + x);
        let mut bytes = T::Bytes::default();

        ptr::copy_nonoverlapping(
            self.bytes.as_ptr().offset(off as isize),
            bytes.as_mut().as_mut_ptr(),
            T::width()
        );

        bytes.into()
    }
}

#[derive(Clone)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize
}
