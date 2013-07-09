// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use encoding::{Encoder, Decoder};
use encoding::{DecodeAsReplacementChar, DecodeAs, SkipInvalidByte,
               TruncateDecoding, FailDecoding};
use encoding::{EncodeAsReplacementChar, EncodeAs, SkipOutOfRangeChar,
               TruncateEncoding, FailEncoding};
use char::ReplacementChar;
use iterator::Iterator;
use option::{Option, None, Some};
use to_bytes::IterBytes;
use uint;
use vec::{CopyableVector, ImmutableVector, MutableVector, MutableCloneableVector};

#[allow(non_camel_case_types)]
pub enum utf16 {
    /// UTF-16, emits a BOM on encoding and consumes one on decoding.
    /// UTF-16BE is used for encoding, and assumed if there is no BOM on decoding.
    utf16,
    /// UTF-16BE
    utf16be,
    /// UTF-16LE
    utf16le,
}

impl<T: Iterator<char>> Encoder<T, UTF16Encoder<T>> for utf16 {
    fn encode(&self, src: T) -> UTF16Encoder<T> {
        match *self {
            utf16 => UTF16Encoder{ iter: src, bom: true, big: true,
                                   buf: [0, ..4], lo: 0, hi: 0 },
            utf16be => UTF16Encoder{ iter: src, bom: false, big: true,
                                     buf: [0, ..4], lo: 0, hi: 0 },
            utf16le => UTF16Encoder{ iter: src, bom: false, big: false,
                                     buf: [0, ..4], lo: 0, hi: 0 }
        }
    }
}

impl<T: Iterator<u8>> Decoder<T, UTF16Decoder<T>> for utf16 {
    fn decode(&self, src: T) -> UTF16Decoder<T> {
        match *self {
            utf16 => UTF16Decoder{ iter: Some(src), bom: true, big: true, c: None },
            utf16be => UTF16Decoder{ iter: Some(src), bom: false, big: true, c: None },
            utf16le => UTF16Decoder{ iter: Some(src), bom: false, big: false, c: None }
        }
    }
}

pub struct UTF16Encoder<T> {
    priv iter: T,
    priv bom: bool,
    priv big: bool,
    priv buf: [u8, ..4],
    priv lo: uint,
    priv hi: uint
}

impl<T: Iterator<char>> Iterator<u8> for UTF16Encoder<T> {
    #[inline]
    fn next(&mut self) -> Option<u8> {
        use encoding::out_of_range::cond;

        if self.bom {
            self.lo = 0;
            for 0xFEFFu16.iter_bytes(!self.big) |b| { self.hi = self.buf.copy_from(b); }
            self.bom = false;
        }
        if self.lo == self.hi {
            loop {
                let c = self.iter.next();
                if c.is_none() { return None }
                let mut c = c.unwrap() as u32;
                fn is_valid(c: u32) -> bool {
                    (c < 0xD800 || c > 0xDBFF) && (c < 0xDC00 || c > 0xDFFF) && c <= 0x10FFFF
                }
                if !is_valid(c) {
                    match cond.raise_default(c as char, || EncodeAsReplacementChar ) {
                        EncodeAsReplacementChar => c = ReplacementChar as u32,
                        EncodeAs(c_) => c = c_ as u32,
                        SkipOutOfRangeChar => loop,
                        TruncateEncoding => return None,
                        FailEncoding => fail!("out-of-range char 0x%x found", c as uint)
                    }
                    if !is_valid(c) {
                        fail!("out-of-range har 0x%x found", c as uint);
                    }
                }
                self.lo = 0;
                if c > 0xFFFF {
                    c -= 0x10000;
                    let lead = (0xD800 + (c >> 10)) as u16;
                    let trail = (0xDC00 + (c & 0x3FF)) as u16;
                    self.hi = 0;
                    for [lead, trail].iter_bytes(!self.big) |b| {
                        self.hi += self.buf.mut_slice(self.hi, 4).copy_from(b);
                    }
                } else {
                    self.hi = 0;
                    for (c as u16).iter_bytes(!self.big) |b| { self.hi += self.buf.copy_from(b); }
                }
                break;
            }
        }
        let r = Some(self.buf[self.lo]);
        self.lo += 1;
        r
    }

    #[inline]
    fn size_hint(&self) -> (uint, Option<uint>) {
        // most common will be 2*length, but surrogate pairs mean it coul be up to 4*length
        let (lo, hi) = self.iter.size_hint();

        let lo = if lo > uint::max_value / 2 {
            uint::max_value
        } else { lo*2 };
        let hi = do hi.chain |x| {
            if x > uint::max_value / 4 { None }
            else { Some(x*4) }
        };
        (lo, hi)
    }
}

pub struct UTF16Decoder<T> {
    priv iter: Option<T>,
    priv bom: bool,
    priv big: bool,
    priv c: Option<char>
}

impl<T: Iterator<u8>> Iterator<char> for UTF16Decoder<T> {
    #[inline]
    fn next(&mut self) -> Option<char> {
        use encoding::invalid_byte::cond;

        if self.c.is_some() {
            return Some(self.c.swap_unwrap());
        }

        if self.iter.is_none() {
            return None;
        }

        let mut lead = None;

        loop {
            let a = self.iter.get_mut_ref().next();
            if a.is_none() { return None }
            let a = a.unwrap();
            let b = self.iter.get_mut_ref().next();
            if b.is_none() {
                // half a codepoint?
                self.iter = None;
                match cond.raise_default(None, || DecodeAsReplacementChar) {
                    DecodeAsReplacementChar => return Some(ReplacementChar),
                    DecodeAs(c) => return Some(c),
                    SkipInvalidByte => return None, // stream is empty
                    TruncateDecoding => return None,
                    FailDecoding => fail!("bytestream terminated unexpectedly")
                }
            }
            let b = b.unwrap();

            if self.bom {
                self.bom = false;
                if a == 0xFE && b == 0xFF {
                    self.big = true;
                    loop;
                } else if a == 0xFF && b == 0xFE {
                    self.big = false;
                    loop;
                }
            }

            let c = if self.big {
                (a as u16 << 8) | (b as u16)
            } else {
                (b as u16 << 8) | (a as u16)
            };

            let mut valid = true;
            let mut arg = [a, b];
            if c >= 0xD800 && c <= 0xDBFF {
                if lead.is_none() {
                    lead = Some((c, a, b));
                    loop;
                }
                valid = false;
            } else if c >= 0xDC00 && c <= 0xDFFF {
                if lead.is_some() {
                    let (lead, _, _) = lead.unwrap();
                    let lead = (lead as u32 - 0xD800) << 10;
                    let trail = c as u32 - 0xDC00;
                    return Some(((lead | trail) + 0x10000) as char);
                }
                valid = false;
            } else if lead.is_some() {
                // the invalid sequence is actually the one that generated lead
                valid = false;
                let (_, a_, b_) = lead.unwrap();
                arg[0] = a_;
                arg[1] = b_;
                self.c = Some(c as char);
            }
            if !valid {
                match cond.raise_default(Some(arg.to_owned()), || DecodeAsReplacementChar) {
                    DecodeAsReplacementChar => return Some(ReplacementChar),
                    DecodeAs(c) => return Some(c),
                    SkipInvalidByte => loop,
                    TruncateDecoding => return None,
                    FailDecoding => fail!("invalid byte sequence encountered")
                }
            }

            return Some(c as char);
        }
    }

    #[inline]
    fn size_hint(&self) -> (uint, Option<uint>) {
        // Could be as many as length/2, or as few as length/4 due to surrogate pairs
        let (lo, hi) = self.iter.map_default((0, None), |it| it.size_hint());

        let lo = if lo == uint::max_value { uint::max_value }
                 else { lo / 4 };
        // round hi up; a trailing byte could turn into a char based on condition handling
        let hi = do hi.map_consume |x| { x / 2 + x % 2 };
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iterator::IteratorUtil;

    #[test]
    fn test_utf16be_encode() {
        let a = ['t', 'e', 's', 't'];
        assert_eq!(a.encode_as(utf16be).collect::<~[u8]>(),
                   ~[0, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8]);

        let b = ['测', '试'];
        assert_eq!(b.encode_as(utf16be).collect::<~[u8]>(),
                   ~[0x6D, 0x4B, 0x8B, 0xD5]);

        let c = ['𡸳','拔'];
        assert_eq!(c.encode_as(utf16be).collect::<~[u8]>(),
                   ~[0xD8, 0x47, 0xDE, 0x33, 0xD8, 0x7E, 0xDC, 0xB6]);
    }

    #[test]
    fn test_utf16le_encode() {
        let a = ['t', 'e', 's', 't'];
        assert_eq!(a.encode_as(utf16le).collect::<~[u8]>(),
                   ~['t' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8, 0]);
    }

    #[test]
    fn test_utf16_encode_bom() {
        let a = ['t', 'e', 's', 't'];
        assert_eq!(a.encode_as(utf16).collect::<~[u8]>(),
                   ~[0xFE, 0xFF, 0, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8]);
    }

    #[test]
    fn test_utf16be_decode() {
        let a = [0, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8];
        assert_eq!(a.decode_as(utf16be).collect::<~[char]>(),
                   ~['t', 'e', 's', 't']);

        let b = [0x6Du8, 0x4Bu8, 0x8Bu8, 0xD5u8];
        assert_eq!(b.decode_as(utf16be).collect::<~[char]>(),
                   ~['测', '试']);

        let c = [0xD8u8, 0x47u8, 0xDEu8, 0x33u8, 0xD8u8, 0x7Eu8, 0xDCu8, 0xB6u8];
        assert_eq!(c.decode_as(utf16be).collect::<~[char]>(),
                   ~['𡸳','拔']);
    }

    #[test]
    fn test_utf16le_decode() {
        let a = ['t' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8, 0];
        assert_eq!(a.decode_as(utf16le).collect::<~[char]>(),
                   ~['t', 'e', 's', 't']);

        let b = [0x47u8, 0xD8u8, 0x33u8, 0xDEu8, 0x7Eu8, 0xD8u8, 0xB6u8, 0xDCu8];
        assert_eq!(b.decode_as(utf16le).collect::<~[char]>(),
                   ~['𡸳','拔']);
    }

    #[test]
    fn test_utf16_decode_bom() {
        let a = [0xFE, 0xFF, 0, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8];
        assert_eq!(a.decode_as(utf16).collect::<~[char]>(),
                   ~['t', 'e', 's', 't']);

        let b = [0, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8];
        assert_eq!(b.decode_as(utf16).collect::<~[char]>(),
                   ~['t', 'e', 's', 't']);

        let c = [0xFF, 0xFE, 't' as u8, 0, 'e' as u8, 0, 's' as u8, 0, 't' as u8, 0];
        assert_eq!(c.decode_as(utf16).collect::<~[char]>(),
                   ~['t', 'e', 's', 't']);
    }

    #[test]
    fn test_utf16_decode_ZWNBS() {
        let a = [0, 't' as u8, 0, 'e' as u8, 0xFE, 0xFF, 0, 's' as u8, 0, 't' as u8];
        assert_eq!(a.decode_as(utf16).collect::<~[char]>(),
                   ~['t', 'e', 0xFEFF as char, 's', 't']);
    }

    #[test]
    fn test_utf16_reencode() {
        let a = [0x47u8, 0xD8u8, 0x33u8, 0xDEu8, 0x7Eu8, 0xD8u8, 0xB6u8, 0xDCu8];
        assert_eq!(a.reencode(utf16le,utf16be).collect::<~[u8]>(),
                   ~[0xD8u8, 0x47u8, 0xDEu8, 0x33u8, 0xD8u8, 0x7Eu8, 0xDCu8, 0xB6u8]);
    }
}
