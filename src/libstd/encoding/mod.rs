// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
 * Encoding support
 *
 * Encodings are implemented as a pair of Iterators, one that translates from u8
 * to char, and one that translate from char to u8.
 *
 * Encoding errors are handled by the invalid_byte and out_of_range conditions.
 */

use iterator::Iterator;

pub use encoding::utf16::{utf16, utf16le, utf16be, UTF16Encoder, UTF16Decoder};

mod utf16;

/// Resolution options for the invalid_byte condition
pub enum InvalidByteResolution {
    /// Emit the ReplacementChar
    DecodeAsReplacementChar,
    /// Emit the given char
    DecodeAs(char),
    /// Ignores the invalid byte and continues decoding
    SkipInvalidByte,
    /// Truncate the decoding at the current point
    TruncateDecoding,
    /// Fail the task
    FailDecoding
}

// XXX: Can't put doc comments on macros
// The invalid_byte condition is raised during decoding when a byte is
// encountered that isn't part of a valid encoding in the given encoding.
// None means the bytestream terminated in the middle of a codepoint.
//
// The default handler is to decode as the ReplacementChar.
condition! {
    // This should be &[u8] but I can't seem to solve the lifetime issues
    invalid_byte: (::option::Option<~[u8]>) -> super::InvalidByteResolution;
}

/// Resolution options for the out_of_range condition
pub enum OutOfRangeResolution {
    /// Encode using the default behavior for this encoding (replacement char if possible)
    EncodeAsReplacementChar,
    /// Encode the given char instead
    EncodeAs(char),
    /// Ignores the out-of-range char and continues encoding
    SkipOutOfRangeChar,
    /// Truncate the encoding at the current point
    TruncateEncoding,
    /// Fail the task
    FailEncoding
}

// XXX: Can't put doc comments on macros
// The out_of_range condition is raised during encoding when a char is
// encountered that cannot be represented in the given encoding.
//
// The default handler is to encode as the ReplacementChar if the encoding
// supports it, otherwise as '?'.
condition! {
    out_of_range: (char) -> super::OutOfRangeResolution;
}

/// The Encoder trait allows for encoding chars into bytes
pub trait Encoder<T: Iterator<char>, U: Iterator<u8>> {
    fn encode(&self, src: T) -> U;
}

/// The Decoder trait allows for decoding bytes into chars
pub trait Decoder<T: Iterator<u8>, U: Iterator<char>> {
    fn decode(&self, src: T) -> U;
}
