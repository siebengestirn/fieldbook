/// Generated from SBE schema package `challenge_orderbook_messages` id 1 version 2.
#[allow(
    clippy::absurd_extreme_comparisons,
    clippy::double_must_use,
    clippy::erasing_op,
    clippy::identity_op,
    clippy::unnecessary_cast
)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(clippy::eq_op)]
#[allow(clippy::manual_range_contains)]
pub mod sbe_rt {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DecodeError {
        /// Buffer shorter than needed for `field` (`needed` vs `available` bytes).
        BufferTooShort { field: &'static str, needed: usize, available: usize },
        /// Wire `schemaId` does not match this codec (`expected` name/id vs `actual`).
        WrongSchema { expected: u16, actual: u16, expected_name: &'static str },
        /// Wire `templateId` does not match this message (`expected` name/id vs `actual`).
        WrongTemplate { expected: u16, actual: u16, expected_name: &'static str },
        /// Multi-template stream saw an id with no registered length/decoder.
        UnknownTemplateLength { template_id: u16 },
        /// Header field value exceeds the supported maximum for this platform.
        InvalidHeaderValue { field: &'static str, value: u64, maximum: u64 },
        /// Length-prefix for var-data exceeds schema max or platform size.
        InvalidVarDataLength { field: &'static str, length: u64, max_length: u64 },
        /// Field/group/data was added in a schema version later than the wire message.
        FieldNotInVersion { field: &'static str, wire_version: u16, since_version: u16 },
        /// Text var-data is not valid UTF-8.
        InvalidUtf8 { field: &'static str, error: core::str::Utf8Error },
        /// Text var-data is not valid ASCII.
        InvalidAscii { field: &'static str },
        /// Boolean wire enum was `NullVal` or an unknown discriminant.
        InvalidBoolean { field: &'static str, discriminant: u64 },
        /// Domain `try_*` conversion failed.
        DomainConversionFailed { field: &'static str, reason: &'static str },
    }
    impl core::fmt::Display for DecodeError {
        #[cold]
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::BufferTooShort { field, needed, available } => {
                    write!(
                        f, "field '{}': needed {} bytes, {} available", field, needed,
                        available
                    )
                }
                Self::WrongSchema { expected, actual, expected_name } => {
                    write!(
                        f, "wrong schema: expected id {} ({}), got id {}", expected,
                        expected_name, actual
                    )
                }
                Self::WrongTemplate { expected, actual, expected_name } => {
                    write!(
                        f, "wrong template: expected id {} ({}), got id {}", expected,
                        expected_name, actual
                    )
                }
                Self::UnknownTemplateLength { template_id } => {
                    write!(
                        f,
                        "unknown template id {}: SBE messages do not carry length. Use decode_frame() with an external frame length.",
                        template_id
                    )
                }
                Self::InvalidHeaderValue { field, value, maximum } => {
                    write!(
                        f,
                        "message header field '{}': value {} exceeds supported maximum {}",
                        field, value, maximum
                    )
                }
                Self::InvalidVarDataLength { field, length, max_length } => {
                    write!(
                        f, "var data field '{}': length {} exceeds max {}", field,
                        length, max_length
                    )
                }
                Self::FieldNotInVersion { field, wire_version, since_version } => {
                    write!(
                        f, "field '{}' not in wire version {} (added in version {})",
                        field, wire_version, since_version
                    )
                }
                Self::InvalidUtf8 { field, error } => {
                    write!(f, "field '{}': invalid UTF-8: {}", field, error)
                }
                Self::InvalidAscii { field } => {
                    write!(f, "field '{}': invalid ASCII", field)
                }
                Self::InvalidBoolean { field, discriminant } => {
                    write!(
                        f,
                        "field '{}': invalid boolean (discriminant {discriminant:#x})",
                        field
                    )
                }
                Self::DomainConversionFailed { field, reason } => {
                    write!(f, "field '{}': domain conversion failed: {}", field, reason)
                }
            }
        }
    }
    impl core::error::Error for DecodeError {}
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EncodeError {
        /// Encode buffer shorter than needed for `field` (`needed` vs `available`).
        BufferTooShort { field: &'static str, needed: usize, available: usize },
        /// Claim buffer length does not match ENCODED_LENGTH.
        ClaimLengthMismatch { expected: usize, actual: usize },
        /// Var-data payload longer than the schema max for `field`.
        VarDataTooLong { field: &'static str, max_length: usize, actual: usize },
        /// Fixed char/byte array source longer than the schema length.
        FixedArrayTooLong { field: &'static str, max_length: usize, actual: usize },
        /// Domain/DTO value outside the schema min/max range.
        ValueOutOfRange { field: &'static str, min: i128, max: i128, actual: i128 },
        /// Tried to write more group entries than the declared count.
        GroupFull { declared: u32, attempted: u32 },
        /// Known-size group closure returned without adding enough entries.
        GroupCountMismatch { declared: u32, actual: u32 },
        /// Unknown-size group entry count does not fit in `numInGroup`.
        GroupCountOverflow { maximum: u32, actual: u32 },
        /// Checked arithmetic overflow in encoded length computation.
        EncodedLengthOverflow,
        /// Domain `try_*` conversion failed.
        DomainConversionFailed { field: &'static str, reason: &'static str },
        /// Nested decode failure during encode/verify paths.
        Decode(DecodeError),
    }
    impl core::fmt::Display for EncodeError {
        #[cold]
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::BufferTooShort { field, needed, available } => {
                    write!(
                        f,
                        "buffer too short for {field}: needed {needed}, available {available}"
                    )
                }
                Self::ClaimLengthMismatch { expected, actual } => {
                    write!(
                        f, "claim buffer length mismatch: expected {}, got {}", expected,
                        actual
                    )
                }
                Self::VarDataTooLong { field, max_length, actual } => {
                    write!(
                        f, "var data too long for field {}: max {}, actual {}", field,
                        max_length, actual
                    )
                }
                Self::FixedArrayTooLong { field, max_length, actual } => {
                    write!(
                        f, "fixed array too long for field {}: max {}, actual {}", field,
                        max_length, actual
                    )
                }
                Self::ValueOutOfRange { field, min, max, actual } => {
                    write!(
                        f, "value out of range for field {}: min {}, max {}, actual {}",
                        field, min, max, actual
                    )
                }
                Self::GroupFull { declared, attempted } => {
                    write!(
                        f, "group full: declared count {}, attempted to write {}",
                        declared, attempted
                    )
                }
                Self::GroupCountMismatch { declared, actual } => {
                    write!(
                        f, "group count mismatch: declared {declared}, wrote {actual}"
                    )
                }
                Self::GroupCountOverflow { maximum, actual } => {
                    write!(f, "group count overflow: max {maximum}, actual {actual}")
                }
                Self::EncodedLengthOverflow => {
                    write!(f, "encoded length computation overflowed")
                }
                Self::DomainConversionFailed { field, reason } => {
                    write!(f, "domain conversion failed for field {field}: {reason}")
                }
                Self::Decode(e) => write!(f, "decode error: {e}"),
            }
        }
    }
    impl core::error::Error for EncodeError {
        fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
            match self {
                Self::Decode(e) => Some(e),
                _ => None,
            }
        }
    }
    impl From<DecodeError> for EncodeError {
        #[inline]
        fn from(e: DecodeError) -> Self {
            Self::Decode(e)
        }
    }
    /// Meta attribute selector (Java `MetaAttribute` parity).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum MetaAttribute {
        /// Epoch / start of time (e.g. `"unix"`).
        Epoch,
        /// Time unit applied to the epoch (e.g. `"nanosecond"`).
        TimeUnit,
        /// SBE semantic type / FIX-tag relationship.
        SemanticType,
        /// Field presence: `"required"`, `"optional"`, or `"constant"`.
        Presence,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum VerifyError {
        /// Buffer shorter than the message header.
        HeaderTooShort,
        /// Wire block length below the minimum readable for this version.
        InvalidBlockLength { expected_min: usize, actual: usize },
        /// Group dimension header for `field` lies past the buffer end.
        GroupDimOutOfBounds { field: &'static str, offset: usize },
        /// Var-data region for `field` lies past the buffer end.
        VarDataOutOfBounds { field: &'static str, offset: usize, length: u64 },
        /// Full message (header + tails) longer than available bytes.
        MessageTooShort { needed: usize, available: usize },
        /// Nested decode error while verifying.
        DecodeError(DecodeError),
    }
    impl core::fmt::Display for VerifyError {
        #[cold]
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::HeaderTooShort => {
                    write!(f, "buffer too short to contain message header")
                }
                Self::InvalidBlockLength { expected_min, actual } => {
                    write!(
                        f, "invalid block length: expected at least {}, actual {}",
                        expected_min, actual
                    )
                }
                Self::GroupDimOutOfBounds { field, offset } => {
                    write!(
                        f, "group dimension header for '{}' out of bounds at offset {}",
                        field, offset
                    )
                }
                Self::VarDataOutOfBounds { field, offset, length } => {
                    write!(
                        f, "var-data for '{}' out of bounds at offset {} with length {}",
                        field, offset, length
                    )
                }
                Self::MessageTooShort { needed, available } => {
                    write!(
                        f, "message too short: needed {} bytes, {} available", needed,
                        available
                    )
                }
                Self::DecodeError(e) => {
                    write!(f, "decode error during verification: {e}")
                }
            }
        }
    }
    impl From<DecodeError> for VerifyError {
        #[inline]
        fn from(e: DecodeError) -> Self {
            VerifyError::DecodeError(e)
        }
    }
    impl core::error::Error for VerifyError {
        fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
            match self {
                Self::DecodeError(e) => Some(e),
                _ => None,
            }
        }
    }
    /// Convert a wire var-data length without truncation and validate
    /// the complete region with overflow-safe offset arithmetic.
    #[inline]
    pub(crate) fn checked_var_data_bounds(
        field: &'static str,
        offset: usize,
        prefix_size: usize,
        wire_length: u64,
        buffer_length: usize,
    ) -> Result<(usize, usize), DecodeError> {
        let length = usize::try_from(wire_length)
            .map_err(|_| {
                DecodeError::InvalidVarDataLength {
                    field,
                    length: wire_length,
                    max_length: usize::MAX as u64,
                }
            })?;
        let data_start = offset
            .checked_add(prefix_size)
            .ok_or(DecodeError::BufferTooShort {
                field,
                needed: usize::MAX,
                available: buffer_length.saturating_sub(offset),
            })?;
        let data_end = data_start
            .checked_add(length)
            .ok_or(DecodeError::BufferTooShort {
                field,
                needed: usize::MAX,
                available: buffer_length.saturating_sub(offset),
            })?;
        if data_end > buffer_length {
            return Err(DecodeError::BufferTooShort {
                field,
                needed: prefix_size.saturating_add(length),
                available: buffer_length.saturating_sub(offset),
            });
        }
        Ok((data_start, data_end))
    }
    #[inline]
    pub(crate) fn checked_header_u16(
        field: &'static str,
        value: u64,
    ) -> Result<u16, DecodeError> {
        u16::try_from(value)
            .map_err(|_| DecodeError::InvalidHeaderValue {
                field,
                value,
                maximum: u16::MAX as u64,
            })
    }
    #[inline]
    pub(crate) fn checked_header_usize(
        field: &'static str,
        value: u64,
    ) -> Result<usize, DecodeError> {
        usize::try_from(value)
            .map_err(|_| DecodeError::InvalidHeaderValue {
                field,
                value,
                maximum: usize::MAX as u64,
            })
    }
    /// Compile-time metadata for a generated SBE message.
    ///
    /// Sealed: the supertrait lives in a private child of the generated
    /// module, so only code generated alongside these types can name
    /// it. Generic framing code can therefore trust that a
    /// `T: SbeMessage` carries this schema's real template id, block
    /// length, schema id, and version.
    #[diagnostic::on_unimplemented(
        message = "`{Self}` is not a generated SBE message type",
        note = "SbeMessage is a sealed trait — only types generated by `ergo_sbe::Generator` can implement it. Import the generated module and use the provided decoder/encoder types directly."
    )]
    pub trait SbeMessage: super::__sbe_message_sealed::Sealed {
        const TEMPLATE_ID: u16;
        const BLOCK_LENGTH: usize;
        const SCHEMA_ID: u16;
        const SCHEMA_VERSION: u16;
    }
    mod private {
        pub trait Sealed {}
    }
    /// How a group decoder was obtained.
    ///
    /// A group reached through its message's tail is **attached**: the
    /// decoder knows the real parent body, so completing it can hand
    /// back the next message stage. A group wrapped standalone is
    /// **detached**: it iterates, random-accesses, and rewinds, but has
    /// no parent to return to, so it exposes no message-stage
    /// completion. The distinction is a zero-sized type parameter —
    /// no runtime field, branch, or allocation.
    pub trait GroupContext: private::Sealed {}
    /// Standalone group: no parent message stage. Default context.
    #[doc(hidden)]
    pub struct Detached(());
    impl private::Sealed for Detached {}
    impl GroupContext for Detached {}
    /// Group reached through a message tail; completion is available.
    ///
    /// There is deliberately no safe public constructor: attachment is
    /// a proof, not a claim a consumer can make.
    #[doc(hidden)]
    pub struct Attached(());
    impl private::Sealed for Attached {}
    impl GroupContext for Attached {}
    pub trait HeaderState: private::Sealed {}
    pub struct HeaderPresent;
    impl private::Sealed for HeaderPresent {}
    impl HeaderState for HeaderPresent {}
    pub struct HeaderAbsent;
    impl private::Sealed for HeaderAbsent {}
    impl HeaderState for HeaderAbsent {}
    /// Return type for group closures (`add`, `bids`, …).
    /// Closures return `Result<(), EncodeError>`; `?` just works.
    pub type GroupResult = Result<(), EncodeError>;
    pub trait IntoGroupResult {
        fn into_group_result(self) -> GroupResult;
    }
    impl IntoGroupResult for () {
        #[inline]
        fn into_group_result(self) -> GroupResult {
            Ok(())
        }
    }
    impl IntoGroupResult for GroupResult {
        #[inline]
        fn into_group_result(self) -> GroupResult {
            self
        }
    }
}
/// Sealing marker for [`sbe_rt::SbeMessage`]. Private to this generated
/// module: no consumer can name it, so no consumer can forge message
/// metadata by implementing `SbeMessage` for its own type.
pub(crate) mod __sbe_message_sealed {
    pub trait Sealed {}
}
/// Convert from a wire type to an application type.
pub trait TryFromSbe<Wire>: Sized {
    type Error: core::fmt::Debug + core::fmt::Display;
    fn try_from_sbe(wire: Wire) -> Result<Self, Self::Error>;
}
/// Convert from an application type to a wire type.
pub trait TryToSbe<Wire> {
    type Error: core::fmt::Debug + core::fmt::Display;
    fn try_to_sbe(&self) -> Result<Wire, Self::Error>;
}
///FIX-standard boolean: one byte on the wire.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BooleanType {
    False = 0,
    True = 1,
    /// Unknown enum value — the wire discriminant did not match any known variant.
    NullVal = 255,
}
impl BooleanType {
    /// Wire discriminant. Not `#[inline]`: measured no-LTO decode
    /// regression when forced.
    pub fn raw(self) -> u8 {
        self as u8
    }
    /// Reconstruct from a wire discriminant (`NullVal` for unknown).
    /// Not `#[inline]`: same measurement rationale as [`Self::raw`].
    pub const fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::False,
            1 => Self::True,
            _ => Self::NullVal,
        }
    }
    /// Map [`NullVal`] → [`None`], any other variant → [`Some`].
    #[inline]
    pub const fn as_option(self) -> Option<Self> {
        if matches!(self, Self::NullVal) { None } else { Some(self) }
    }
    /// Returns `Some(true)` / `Some(false)` for the valid boolean
    /// values. Returns `None` for `NullVal` or any unknown raw
    /// discriminant — the SBE boolean wire type is tri-state
    /// (F, T, null). Prefer this (or `TryFrom`) over treating the
    /// raw discriminant as a Rust `bool`.
    #[inline]
    pub const fn as_bool(self) -> Option<bool> {
        match self {
            Self::False => Some(false),
            Self::True => Some(true),
            _ => None,
        }
    }
}
impl From<BooleanType> for u8 {
    #[inline]
    fn from(val: BooleanType) -> Self {
        val as u8
    }
}
impl From<u8> for BooleanType {
    #[inline]
    fn from(val: u8) -> Self {
        Self::from_raw(val)
    }
}
impl core::fmt::Display for BooleanType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::False => f.write_str(stringify!(False)),
            Self::True => f.write_str(stringify!(True)),
            Self::NullVal => f.write_str("NullVal"),
        }
    }
}
impl core::str::FromStr for BooleanType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            stringify!(False) => Ok(Self::False),
            stringify!(True) => Ok(Self::True),
            "NullVal" => Ok(Self::NullVal),
            _ => Err(()),
        }
    }
}
impl From<bool> for BooleanType {
    #[inline]
    fn from(val: bool) -> Self {
        if val { Self::True } else { Self::False }
    }
}
impl TryFrom<BooleanType> for bool {
    type Error = ();
    #[inline]
    fn try_from(val: BooleanType) -> Result<Self, Self::Error> {
        val.as_bool().ok_or(())
    }
}
///Message identifiers and length of message root Standard 8-byte SBE message header. These names are required by the tooling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MessageHeader(pub [u8; 8]);
impl MessageHeader {
    #[inline]
    pub fn block_length(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 0))
    }
    #[inline]
    pub fn template_id(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 2))
    }
    #[inline]
    pub fn schema_id(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 4))
    }
    #[inline]
    pub fn version(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 6))
    }
    #[inline]
    pub fn new(
        block_length: u16,
        template_id: u16,
        schema_id: u16,
        version: u16,
    ) -> Self {
        let mut bytes = [0u8; 8];
        let val_bytes = block_length.to_le_bytes();
        write_bytes::<2>(&mut bytes, 0, &val_bytes);
        let val_bytes = template_id.to_le_bytes();
        write_bytes::<2>(&mut bytes, 2, &val_bytes);
        let val_bytes = schema_id.to_le_bytes();
        write_bytes::<2>(&mut bytes, 4, &val_bytes);
        let val_bytes = version.to_le_bytes();
        write_bytes::<2>(&mut bytes, 6, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(core::mem::size_of:: < MessageHeader > () == 8);
/// Canonical wire size of the SBE message header.
pub const MESSAGE_HEADER_ENCODED_LENGTH: usize = 8;
/// Parsed `(template_id, schema_id)` from [`MessageHeader::peek_header`].
/// Named fields prevent silent transposition of the two `u16` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeekedHeader {
    pub template_id: u16,
    pub schema_id: u16,
}
impl MessageHeader {
    /// Read the header fields from a buffer without constructing a
    /// full `MessageHeader`. Returns `None` when the buffer is
    /// shorter than the header.
    #[inline]
    pub fn peek_header(data: &[u8]) -> Option<PeekedHeader> {
        if data.len() < 8 {
            return None;
        }
        let mut hdr = [0u8; 8];
        hdr.copy_from_slice(&data[..8]);
        let this = Self(hdr);
        let template_id = u16::try_from(this.template_id() as u64).ok()?;
        let schema_id = u16::try_from(this.schema_id() as u64).ok()?;
        Some(PeekedHeader {
            template_id,
            schema_id,
        })
    }
    /// Read `template_id` from a frame without constructing a full
    /// `MessageHeader`. Returns `None` when the buffer is shorter
    /// than the header. For correct multi-schema dispatch,
    /// prefer [`Self::peek_header`] which also returns `schema_id`.
    #[inline]
    pub fn peek_template_id(data: &[u8]) -> Option<u16> {
        if data.len() < 8 {
            return None;
        }
        let mut hdr = [0u8; 8];
        hdr.copy_from_slice(&data[..8]);
        u16::try_from(Self(hdr).template_id() as u64).ok()
    }
    /// Validate `schema_id` and return `template_id`. Returns
    /// `None` when the buffer is too short or the schema doesn't
    /// match. Use this for correct multi-schema dispatch.
    #[inline]
    pub fn peek_for_schema(data: &[u8], expected_schema_id: u16) -> Option<u16> {
        let header = Self::peek_header(data)?;
        if header.schema_id == expected_schema_id {
            Some(header.template_id)
        } else {
            None
        }
    }
}
/// Flyweight decoder for the `messageHeader` composite.
#[derive(Clone, Copy)]
pub struct MessageHeaderDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> MessageHeaderDecoder<'a> {
    #[inline]
    pub fn block_length(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 0)
        })
    }
    #[inline]
    pub fn template_id(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 2)
        })
    }
    #[inline]
    pub fn schema_id(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 4)
        })
    }
    #[inline]
    pub fn version(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 6)
        })
    }
}
///Repeating group dimensions Repeating-group dimensions written before each group instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct GroupSizeEncoding(pub [u8; 4]);
impl GroupSizeEncoding {
    #[inline]
    pub fn block_length(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 0))
    }
    #[inline]
    pub fn num_in_group(&self) -> u16 {
        u16::from_le_bytes(read_bytes::<2>(&self.0, 2))
    }
    #[inline]
    pub fn new(block_length: u16, num_in_group: u16) -> Self {
        let mut bytes = [0u8; 4];
        let val_bytes = block_length.to_le_bytes();
        write_bytes::<2>(&mut bytes, 0, &val_bytes);
        let val_bytes = num_in_group.to_le_bytes();
        write_bytes::<2>(&mut bytes, 2, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(core::mem::size_of:: < GroupSizeEncoding > () == 4);
/// Flyweight decoder for the `groupSizeEncoding` composite.
#[derive(Clone, Copy)]
pub struct GroupSizeEncodingDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> GroupSizeEncodingDecoder<'a> {
    #[inline]
    pub fn block_length(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 0)
        })
    }
    #[inline]
    pub fn num_in_group(&self) -> u16 {
        u16::from_le_bytes(unsafe {
            read_bytes_unchecked::<2>(self.buf, self.offset + 2)
        })
    }
}
///Variable-length UTF-8 string Variable-length UTF-8 string with a uint32 byte-length prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VarStringEncoding(pub [u8; 4]);
impl VarStringEncoding {
    #[inline]
    pub fn length(&self) -> u32 {
        u32::from_le_bytes(read_bytes::<4>(&self.0, 0))
    }
    #[inline]
    pub fn var_data(&self) -> [u8; 0] {
        []
    }
    #[inline]
    pub fn new(length: u32, var_data: [u8; 0]) -> Self {
        let mut bytes = [0u8; 4];
        let val_bytes = length.to_le_bytes();
        write_bytes::<4>(&mut bytes, 0, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(core::mem::size_of:: < VarStringEncoding > () == 4);
/// Flyweight decoder for the `varStringEncoding` composite.
#[derive(Clone, Copy)]
pub struct VarStringEncodingDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> VarStringEncodingDecoder<'a> {
    #[inline]
    pub fn length(&self) -> u32 {
        u32::from_le_bytes(unsafe {
            read_bytes_unchecked::<4>(self.buf, self.offset + 0)
        })
    }
    #[inline]
    pub fn var_data(&self) -> [u8; 0] {
        []
    }
}
///UTC microseconds since the UNIX epoch UTC microseconds since the UNIX epoch; the constant unit occupies no wire bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UTCTimestampMicrosEncoding(pub [u8; 8]);
impl UTCTimestampMicrosEncoding {
    #[inline]
    pub fn time(&self) -> u64 {
        u64::from_le_bytes(read_bytes::<8>(&self.0, 0))
    }
    #[inline]
    pub const fn unit(&self) -> u8 {
        6
    }
    #[inline]
    pub fn new(time: u64) -> Self {
        let mut bytes = [0u8; 8];
        let val_bytes = time.to_le_bytes();
        write_bytes::<8>(&mut bytes, 0, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(core::mem::size_of:: < UTCTimestampMicrosEncoding > () == 8);
/// Flyweight decoder for the `UTCTimestampMicrosEncoding` composite.
#[derive(Clone, Copy)]
pub struct UTCTimestampMicrosEncodingDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> UTCTimestampMicrosEncodingDecoder<'a> {
    #[inline]
    pub fn time(&self) -> u64 {
        u64::from_le_bytes(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 0)
        })
    }
    #[inline]
    pub const fn unit(&self) -> u8 {
        6
    }
}
///```text
///UTC microseconds since the UNIX epoch, or null Same 8 bytes, but u64::MAX is reserved to mean "unknown". SBE has no
///       presence="optional" for a composite as a whole, so the nullability rides
///       on `time` and you test that member — the pattern real-logic points at in
///       issue #509. Picked by `type: timestamp, optional: true` in the yaml.
///```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UTCTimestampMicrosNullableEncoding(pub [u8; 8]);
impl UTCTimestampMicrosNullableEncoding {
    #[inline]
    pub fn time(&self) -> u64 {
        u64::from_le_bytes(read_bytes::<8>(&self.0, 0))
    }
    #[inline]
    pub const fn unit(&self) -> u8 {
        6
    }
    #[inline]
    pub fn new(time: u64) -> Self {
        let mut bytes = [0u8; 8];
        let val_bytes = time.to_le_bytes();
        write_bytes::<8>(&mut bytes, 0, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(
    core::mem::size_of:: < UTCTimestampMicrosNullableEncoding > () == 8
);
/// Flyweight decoder for the `UTCTimestampMicrosNullableEncoding` composite.
#[derive(Clone, Copy)]
pub struct UTCTimestampMicrosNullableEncodingDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> UTCTimestampMicrosNullableEncodingDecoder<'a> {
    #[inline]
    pub fn time(&self) -> u64 {
        u64::from_le_bytes(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 0)
        })
    }
    #[inline]
    pub const fn unit(&self) -> u8 {
        6
    }
}
///Orderbook side: 0 = Buy (bid), 1 = Sell (ask) The shared Side domain enum owns and validates the Buy/Sell values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SideEncoding(pub [u8; 1]);
impl SideEncoding {
    #[inline]
    pub fn value(&self) -> u8 {
        u8::from_le_bytes(read_bytes::<1>(&self.0, 0))
    }
    #[inline]
    pub fn new(value: u8) -> Self {
        let mut bytes = [0u8; 1];
        let val_bytes = value.to_le_bytes();
        write_bytes::<1>(&mut bytes, 0, &val_bytes);
        Self(bytes)
    }
}
const _: () = assert!(core::mem::size_of:: < SideEncoding > () == 1);
/// Flyweight decoder for the `SideEncoding` composite.
#[derive(Clone, Copy)]
pub struct SideEncodingDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the composite body within `self.buf`.
    pub(crate) offset: usize,
}
impl<'a> SideEncodingDecoder<'a> {
    #[inline]
    pub fn value(&self) -> u8 {
        u8::from_le_bytes(unsafe {
            read_bytes_unchecked::<1>(self.buf, self.offset + 0)
        })
    }
}
#[doc = concat!(
    "Schema constants for the `", stringify!(SnapshotSchema),
    "` message: `SCHEMA_ID`, `SCHEMA_VERSION`, `TEMPLATE_ID`, `BLOCK_LENGTH`, `HEADER_LENGTH`."
)]
pub struct SnapshotSchema;
impl SnapshotSchema {
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 1;
    pub const BLOCK_LENGTH: usize = 16;
    pub const HEADER_LENGTH: usize = 8;
    /// Full structural verification of a buffer: validates header,
    /// block-length extent, group dimension headers, entry strides,
    /// and var-data bounds. Use **before** construction when the
    /// entire frame must be proven valid without building a decoder.
    #[inline]
    pub fn verify(buf: &[u8]) -> Result<(), sbe_rt::VerifyError> {
        SnapshotDecoder::verify(buf)
    }
}
///A full snapshot of the orderbook
#[must_use = "decoder must be read or advanced; dropping is fine only after use"]
pub struct SnapshotDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the message body within `self.buf`.
    pub(crate) offset: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
/// Buffer-placement and wire-frame metadata. Holds a reference to the
/// parent decoder — zero-copy. Utility methods live here so no schema
/// field can collide with them. Byte views on this facet span the
/// **acting fixed block only**; complete frames use the complete stage
/// or the decoder's tail-rescan helpers when the message has groups
/// or var-data.
#[derive(Clone, Copy)]
pub struct SnapshotDecoderMetadata<'m, 'a> {
    decoder: &'m SnapshotDecoder<'a>,
}
impl<'m, 'a> SnapshotDecoderMetadata<'m, 'a> {
    /// Absolute offset of this message's frame start (first header byte)
    /// within the underlying buffer.
    #[inline]
    pub fn message_offset(&self) -> usize {
        self.decoder.byte_offset().saturating_sub(SnapshotDecoder::HEADER_LENGTH)
    }
    /// End of the **acting fixed block** (body start + acting block length).
    /// Not the full message end when groups/var-data follow — use a complete
    /// stage or inherent `encoded_length_with_header` after walking tails.
    #[inline]
    pub fn limit(&self) -> usize {
        self.decoder.byte_offset() + self.decoder.acting_block_length
    }
    /// The full underlying buffer slice this decoder was wrapped on.
    #[inline]
    pub fn buffer(&self) -> &'a [u8] {
        self.decoder.buf
    }
    /// Bytes after the acting fixed block end. May still contain unread
    /// groups/var-data of **this** message until the consuming walk finishes.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        let end = (self.decoder.byte_offset() + self.decoder.acting_block_length)
            .min(self.decoder.buf.len());
        &self.decoder.buf[end..]
    }
    /// Fixed-block body only (groups/var-data not included).
    /// For a complete frame walk tails then use the complete stage's
    /// `as_bytes_with_header`, or the decoder's inherent
    /// `as_bytes_with_header` which rescans tails without consuming
    /// the stage.
    #[inline]
    pub fn as_fixed_body_bytes(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let start = self.decoder.byte_offset();
        let end = self.decoder.byte_offset() + self.decoder.acting_block_length;
        if start > self.decoder.buf.len() || end > self.decoder.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "body",
                needed: end.saturating_sub(start),
                available: self.decoder.buf.len().saturating_sub(start),
            });
        }
        Ok(&self.decoder.buf[start..end])
    }
    /// Header + fixed block only — **not** a complete SBE message when
    /// groups or var-data remain. Prefer the complete stage's
    /// `as_bytes_with_header` after finishing the walk.
    #[inline]
    pub fn as_fixed_region_with_header(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let start = self.message_offset();
        let end = self.decoder.byte_offset() + self.decoder.acting_block_length;
        if start > self.decoder.buf.len() || end > self.decoder.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "frame",
                needed: end.saturating_sub(start),
                available: self.decoder.buf.len().saturating_sub(start),
            });
        }
        Ok(&self.decoder.buf[start..end])
    }
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub fn acting_version(&self) -> u16 {
        self.decoder.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub fn acting_block_length(&self) -> usize {
        self.decoder.acting_block_length
    }
}
impl<'a> SnapshotDecoder<'a> {
    /// Metadata accessor: buffer positions, wire-frame boundaries,
    /// version/block-length state. Returns a zero-copy reference to
    /// the parent decoder — no fields are copied. All utility methods
    /// are scoped here so no schema field name can collide with them.
    #[inline]
    pub fn get_metadata(&self) -> SnapshotDecoderMetadata<'_, 'a> {
        SnapshotDecoderMetadata {
            decoder: self,
        }
    }
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 1;
    pub const BLOCK_LENGTH: usize = 16;
    const _BLOCK_LEN: () = assert!(Self::BLOCK_LENGTH == 16);
    /// Schema-declared message header size in bytes.
    pub const HEADER_LENGTH: usize = 8;
    /// Minimum body bytes needed to safely read every fixed field present
    /// at `acting_version` (version-aware; not always full `BLOCK_LENGTH`).
    #[inline]
    pub const fn min_readable_fixed_extent(acting_version: u16) -> usize {
        let mut m = 16;
        m
    }
    /// Wrap a buffer for decoding at **message start** with bounds checks.
    /// Fields are at `message_offset + HEADER_LENGTH + field_offset`.
    ///
    /// Validates that the body holds `max(acting_block_length,
    /// min_readable_fixed_extent(acting_version))` bytes so required
    /// accessors never read out of bounds from safe code.
    ///
    /// # Migration from sbe-tool
    ///
    /// sbe-tool Rust `wrap` takes the **body** offset (usually
    /// `message_start + 8`). ergo-sbe takes the **message** start so the
    /// same offset works for `wrap`, `decode`, and claim buffers.
    #[inline]
    pub fn try_wrap(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Result<Self, sbe_rt::DecodeError> {
        let Some(body_offset) = message_offset.checked_add(Self::HEADER_LENGTH) else {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: Self::HEADER_LENGTH,
                available: buf.len().saturating_sub(message_offset),
            });
        };
        let available_body = buf.len().saturating_sub(body_offset);
        let min_fixed = Self::min_readable_fixed_extent(acting_version);
        let body_need = if acting_block_length > min_fixed {
            acting_block_length
        } else {
            min_fixed
        };
        if body_need > available_body {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message body",
                needed: Self::HEADER_LENGTH.saturating_add(body_need),
                available: buf.len().saturating_sub(message_offset),
            });
        }
        Ok(unsafe {
            Self::wrap_unchecked(
                buf,
                message_offset,
                acting_block_length,
                acting_version,
            )
        })
    }
    /// Trusted external-metadata wrap. Proves version-aware fixed extent
    /// then constructs; **panics** if the buffer is too short. Field
    /// accessors use unchecked reads justified by that proof.
    ///
    /// Prefer [`Self::try_wrap`] at untrusted boundaries. Uses a direct
    /// extent check (not `try_wrap` + match) so the hot success path does
    /// not construct a `Result` — same contract as encoder bare `wrap`.
    #[inline]
    pub fn wrap(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        let Some(body_offset) = message_offset.checked_add(Self::HEADER_LENGTH) else {
            panic!("buffer too short for message header");
        };
        let available_body = buf.len().saturating_sub(body_offset);
        let min_fixed = Self::min_readable_fixed_extent(acting_version);
        let body_need = if acting_block_length > min_fixed {
            acting_block_length
        } else {
            min_fixed
        };
        if body_need > available_body {
            panic!("buffer too short for message body");
        }
        unsafe {
            Self::wrap_unchecked(
                buf,
                message_offset,
                acting_block_length,
                acting_version,
            )
        }
    }
    /// Zero-check wrap — raw pointer accessors, **UB** on OOB.
    /// Only for proven-tight hot loops after an external extent proof.
    ///
    /// # Safety
    /// `message_offset + HEADER_LENGTH + max(acting_block_length,
    /// min_readable_fixed_extent(acting_version))` must not overflow
    /// and must be ≤ `buf.len()`.
    #[inline]
    pub unsafe fn wrap_unchecked(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        let body_offset = message_offset + Self::HEADER_LENGTH;
        Self {
            buf,
            offset: body_offset,
            acting_block_length,
            acting_version,
        }
    }
    /// Decode a framed message at **message start** (`offset` = first
    /// byte of the header). Validates header fields and the
    /// version-aware fixed body extent. See [`Self::wrap`] for the
    /// message-start coordinate system.
    #[inline]
    pub fn try_decode(
        buf: &'a [u8],
        offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        if 8 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: 8,
                available: buf.len().saturating_sub(offset),
            });
        }
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "Snapshot",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Self::try_wrap(buf, offset, acting_block_length, acting_version)
    }
    /// Trusted framed decode — **hybrid return** (freeze-friendly):
    ///
    /// - **Extent (short buffer):** panics after the same proof as
    ///   [`Self::wrap`] (trusted tier).
    /// - **Identity (wrong template/schema):** still returns `Err`
    ///   so session demux can recover without catch_unwind.
    ///
    /// Signature therefore looks like [`Self::try_decode`], but short
    /// buffers do **not** yield `BufferTooShort` — they panic. Prefer
    /// [`Self::try_decode`] at untrusted boundaries when every failure
    /// must be a `Result`.
    #[inline]
    pub fn decode(buf: &'a [u8], offset: usize) -> Result<Self, sbe_rt::DecodeError> {
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "Snapshot",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Ok(Self::wrap(buf, offset, acting_block_length, acting_version))
    }
    /// Unchecked **extent**, checked **identity**.
    ///
    /// Header/body bytes are read without bounds checks (**UB** if the
    /// caller has not proven the frame fits). Template/schema identity
    /// still returns `Err` (same hybrid policy as [`Self::decode`]).
    ///
    /// # Safety
    /// Header and version-readable fixed body for this template must
    /// be fully in-bounds at `offset`.
    #[inline]
    pub unsafe fn decode_unchecked(
        buf: &'a [u8],
        offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        let header_bytes: [u8; 8] = unsafe { read_bytes_unchecked::<8>(buf, offset) };
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "Snapshot",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Ok(unsafe {
            Self::wrap_unchecked(buf, offset, acting_block_length, acting_version)
        })
    }
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
    #[inline]
    pub fn received_at_wire(&self) -> UTCTimestampMicrosEncodingDecoder<'_> {
        UTCTimestampMicrosEncodingDecoder {
            buf: self.buf,
            offset: self.offset + 0,
        }
    }
    #[inline]
    pub fn received_at_value(&self) -> UTCTimestampMicrosEncoding {
        UTCTimestampMicrosEncoding(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 0)
        })
    }
    pub const RECEIVED_AT_ID: u16 = 1;
    pub const RECEIVED_AT_SINCE_VERSION: u16 = 0;
    pub const RECEIVED_AT_ENCODING_OFFSET: usize = 0;
    pub const RECEIVED_AT_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn received_at_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    #[inline]
    pub fn exchange_timestamp_wire(&self) -> UTCTimestampMicrosEncodingDecoder<'_> {
        UTCTimestampMicrosEncodingDecoder {
            buf: self.buf,
            offset: self.offset + 8,
        }
    }
    #[inline]
    pub fn exchange_timestamp_value(&self) -> UTCTimestampMicrosEncoding {
        UTCTimestampMicrosEncoding(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 8)
        })
    }
    pub const EXCHANGE_TIMESTAMP_ID: u16 = 2;
    pub const EXCHANGE_TIMESTAMP_SINCE_VERSION: u16 = 0;
    pub const EXCHANGE_TIMESTAMP_ENCODING_OFFSET: usize = 8;
    pub const EXCHANGE_TIMESTAMP_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn exchange_timestamp_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    /// Byte offset of the message body within `self.buf`.
    #[inline]
    fn byte_offset(&self) -> usize {
        self.offset
    }
    #[inline]
    fn tail_offset_0(&self) -> Result<usize, sbe_rt::DecodeError> {
        Ok(self.byte_offset() + self.acting_block_length)
    }
    #[inline]
    fn tail_offset_1(&self) -> Result<usize, sbe_rt::DecodeError> {
        let start = self.tail_offset_0()?;
        if start + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: 4,
                available: self.buf.len().saturating_sub(start),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(self.buf, start);
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_len = header.block_length() as usize;
        let mut offset = start + 4;
        let mut idx = 0;
        while idx < count {
            offset = SnapshotBidsEntryDecoder::skip(
                self.buf,
                offset,
                block_len,
                self.acting_version,
            )?;
            idx += 1;
        }
        Ok(offset)
    }
    #[inline]
    fn tail_offset_2(&self) -> Result<usize, sbe_rt::DecodeError> {
        let start = self.tail_offset_1()?;
        if start + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: 4,
                available: self.buf.len().saturating_sub(start),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(self.buf, start);
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_len = header.block_length() as usize;
        let mut offset = start + 4;
        let mut idx = 0;
        while idx < count {
            offset = SnapshotAsksEntryDecoder::skip(
                self.buf,
                offset,
                block_len,
                self.acting_version,
            )?;
            idx += 1;
        }
        Ok(offset)
    }
    #[inline]
    fn tail_offset_3(&self) -> Result<usize, sbe_rt::DecodeError> {
        let start = self.tail_offset_2()?;
        if 4 > self.buf.len().saturating_sub(start) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(start),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(self.buf, start);
        let header = VarStringEncoding(bytes);
        let wire_length = header.length() as u64;
        let (_, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            start,
            4,
            wire_length,
            self.buf.len(),
        )?;
        Ok(data_end)
    }
    #[inline]
    fn bids(&self) -> Result<SnapshotBidsDecoder<'a>, sbe_rt::DecodeError> {
        let offset = self.tail_offset_0()?;
        SnapshotBidsDecoder::wrap(self.buf, offset, self.acting_version)
    }
    #[inline]
    fn asks(&self) -> Result<SnapshotAsksDecoder<'a>, sbe_rt::DecodeError> {
        let offset = self.tail_offset_1()?;
        SnapshotAsksDecoder::wrap(self.buf, offset, self.acting_version)
    }
    #[inline]
    pub fn symbol(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let offset = self.tail_offset_2()?;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: stringify!(symbol),
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let header = VarStringEncoding(bytes);
        let wire_length = header.length() as u64;
        if wire_length > 1073741824 as u64 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: stringify!(symbol),
                length: wire_length,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            stringify!(symbol),
            offset,
            4,
            wire_length,
            self.buf.len(),
        )?;
        Ok(&self.buf[data_start..data_end])
    }
    /// View this UTF-8 var-data field as `&str`.
    #[inline]
    pub fn symbol_as_str(&self) -> Result<&'a str, sbe_rt::DecodeError> {
        let bytes = self.symbol()?;
        core::str::from_utf8(bytes)
            .map_err(|e| sbe_rt::DecodeError::InvalidUtf8 {
                field: "symbol",
                error: e,
            })
    }
    /// View this text var-data field as `&str` without UTF-8
    /// validation.
    ///
    /// # Safety
    ///
    /// The wire bytes must be valid UTF-8.
    #[inline]
    pub unsafe fn symbol_as_str_unchecked(&self) -> &'a str {
        let bytes = unsafe { self.symbol().unwrap() };
        unsafe { core::str::from_utf8_unchecked(bytes) }
    }
    /// Consume this stage and return a fresh decoder at the initial
    /// message position. The consumed stage cannot be reused.
    #[inline]
    pub fn rewind(self) -> Self {
        self
    }
    #[inline]
    pub fn encoded_length(&self) -> Result<usize, sbe_rt::DecodeError> {
        let end = self.tail_offset_3()?;
        Ok(end - self.byte_offset())
    }
    #[inline]
    pub fn encoded_length_with_header(&self) -> Result<usize, sbe_rt::DecodeError> {
        let len = self.encoded_length()?;
        Ok(len + 8)
    }
    #[inline]
    pub fn as_body_bytes(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let end = self.tail_offset_3()?;
        let start = self.byte_offset();
        Ok(&self.buf[start..end])
    }
    #[inline]
    pub fn as_bytes_with_header(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let end = self.tail_offset_3()?;
        let start = self.byte_offset().saturating_sub(Self::HEADER_LENGTH);
        Ok(&self.buf[start..end])
    }
    #[inline]
    pub fn verify(buf: &[u8]) -> Result<(), sbe_rt::VerifyError> {
        if buf.len() < 8 {
            return Err(sbe_rt::VerifyError::HeaderTooShort);
        }
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, 0);
        let header = MessageHeader(header_bytes);
        let block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        if block_length < Self::BLOCK_LENGTH {
            return Err(sbe_rt::VerifyError::InvalidBlockLength {
                expected_min: Self::BLOCK_LENGTH,
                actual: block_length,
            });
        }
        let body_end = (8 as usize)
            .checked_add(block_length)
            .ok_or(sbe_rt::VerifyError::MessageTooShort {
                needed: usize::MAX,
                available: buf.len(),
            })?;
        if body_end > buf.len() {
            return Err(sbe_rt::VerifyError::MessageTooShort {
                needed: body_end,
                available: buf.len(),
            });
        }
        let mut offset = body_end;
        {
            if offset + 4 > buf.len() {
                return Err(sbe_rt::VerifyError::GroupDimOutOfBounds {
                    field: "bids",
                    offset,
                });
            }
            let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
            let dim = GroupSizeEncoding(bytes);
            let count = dim.num_in_group() as usize;
            let entries_end = offset + 4 + count * 16;
            if entries_end > buf.len() {
                return Err(sbe_rt::VerifyError::MessageTooShort {
                    needed: entries_end,
                    available: buf.len(),
                });
            }
            offset = entries_end;
        }
        {
            if offset + 4 > buf.len() {
                return Err(sbe_rt::VerifyError::GroupDimOutOfBounds {
                    field: "asks",
                    offset,
                });
            }
            let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
            let dim = GroupSizeEncoding(bytes);
            let count = dim.num_in_group() as usize;
            let entries_end = offset + 4 + count * 16;
            if entries_end > buf.len() {
                return Err(sbe_rt::VerifyError::MessageTooShort {
                    needed: entries_end,
                    available: buf.len(),
                });
            }
            offset = entries_end;
        }
        {
            if 4 > buf.len().saturating_sub(offset) {
                return Err(sbe_rt::VerifyError::VarDataOutOfBounds {
                    field: "symbol",
                    offset,
                    length: 0,
                });
            }
            let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
            let var_header = VarStringEncoding(bytes);
            let len = var_header.length() as u64;
            let (_, data_end) = match sbe_rt::checked_var_data_bounds(
                "symbol",
                offset,
                4,
                len,
                buf.len(),
            ) {
                Ok(bounds) => bounds,
                Err(_) => {
                    return Err(sbe_rt::VerifyError::VarDataOutOfBounds {
                        field: "symbol",
                        offset,
                        length: len,
                    });
                }
            };
            offset = data_end;
        }
        Ok(())
    }
}
impl<'a> TryFrom<&'a [u8]> for SnapshotDecoder<'a> {
    type Error = sbe_rt::DecodeError;
    #[inline]
    fn try_from(buf: &'a [u8]) -> Result<Self, Self::Error> {
        Self::try_decode(buf, 0)
    }
}
impl<'a> __sbe_message_sealed::Sealed for SnapshotDecoder<'a> {}
impl<'a> sbe_rt::SbeMessage for SnapshotDecoder<'a> {
    const TEMPLATE_ID: u16 = 1;
    const BLOCK_LENGTH: usize = 16;
    const SCHEMA_ID: u16 = 1;
    const SCHEMA_VERSION: u16 = 2;
}
impl<'a> core::fmt::Display for SnapshotDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}
impl<'a> core::fmt::Debug for SnapshotDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("SnapshotDecoder");
        if self.byte_offset().saturating_add(8) <= self.buf.len()
            && 8 <= self.acting_block_length
        {
            if let Ok(v) = self.try_received_at() {
                d.field("received_at", &format_args!("{}", v));
            }
        }
        if self.byte_offset().saturating_add(16) <= self.buf.len()
            && 16 <= self.acting_block_length
        {
            if let Ok(v) = self.try_exchange_timestamp() {
                d.field("exchange_timestamp", &format_args!("{}", v));
            }
        }
        if let Ok(_g) = self.bids() {
            let entries: Vec<String> = _g.map(|e| format!("{e}")).collect();
            d.field("bids", &entries);
        }
        if let Ok(_g) = self.asks() {
            let entries: Vec<String> = _g.map(|e| format!("{e}")).collect();
            d.field("asks", &entries);
        }
        if let Ok(_data) = self.symbol() {
            match std::str::from_utf8(_data) {
                Ok(_s) => d.field("symbol", &_s),
                Err(_) => d.field("symbol", &format!("<{} bytes>", _data.len())),
            };
        }
        d.finish()
    }
}
///Bid price levels
pub struct SnapshotBidsDecoder<'a, C: sbe_rt::GroupContext = sbe_rt::Detached> {
    buf: &'a [u8],
    offset: usize,
    count: usize,
    start: usize,
    total: usize,
    acting_version: u16,
    acting_block_length: usize,
    parent_pos: usize,
    parent_block_length: usize,
    _context: core::marker::PhantomData<C>,
}
impl<'a, C: sbe_rt::GroupContext> SnapshotBidsDecoder<'a, C> {
    /// Proof-dependent constructor: like `wrap()` but remembers the
    /// parent message body position and acting block length so
    /// `finish()` can rebuild the next stage.
    ///
    /// Private to the generated module — a caller outside it cannot
    /// invent parent state and then `finish()` into a message stage
    /// that never existed.
    ///
    /// # Safety
    /// `parent_pos` and `parent_block_length` must describe the message
    /// body this group is genuinely nested in, and `offset` must be that
    /// message's real dimension-header offset for this group. The
    /// dimension header, the acting block length, and the group extent
    /// are still validated here and may be untrusted.
    #[inline]
    unsafe fn wrap_with_parent(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
        parent_pos: usize,
        parent_block_length: usize,
    ) -> Result<SnapshotBidsDecoder<'a, sbe_rt::Attached>, sbe_rt::DecodeError> {
        if 4 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: 4,
                available: buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_length = header.block_length() as usize;
        let entries_start = offset + 4;
        let min_fixed = <SnapshotBidsDecoder<
            '_,
            sbe_rt::Detached,
        >>::min_readable_fixed_extent(acting_version);
        if count > 0 && block_length < min_fixed {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: min_fixed,
                available: block_length,
            });
        }
        let entries_length = count
            .checked_mul(block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: usize::MAX,
                available: buf.len().saturating_sub(entries_start),
            })?;
        if entries_length > buf.len().saturating_sub(entries_start) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: entries_length,
                available: buf.len().saturating_sub(entries_start),
            });
        }
        Ok(SnapshotBidsDecoder {
            buf,
            offset: entries_start,
            count,
            start: entries_start,
            total: count,
            acting_version,
            acting_block_length: block_length,
            parent_pos,
            parent_block_length,
            _context: core::marker::PhantomData,
        })
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}
impl<'a> SnapshotBidsDecoder<'a, sbe_rt::Detached> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Minimum entry bytes needed to safely read every **required**
    /// fixed field present at `acting_version`.
    ///
    /// Version-aware, and not always the compiled
    /// `ENTRY_BLOCK_LENGTH`: a forward-compatible reader accepts a
    /// wire block length it does not recognise, but never one too
    /// small for the fields it will actually read.
    #[inline]
    pub const fn min_readable_fixed_extent(acting_version: u16) -> usize {
        let mut m = 16;
        m
    }
    /// Wrap a standalone group at its dimension header, with bounds
    /// checks.
    ///
    /// This is the only public constructor. It validates the dimension
    /// header, rejects a wire block length too small to hold the
    /// required fixed fields active at `acting_version`, and — for
    /// fixed-stride groups — proves the whole entry region at once.
    ///
    /// The result is *detached*: it iterates, random-accesses, and
    /// rewinds, but has no parent message to complete into, so it has
    /// no `finish` / `skip_remaining`.
    #[inline]
    pub fn wrap(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
    ) -> Result<SnapshotBidsDecoder<'a, sbe_rt::Detached>, sbe_rt::DecodeError> {
        let attached = unsafe {
            <SnapshotBidsDecoder<
                'a,
                sbe_rt::Attached,
            >>::wrap_with_parent(buf, offset, acting_version, 0, 0)?
        };
        Ok(SnapshotBidsDecoder {
            buf: attached.buf,
            offset: attached.offset,
            count: attached.count,
            start: attached.start,
            total: attached.total,
            acting_version: attached.acting_version,
            acting_block_length: attached.acting_block_length,
            parent_pos: attached.parent_pos,
            parent_block_length: attached.parent_block_length,
            _context: core::marker::PhantomData,
        })
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotBidsDecoder<'a, C> {
    /// Entries not yet advanced (count), not a byte slice.
    /// For message-level byte tails use `get_metadata().remaining()`.
    #[inline]
    pub const fn remaining(&self) -> usize {
        self.count
    }
    /// Dimension wrap after the caller has proven
    /// the dimension header (and, for fixed groups, the full entry
    /// region) is in-bounds. Prefer [`Self::wrap`] / [`Self::wrap_with_parent`].
    ///
    /// # Safety
    /// `offset + dimension_header_size` must not overflow and must be
    /// ≤ `buf.len()`. For fixed-block groups (no nested tail),
    /// `offset + dim + count * acting_block_length` must also fit. Entry
    /// accessors then use unchecked fixed-field reads under that proof.
    #[inline]
    pub(crate) unsafe fn wrap_trusted(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
        parent_pos: usize,
        parent_block_length: usize,
    ) -> Self {
        let bytes: [u8; 4] = unsafe { read_bytes_unchecked::<4>(buf, offset) };
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_length = header.block_length() as usize;
        let min_fixed = <SnapshotBidsDecoder<
            '_,
            sbe_rt::Detached,
        >>::min_readable_fixed_extent(acting_version);
        Self {
            buf,
            offset: offset + 4,
            count,
            start: offset + 4,
            total: count,
            acting_version,
            acting_block_length: block_length,
            parent_pos,
            parent_block_length,
            _context: core::marker::PhantomData,
        }
    }
    /// Restart iteration from the group's proven start.
    ///
    /// This is the one operation that clears a poisoned group: the
    /// start offset was validated at wrap time, so retrying from there
    /// is sound even after an entry failed.
    #[inline]
    pub fn rewind(&mut self) -> &mut Self {
        self.offset = self.start;
        self.count = self.total;
        self
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotBidsDecoder<'a, C> {
    #[inline]
    pub fn skip_n(&mut self, n: usize) -> Result<(), sbe_rt::DecodeError> {
        if n > self.count {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: n.saturating_mul(self.acting_block_length),
                available: self.count.saturating_mul(self.acting_block_length),
            });
        }
        self.offset += n.saturating_mul(self.acting_block_length);
        self.count -= n;
        Ok(())
    }
    /// Bulk-decode all remaining entries into a caller-owned `Vec`.
    /// Zero-allocation after warm-up — the caller reuses the
    /// destination buffer across messages.
    #[inline]
    pub fn bulk_decode_into(
        &mut self,
        dst: &mut Vec<SnapshotBidsEntry>,
    ) -> Result<usize, sbe_rt::DecodeError> {
        let needed = self
            .count
            .checked_mul(self.acting_block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: usize::MAX,
                available: 0,
            })?;
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let cap = self.count;
        dst.clear();
        dst.reserve(cap);
        for _ in 0..cap {
            let offset = self.offset;
            self.offset += self.acting_block_length;
            dst.push(SnapshotBidsEntry {
                price_level: i64::from_le_bytes(
                    self.buf[offset + 0..offset + 0 + 8].try_into().unwrap(),
                ),
                quantity: i64::from_le_bytes(
                    self.buf[offset + 8..offset + 8 + 8].try_into().unwrap(),
                ),
            });
        }
        self.count = 0;
        Ok(cap)
    }
    /// Bulk-decode all remaining entries into a new `Vec`.
    /// Convenience wrapper around [`Self::bulk_decode_into`].
    /// One bounds check for the whole batch — faster than
    /// iterating with [`Iterator::next`] when materialising
    /// the entire group (DTO construction, snapshots).
    #[inline]
    pub fn bulk_decode(
        &mut self,
    ) -> Result<Vec<SnapshotBidsEntry>, sbe_rt::DecodeError> {
        let mut out = Vec::new();
        self.bulk_decode_into(&mut out)?;
        Ok(out)
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotBidsDecoder<'a, C> {
    #[inline]
    pub fn entry_at(
        &self,
        idx: usize,
    ) -> Result<SnapshotBidsEntryDecoder<'a>, sbe_rt::DecodeError> {
        if idx >= self.total {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: idx.saturating_add(1).saturating_mul(self.acting_block_length),
                available: self.total.saturating_mul(self.acting_block_length),
            });
        }
        let byte_offset = idx
            .checked_mul(self.acting_block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: usize::MAX,
                available: self.buf.len().saturating_sub(self.start),
            })?;
        let offset = self
            .start
            .checked_add(byte_offset)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: usize::MAX,
                available: self.buf.len().saturating_sub(self.start),
            })?;
        if self.acting_block_length > self.buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "bids",
                needed: self.acting_block_length,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        Ok(unsafe {
            SnapshotBidsEntryDecoder::wrap(
                self.buf,
                offset,
                self.acting_block_length,
                self.acting_version,
            )
        })
    }
}
impl<'a, C: sbe_rt::GroupContext> Iterator for SnapshotBidsDecoder<'a, C> {
    type Item = SnapshotBidsEntryDecoder<'a>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let entry = unsafe {
            SnapshotBidsEntryDecoder::wrap(
                self.buf,
                self.offset,
                self.acting_block_length,
                self.acting_version,
            )
        };
        self.offset += self.acting_block_length;
        self.count -= 1;
        Some(entry)
    }
}
impl<'a, C: sbe_rt::GroupContext> ExactSizeIterator for SnapshotBidsDecoder<'a, C> {
    #[inline]
    fn len(&self) -> usize {
        self.count
    }
}
///Bid price levels
pub struct SnapshotBidsEntryDecoder<'a> {
    buf: &'a [u8],
    offset: usize,
    acting_version: u16,
    acting_block_length: usize,
}
impl<'a> SnapshotBidsEntryDecoder<'a> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Private entry wrap after the group iterator (or equivalent)
    /// has proven the acting fixed block is in-bounds at `offset`.
    ///
    /// # Safety
    /// `offset + max(acting_block_length, ENTRY_BLOCK_LENGTH)` (and any
    /// field offset used by accessors) must not overflow and must be
    /// ≤ `buf.len()`. Fixed-field getters may then use unchecked reads.
    #[inline]
    unsafe fn wrap(
        buf: &'a [u8],
        offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        Self {
            buf,
            offset,
            acting_version,
            acting_block_length,
        }
    }
    #[inline]
    pub fn price_level(&self) -> i64 {
        let offset = self.offset + 0;
        i64::from_le_bytes(unsafe { read_bytes_unchecked::<8>(self.buf, offset) })
    }
    pub const PRICE_LEVEL_ID: u16 = 5;
    pub const PRICE_LEVEL_SINCE_VERSION: u16 = 0;
    pub const PRICE_LEVEL_ENCODING_OFFSET: usize = 0;
    pub const PRICE_LEVEL_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn price_level_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const PRICE_LEVEL_NULL: i64 = -9223372036854775808_i64;
    pub const PRICE_LEVEL_MIN: i64 = -9223372036854775807_i64;
    pub const PRICE_LEVEL_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    pub fn quantity(&self) -> i64 {
        let offset = self.offset + 8;
        i64::from_le_bytes(unsafe { read_bytes_unchecked::<8>(self.buf, offset) })
    }
    pub const QUANTITY_ID: u16 = 6;
    pub const QUANTITY_SINCE_VERSION: u16 = 0;
    pub const QUANTITY_ENCODING_OFFSET: usize = 8;
    pub const QUANTITY_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn quantity_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const QUANTITY_NULL: i64 = -9223372036854775808_i64;
    pub const QUANTITY_MIN: i64 = -9223372036854775807_i64;
    pub const QUANTITY_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    fn tail_offset_0(&self) -> Result<usize, sbe_rt::DecodeError> {
        if self.acting_block_length > self.buf.len().saturating_sub(self.offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "group entry",
                needed: self.acting_block_length,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        Ok(self.offset + self.acting_block_length)
    }
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.acting_block_length
    }
    #[inline]
    pub fn skip(
        buf: &'a [u8],
        offset: usize,
        block_len: usize,
        _acting_version: u16,
    ) -> Result<usize, sbe_rt::DecodeError> {
        if block_len > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: buf.len().saturating_sub(offset),
            });
        }
        Ok(offset + block_len)
    }
}
impl<'a> core::fmt::Display for SnapshotBidsEntryDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{ ")?;
        {
            let v = self.price_level();
            write!(f, "price_level: {:?}", v)?;
        }
        {
            let v = self.quantity();
            write!(f, ", quantity: {:?}", v)?;
        }
        write!(f, " }}")
    }
}
///Ask price levels
pub struct SnapshotAsksDecoder<'a, C: sbe_rt::GroupContext = sbe_rt::Detached> {
    buf: &'a [u8],
    offset: usize,
    count: usize,
    start: usize,
    total: usize,
    acting_version: u16,
    acting_block_length: usize,
    parent_pos: usize,
    parent_block_length: usize,
    _context: core::marker::PhantomData<C>,
}
impl<'a, C: sbe_rt::GroupContext> SnapshotAsksDecoder<'a, C> {
    /// Proof-dependent constructor: like `wrap()` but remembers the
    /// parent message body position and acting block length so
    /// `finish()` can rebuild the next stage.
    ///
    /// Private to the generated module — a caller outside it cannot
    /// invent parent state and then `finish()` into a message stage
    /// that never existed.
    ///
    /// # Safety
    /// `parent_pos` and `parent_block_length` must describe the message
    /// body this group is genuinely nested in, and `offset` must be that
    /// message's real dimension-header offset for this group. The
    /// dimension header, the acting block length, and the group extent
    /// are still validated here and may be untrusted.
    #[inline]
    unsafe fn wrap_with_parent(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
        parent_pos: usize,
        parent_block_length: usize,
    ) -> Result<SnapshotAsksDecoder<'a, sbe_rt::Attached>, sbe_rt::DecodeError> {
        if 4 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: 4,
                available: buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_length = header.block_length() as usize;
        let entries_start = offset + 4;
        let min_fixed = <SnapshotAsksDecoder<
            '_,
            sbe_rt::Detached,
        >>::min_readable_fixed_extent(acting_version);
        if count > 0 && block_length < min_fixed {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: min_fixed,
                available: block_length,
            });
        }
        let entries_length = count
            .checked_mul(block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: usize::MAX,
                available: buf.len().saturating_sub(entries_start),
            })?;
        if entries_length > buf.len().saturating_sub(entries_start) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: entries_length,
                available: buf.len().saturating_sub(entries_start),
            });
        }
        Ok(SnapshotAsksDecoder {
            buf,
            offset: entries_start,
            count,
            start: entries_start,
            total: count,
            acting_version,
            acting_block_length: block_length,
            parent_pos,
            parent_block_length,
            _context: core::marker::PhantomData,
        })
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}
impl<'a> SnapshotAsksDecoder<'a, sbe_rt::Detached> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Minimum entry bytes needed to safely read every **required**
    /// fixed field present at `acting_version`.
    ///
    /// Version-aware, and not always the compiled
    /// `ENTRY_BLOCK_LENGTH`: a forward-compatible reader accepts a
    /// wire block length it does not recognise, but never one too
    /// small for the fields it will actually read.
    #[inline]
    pub const fn min_readable_fixed_extent(acting_version: u16) -> usize {
        let mut m = 16;
        m
    }
    /// Wrap a standalone group at its dimension header, with bounds
    /// checks.
    ///
    /// This is the only public constructor. It validates the dimension
    /// header, rejects a wire block length too small to hold the
    /// required fixed fields active at `acting_version`, and — for
    /// fixed-stride groups — proves the whole entry region at once.
    ///
    /// The result is *detached*: it iterates, random-accesses, and
    /// rewinds, but has no parent message to complete into, so it has
    /// no `finish` / `skip_remaining`.
    #[inline]
    pub fn wrap(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
    ) -> Result<SnapshotAsksDecoder<'a, sbe_rt::Detached>, sbe_rt::DecodeError> {
        let attached = unsafe {
            <SnapshotAsksDecoder<
                'a,
                sbe_rt::Attached,
            >>::wrap_with_parent(buf, offset, acting_version, 0, 0)?
        };
        Ok(SnapshotAsksDecoder {
            buf: attached.buf,
            offset: attached.offset,
            count: attached.count,
            start: attached.start,
            total: attached.total,
            acting_version: attached.acting_version,
            acting_block_length: attached.acting_block_length,
            parent_pos: attached.parent_pos,
            parent_block_length: attached.parent_block_length,
            _context: core::marker::PhantomData,
        })
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotAsksDecoder<'a, C> {
    /// Entries not yet advanced (count), not a byte slice.
    /// For message-level byte tails use `get_metadata().remaining()`.
    #[inline]
    pub const fn remaining(&self) -> usize {
        self.count
    }
    /// Dimension wrap after the caller has proven
    /// the dimension header (and, for fixed groups, the full entry
    /// region) is in-bounds. Prefer [`Self::wrap`] / [`Self::wrap_with_parent`].
    ///
    /// # Safety
    /// `offset + dimension_header_size` must not overflow and must be
    /// ≤ `buf.len()`. For fixed-block groups (no nested tail),
    /// `offset + dim + count * acting_block_length` must also fit. Entry
    /// accessors then use unchecked fixed-field reads under that proof.
    #[inline]
    pub(crate) unsafe fn wrap_trusted(
        buf: &'a [u8],
        offset: usize,
        acting_version: u16,
        parent_pos: usize,
        parent_block_length: usize,
    ) -> Self {
        let bytes: [u8; 4] = unsafe { read_bytes_unchecked::<4>(buf, offset) };
        let header = GroupSizeEncoding(bytes);
        let count = header.num_in_group() as usize;
        let block_length = header.block_length() as usize;
        let min_fixed = <SnapshotAsksDecoder<
            '_,
            sbe_rt::Detached,
        >>::min_readable_fixed_extent(acting_version);
        Self {
            buf,
            offset: offset + 4,
            count,
            start: offset + 4,
            total: count,
            acting_version,
            acting_block_length: block_length,
            parent_pos,
            parent_block_length,
            _context: core::marker::PhantomData,
        }
    }
    /// Restart iteration from the group's proven start.
    ///
    /// This is the one operation that clears a poisoned group: the
    /// start offset was validated at wrap time, so retrying from there
    /// is sound even after an entry failed.
    #[inline]
    pub fn rewind(&mut self) -> &mut Self {
        self.offset = self.start;
        self.count = self.total;
        self
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotAsksDecoder<'a, C> {
    #[inline]
    pub fn skip_n(&mut self, n: usize) -> Result<(), sbe_rt::DecodeError> {
        if n > self.count {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: n.saturating_mul(self.acting_block_length),
                available: self.count.saturating_mul(self.acting_block_length),
            });
        }
        self.offset += n.saturating_mul(self.acting_block_length);
        self.count -= n;
        Ok(())
    }
    /// Bulk-decode all remaining entries into a caller-owned `Vec`.
    /// Zero-allocation after warm-up — the caller reuses the
    /// destination buffer across messages.
    #[inline]
    pub fn bulk_decode_into(
        &mut self,
        dst: &mut Vec<SnapshotAsksEntry>,
    ) -> Result<usize, sbe_rt::DecodeError> {
        let needed = self
            .count
            .checked_mul(self.acting_block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: usize::MAX,
                available: 0,
            })?;
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let cap = self.count;
        dst.clear();
        dst.reserve(cap);
        for _ in 0..cap {
            let offset = self.offset;
            self.offset += self.acting_block_length;
            dst.push(SnapshotAsksEntry {
                price_level: i64::from_le_bytes(
                    self.buf[offset + 0..offset + 0 + 8].try_into().unwrap(),
                ),
                quantity: i64::from_le_bytes(
                    self.buf[offset + 8..offset + 8 + 8].try_into().unwrap(),
                ),
            });
        }
        self.count = 0;
        Ok(cap)
    }
    /// Bulk-decode all remaining entries into a new `Vec`.
    /// Convenience wrapper around [`Self::bulk_decode_into`].
    /// One bounds check for the whole batch — faster than
    /// iterating with [`Iterator::next`] when materialising
    /// the entire group (DTO construction, snapshots).
    #[inline]
    pub fn bulk_decode(
        &mut self,
    ) -> Result<Vec<SnapshotAsksEntry>, sbe_rt::DecodeError> {
        let mut out = Vec::new();
        self.bulk_decode_into(&mut out)?;
        Ok(out)
    }
}
impl<'a, C: sbe_rt::GroupContext> SnapshotAsksDecoder<'a, C> {
    #[inline]
    pub fn entry_at(
        &self,
        idx: usize,
    ) -> Result<SnapshotAsksEntryDecoder<'a>, sbe_rt::DecodeError> {
        if idx >= self.total {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: idx.saturating_add(1).saturating_mul(self.acting_block_length),
                available: self.total.saturating_mul(self.acting_block_length),
            });
        }
        let byte_offset = idx
            .checked_mul(self.acting_block_length)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: usize::MAX,
                available: self.buf.len().saturating_sub(self.start),
            })?;
        let offset = self
            .start
            .checked_add(byte_offset)
            .ok_or(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: usize::MAX,
                available: self.buf.len().saturating_sub(self.start),
            })?;
        if self.acting_block_length > self.buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "asks",
                needed: self.acting_block_length,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        Ok(unsafe {
            SnapshotAsksEntryDecoder::wrap(
                self.buf,
                offset,
                self.acting_block_length,
                self.acting_version,
            )
        })
    }
}
impl<'a, C: sbe_rt::GroupContext> Iterator for SnapshotAsksDecoder<'a, C> {
    type Item = SnapshotAsksEntryDecoder<'a>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let entry = unsafe {
            SnapshotAsksEntryDecoder::wrap(
                self.buf,
                self.offset,
                self.acting_block_length,
                self.acting_version,
            )
        };
        self.offset += self.acting_block_length;
        self.count -= 1;
        Some(entry)
    }
}
impl<'a, C: sbe_rt::GroupContext> ExactSizeIterator for SnapshotAsksDecoder<'a, C> {
    #[inline]
    fn len(&self) -> usize {
        self.count
    }
}
///Ask price levels
pub struct SnapshotAsksEntryDecoder<'a> {
    buf: &'a [u8],
    offset: usize,
    acting_version: u16,
    acting_block_length: usize,
}
impl<'a> SnapshotAsksEntryDecoder<'a> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Private entry wrap after the group iterator (or equivalent)
    /// has proven the acting fixed block is in-bounds at `offset`.
    ///
    /// # Safety
    /// `offset + max(acting_block_length, ENTRY_BLOCK_LENGTH)` (and any
    /// field offset used by accessors) must not overflow and must be
    /// ≤ `buf.len()`. Fixed-field getters may then use unchecked reads.
    #[inline]
    unsafe fn wrap(
        buf: &'a [u8],
        offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        Self {
            buf,
            offset,
            acting_version,
            acting_block_length,
        }
    }
    #[inline]
    pub fn price_level(&self) -> i64 {
        let offset = self.offset + 0;
        i64::from_le_bytes(unsafe { read_bytes_unchecked::<8>(self.buf, offset) })
    }
    pub const PRICE_LEVEL_ID: u16 = 8;
    pub const PRICE_LEVEL_SINCE_VERSION: u16 = 0;
    pub const PRICE_LEVEL_ENCODING_OFFSET: usize = 0;
    pub const PRICE_LEVEL_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn price_level_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const PRICE_LEVEL_NULL: i64 = -9223372036854775808_i64;
    pub const PRICE_LEVEL_MIN: i64 = -9223372036854775807_i64;
    pub const PRICE_LEVEL_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    pub fn quantity(&self) -> i64 {
        let offset = self.offset + 8;
        i64::from_le_bytes(unsafe { read_bytes_unchecked::<8>(self.buf, offset) })
    }
    pub const QUANTITY_ID: u16 = 9;
    pub const QUANTITY_SINCE_VERSION: u16 = 0;
    pub const QUANTITY_ENCODING_OFFSET: usize = 8;
    pub const QUANTITY_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn quantity_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const QUANTITY_NULL: i64 = -9223372036854775808_i64;
    pub const QUANTITY_MIN: i64 = -9223372036854775807_i64;
    pub const QUANTITY_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    fn tail_offset_0(&self) -> Result<usize, sbe_rt::DecodeError> {
        if self.acting_block_length > self.buf.len().saturating_sub(self.offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "group entry",
                needed: self.acting_block_length,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        Ok(self.offset + self.acting_block_length)
    }
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.acting_block_length
    }
    #[inline]
    pub fn skip(
        buf: &'a [u8],
        offset: usize,
        block_len: usize,
        _acting_version: u16,
    ) -> Result<usize, sbe_rt::DecodeError> {
        if block_len > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: buf.len().saturating_sub(offset),
            });
        }
        Ok(offset + block_len)
    }
}
impl<'a> core::fmt::Display for SnapshotAsksEntryDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{ ")?;
        {
            let v = self.price_level();
            write!(f, "price_level: {:?}", v)?;
        }
        {
            let v = self.quantity();
            write!(f, ", quantity: {:?}", v)?;
        }
        write!(f, " }}")
    }
}
/// Consuming decoder stage — drop without `into_*` / `finish` skips
/// remaining wire tails.
#[must_use = "decoder stage must be advanced with into_*/finish or tails are skipped"]
pub struct SnapshotDecoderAfterBids<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) offset: usize,
    pub(crate) tail_start: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
/// Consuming decoder stage — drop without `into_*` / `finish` skips
/// remaining wire tails.
#[must_use = "decoder stage must be advanced with into_*/finish or tails are skipped"]
pub struct SnapshotDecoderAfterAsks<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) offset: usize,
    pub(crate) tail_start: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
/// Consuming decoder stage — drop without `into_*` / `finish` skips
/// remaining wire tails.
#[must_use = "decoder stage must be advanced with into_*/finish or tails are skipped"]
pub struct SnapshotDecoderComplete<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) offset: usize,
    pub(crate) tail_start: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
impl<'a> SnapshotDecoderAfterBids<'a> {
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
}
impl<'a> SnapshotDecoderComplete<'a> {
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
}
impl<'a> SnapshotDecoder<'a> {
    /// Consume this stage and start decoding the next tail group,
    /// enforcing wire order. The returned group decoder owns the
    /// right to advance to the following stage via `finish()`.
    #[inline]
    pub fn into_bids(
        self,
    ) -> Result<SnapshotBidsDecoder<'a, sbe_rt::Attached>, sbe_rt::DecodeError> {
        let group_start = self.byte_offset() + self.acting_block_length;
        unsafe {
            <SnapshotBidsDecoder<
                'a,
                sbe_rt::Attached,
            >>::wrap_with_parent(
                self.buf,
                group_start,
                self.acting_version,
                self.byte_offset(),
                self.acting_block_length,
            )
        }
    }
}
impl<'a> SnapshotDecoderAfterBids<'a> {
    /// Consume this stage and start decoding the next tail group,
    /// enforcing wire order. The returned group decoder owns the
    /// right to advance to the following stage via `finish()`.
    #[inline]
    pub fn into_asks(
        self,
    ) -> Result<SnapshotAsksDecoder<'a, sbe_rt::Attached>, sbe_rt::DecodeError> {
        let group_start = self.tail_start;
        unsafe {
            <SnapshotAsksDecoder<
                'a,
                sbe_rt::Attached,
            >>::wrap_with_parent(
                self.buf,
                group_start,
                self.acting_version,
                self.offset,
                self.acting_block_length,
            )
        }
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Consume this stage, read the next var-data field, and advance
    /// to the following stage. Wire order is enforced by consumption.
    #[inline]
    pub fn into_symbol(
        self,
    ) -> Result<(&'a [u8], SnapshotDecoderComplete<'a>), sbe_rt::DecodeError> {
        let offset = self.tail_start;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let len = u32::from_le_bytes(bytes) as u64;
        if len > 1073741824 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: "symbol",
                length: len,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            offset,
            4,
            len,
            self.buf.len(),
        )?;
        let data = &self.buf[data_start..data_end];
        let next = SnapshotDecoderComplete {
            buf: self.buf,
            offset: self.offset,
            tail_start: data_end,
            acting_version: self.acting_version,
            acting_block_length: self.acting_block_length,
        };
        Ok((data, next))
    }
    /**Non-consuming variant: read this var-data field as `&[u8]` without advancing or constructing the next stage.

Cheaper than [`Self::into_symbol`] when only the bytes are needed.*/
    #[inline]
    pub fn symbol_slice(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let offset = self.tail_start;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let len = u32::from_le_bytes(bytes) as u64;
        if len > 1073741824 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: "symbol",
                length: len,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            offset,
            4,
            len,
            self.buf.len(),
        )?;
        Ok(&self.buf[data_start..data_end])
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Consume this stage, read the next UTF-8 var-data
    /// field as a validated `&str`, and advance.
    #[inline]
    pub fn into_symbol_as_str(
        self,
    ) -> Result<(&'a str, SnapshotDecoderComplete<'a>), sbe_rt::DecodeError> {
        let (bytes, next) = self.into_symbol()?;
        let s = core::str::from_utf8(bytes)
            .map_err(|e| {
                sbe_rt::DecodeError::InvalidUtf8 {
                    field: "symbol",
                    error: e,
                }
            })?;
        Ok((s, next))
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Consume this stage, read the next text var-data field as
    /// a `&str` without encoding validation, and advance.
    ///
    /// # Safety
    /// The wire bytes must be valid for the schema-declared
    /// character encoding (UTF-8 or ASCII).
    #[inline]
    pub unsafe fn into_symbol_as_str_unchecked(
        self,
    ) -> (&'a str, SnapshotDecoderComplete<'a>) {
        let (bytes, next) = unsafe { self.into_symbol().unwrap() };
        let s = unsafe { core::str::from_utf8_unchecked(bytes) };
        (s, next)
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Fallible scoped var-data accessor. Calls the closure with
    /// the decoded bytes and returns the next stage on success.
    #[inline]
    pub fn try_symbol<E, F>(self, f: F) -> Result<SnapshotDecoderComplete<'a>, E>
    where
        E: From<sbe_rt::DecodeError>,
        F: FnOnce(&[u8]) -> Result<(), E>,
    {
        let (data, next) = self.into_symbol()?;
        f(data)?;
        Ok(next)
    }
}
impl<'a> SnapshotDecoderAfterAsks<'a> {
    /// Consume this stage, decode the var-data field as a nested
    /// SBE message via `AnyMessage::decode_frame`, and advance
    /// to the next stage.
    #[inline]
    pub fn into_symbol_as_message(
        self,
    ) -> Result<(DecodedFrame<'a>, SnapshotDecoderComplete<'a>), sbe_rt::DecodeError> {
        let (data, next) = self.into_symbol()?;
        let frame = AnyMessage::decode_frame(data, 0, data.len())?;
        Ok((frame, next))
    }
    /// Fallible scoped nested-message accessor.
    #[inline]
    pub fn try_symbol_as_message<E, F>(
        self,
        f: F,
    ) -> Result<SnapshotDecoderComplete<'a>, E>
    where
        E: From<sbe_rt::DecodeError>,
        F: FnOnce(DecodedFrame<'a>) -> Result<(), E>,
    {
        let (frame, next) = self.into_symbol_as_message()?;
        f(frame)?;
        Ok(next)
    }
}
impl<'a> SnapshotBidsDecoder<'a, sbe_rt::Attached> {
    /// Scan past any unread entries (including nested tails) in wire
    /// order and return the next decoder stage.
    ///
    /// Only an *attached* group — one reached through its message's
    /// tail — can complete into a message stage. A standalone
    /// [`Self::wrap`] has no parent to return to.
    #[inline]
    pub fn finish(self) -> Result<SnapshotDecoderAfterBids<'a>, sbe_rt::DecodeError> {
        let mut offset = self.offset;
        let mut remaining = self.count;
        let block_len = self.acting_block_length;
        while remaining > 0 {
            offset = SnapshotBidsEntryDecoder::skip(
                self.buf,
                offset,
                block_len,
                self.acting_version,
            )?;
            remaining -= 1;
        }
        Ok(SnapshotDecoderAfterBids {
            buf: self.buf,
            offset: self.parent_pos,
            tail_start: offset,
            acting_version: self.acting_version,
            acting_block_length: self.parent_block_length,
        })
    }
    /// Explicit sequential spelling of "advance past the rest of this group".
    #[inline]
    pub fn skip_remaining(
        self,
    ) -> Result<SnapshotDecoderAfterBids<'a>, sbe_rt::DecodeError> {
        self.finish()
    }
}
impl<'a> SnapshotAsksDecoder<'a, sbe_rt::Attached> {
    /// Scan past any unread entries (including nested tails) in wire
    /// order and return the next decoder stage.
    ///
    /// Only an *attached* group — one reached through its message's
    /// tail — can complete into a message stage. A standalone
    /// [`Self::wrap`] has no parent to return to.
    #[inline]
    pub fn finish(self) -> Result<SnapshotDecoderAfterAsks<'a>, sbe_rt::DecodeError> {
        let mut offset = self.offset;
        let mut remaining = self.count;
        let block_len = self.acting_block_length;
        while remaining > 0 {
            offset = SnapshotAsksEntryDecoder::skip(
                self.buf,
                offset,
                block_len,
                self.acting_version,
            )?;
            remaining -= 1;
        }
        Ok(SnapshotDecoderAfterAsks {
            buf: self.buf,
            offset: self.parent_pos,
            tail_start: offset,
            acting_version: self.acting_version,
            acting_block_length: self.parent_block_length,
        })
    }
    /// Explicit sequential spelling of "advance past the rest of this group".
    #[inline]
    pub fn skip_remaining(
        self,
    ) -> Result<SnapshotDecoderAfterAsks<'a>, sbe_rt::DecodeError> {
        self.finish()
    }
}
impl<'a> SnapshotDecoderComplete<'a> {
    /// Body bytes (excluding the message header; for entries this is the
    /// complete entry bytes).
    #[inline]
    pub fn as_body_bytes(&self) -> &'a [u8] {
        &self.buf[self.offset..self.tail_start]
    }
    /// Complete SBE frame (header + body) for message stages.
    /// For entry stages (`HEADER_LENGTH == 0`) this equals [`Self::as_body_bytes`].
    #[inline]
    pub fn as_bytes_with_header(&self) -> &'a [u8] {
        &self.buf[self.offset - 8..self.tail_start]
    }
    /// Body length (excluding header).
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.tail_start - self.offset
    }
    /// Total message length including the schema-declared header.
    /// Pure arithmetic: body length + `HEADER_LENGTH`.
    #[inline]
    pub fn encoded_length_with_header(&self) -> usize {
        self.tail_start - self.offset + 8
    }
    /// Bytes after this message/entry.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        &self.buf[self.tail_start..]
    }
}
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Materialise with [`Self::try_from_decoder`] (from a decoder).
/// This is an inherent method, not `TryFrom`/`From`: conversion is never
/// infallible (groups, var-data, converters).
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotBidsEntryDomain {
    pub price_level: i64,
    pub quantity: i64,
}
impl SnapshotBidsEntryDomain {
    /// Fallible conversion from a flyweight decoder.
    ///
    /// Propagates decode errors from malformed group entries and var-data
    /// instead of panicking. Prefer this over `From`/`TryFrom`.
    #[inline]
    pub fn try_from_decoder(
        dec: SnapshotBidsEntryDecoder<'_>,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Ok(Self {
            price_level: dec.price_level(),
            quantity: dec.quantity(),
        })
    }
}
impl SnapshotBidsEntryDomain {
    #[inline]
    pub fn encode_into<'a>(
        &self,
        enc: &mut SnapshotBidsEntryEncoder<'a>,
    ) -> Result<(), sbe_rt::EncodeError> {
        {
            let __v = self.price_level as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "price_level",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.price_level(self.price_level);
        {
            let __v = self.quantity as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "quantity",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.quantity(self.quantity);
        Ok(())
    }
    /// Compute this entry's contribution to the total encoded length
    /// (entry block + nested groups + entry var-data).
    #[inline]
    pub fn length_contribution(&self) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 16;
        Ok(len)
    }
}
impl SnapshotBidsEntryDomain {
    /// Convert to the wire entry struct for bulk encoding.
    #[inline]
    pub fn to_wire_entry(&self) -> SnapshotBidsEntry {
        SnapshotBidsEntry {
            price_level: self.price_level,
            quantity: self.quantity,
        }
    }
}
impl<'a> SnapshotBidsEncoder<'a> {
    /// Encode flat domain entries with one complete-region bounds check
    /// and no temporary wire-entry allocation.
    #[inline]
    pub fn bulk_add_domain(
        &mut self,
        entries: &[SnapshotBidsEntryDomain],
    ) -> Result<(), sbe_rt::EncodeError> {
        self.bulk_add_with(
            entries,
            |entry, slot| {
                {
                    let __v = entry.price_level as i128;
                    if __v < -9223372036854775807 || __v > 9223372036854775807 {
                        return Err(sbe_rt::EncodeError::ValueOutOfRange {
                            field: "price_level",
                            min: -9223372036854775807,
                            max: 9223372036854775807,
                            actual: __v,
                        });
                    }
                }
                {
                    let __v = entry.quantity as i128;
                    if __v < -9223372036854775807 || __v > 9223372036854775807 {
                        return Err(sbe_rt::EncodeError::ValueOutOfRange {
                            field: "quantity",
                            min: -9223372036854775807,
                            max: 9223372036854775807,
                            actual: __v,
                        });
                    }
                }
                slot[0..0 + 8].copy_from_slice(&entry.price_level.to_le_bytes());
                slot[8..8 + 8].copy_from_slice(&entry.quantity.to_le_bytes());
                Ok(())
            },
        )
    }
}
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Materialise with [`Self::try_from_decoder`] (from a decoder).
/// This is an inherent method, not `TryFrom`/`From`: conversion is never
/// infallible (groups, var-data, converters).
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotAsksEntryDomain {
    pub price_level: i64,
    pub quantity: i64,
}
impl SnapshotAsksEntryDomain {
    /// Fallible conversion from a flyweight decoder.
    ///
    /// Propagates decode errors from malformed group entries and var-data
    /// instead of panicking. Prefer this over `From`/`TryFrom`.
    #[inline]
    pub fn try_from_decoder(
        dec: SnapshotAsksEntryDecoder<'_>,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Ok(Self {
            price_level: dec.price_level(),
            quantity: dec.quantity(),
        })
    }
}
impl SnapshotAsksEntryDomain {
    #[inline]
    pub fn encode_into<'a>(
        &self,
        enc: &mut SnapshotAsksEntryEncoder<'a>,
    ) -> Result<(), sbe_rt::EncodeError> {
        {
            let __v = self.price_level as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "price_level",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.price_level(self.price_level);
        {
            let __v = self.quantity as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "quantity",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.quantity(self.quantity);
        Ok(())
    }
    /// Compute this entry's contribution to the total encoded length
    /// (entry block + nested groups + entry var-data).
    #[inline]
    pub fn length_contribution(&self) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 16;
        Ok(len)
    }
}
impl SnapshotAsksEntryDomain {
    /// Convert to the wire entry struct for bulk encoding.
    #[inline]
    pub fn to_wire_entry(&self) -> SnapshotAsksEntry {
        SnapshotAsksEntry {
            price_level: self.price_level,
            quantity: self.quantity,
        }
    }
}
impl<'a> SnapshotAsksEncoder<'a> {
    /// Encode flat domain entries with one complete-region bounds check
    /// and no temporary wire-entry allocation.
    #[inline]
    pub fn bulk_add_domain(
        &mut self,
        entries: &[SnapshotAsksEntryDomain],
    ) -> Result<(), sbe_rt::EncodeError> {
        self.bulk_add_with(
            entries,
            |entry, slot| {
                {
                    let __v = entry.price_level as i128;
                    if __v < -9223372036854775807 || __v > 9223372036854775807 {
                        return Err(sbe_rt::EncodeError::ValueOutOfRange {
                            field: "price_level",
                            min: -9223372036854775807,
                            max: 9223372036854775807,
                            actual: __v,
                        });
                    }
                }
                {
                    let __v = entry.quantity as i128;
                    if __v < -9223372036854775807 || __v > 9223372036854775807 {
                        return Err(sbe_rt::EncodeError::ValueOutOfRange {
                            field: "quantity",
                            min: -9223372036854775807,
                            max: 9223372036854775807,
                            actual: __v,
                        });
                    }
                }
                slot[0..0 + 8].copy_from_slice(&entry.price_level.to_le_bytes());
                slot[8..8 + 8].copy_from_slice(&entry.quantity.to_le_bytes());
                Ok(())
            },
        )
    }
}
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Materialise with [`Self::try_from_decoder`] (from a decoder) or
/// [`Self::try_from_slice_with_header`] (from framed bytes).
/// These are inherent methods, not `TryFrom`/`From`: there are two fallible
/// sources, and conversion is never infallible (groups, var-data, converters).
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotDomain {
    pub received_at: chrono::DateTime<chrono::Utc>,
    pub exchange_timestamp: chrono::DateTime<chrono::Utc>,
    pub bids: Vec<SnapshotBidsEntryDomain>,
    pub asks: Vec<SnapshotAsksEntryDomain>,
    pub symbol: String,
}
impl SnapshotDomain {
    /// Fallible conversion from a flyweight decoder.
    ///
    /// Propagates decode errors from malformed group entries and var-data
    /// instead of panicking. Prefer this over `From`/`TryFrom`: the companion
    /// entry point is [`Self::try_from_slice_with_header`] (when generated),
    /// and named methods make the two sources unambiguous.
    #[inline]
    pub fn try_from_decoder(
        dec: SnapshotDecoder<'_>,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Ok(Self {
            received_at: dec
                .try_received_at()
                .map_err(|_| {
                    sbe_rt::DecodeError::DomainConversionFailed {
                        field: "received_at",
                        reason: "try_* conversion rejected wire value",
                    }
                })?,
            exchange_timestamp: dec
                .try_exchange_timestamp()
                .map_err(|_| {
                    sbe_rt::DecodeError::DomainConversionFailed {
                        field: "exchange_timestamp",
                        reason: "try_* conversion rejected wire value",
                    }
                })?,
            bids: dec
                .bids()
                .map(|g| {
                    g.map(SnapshotBidsEntryDomain::try_from_decoder)
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or_else(|e| Err(e))?,
            asks: dec
                .asks()
                .map(|g| {
                    g.map(SnapshotAsksEntryDomain::try_from_decoder)
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or_else(|e| Err(e))?,
            symbol: match dec.symbol() {
                Ok(data) => {
                    match core::str::from_utf8(data) {
                        Ok(s) => s.to_owned(),
                        Err(error) => {
                            return Err(sbe_rt::DecodeError::InvalidUtf8 {
                                field: "symbol",
                                error,
                            });
                        }
                    }
                }
                Err(e) => return Err(e),
            },
        })
    }
    /// Decode from a framed byte slice: validate the message header, then
    /// materialise the full domain object.
    ///
    /// Distinct from [`Self::try_from_decoder`]: this path owns header
    /// validation + offset; that path starts from an already-wrapped decoder.
    /// Named methods (not `TryFrom`/`From`) keep the two sources obvious.
    #[inline]
    pub fn try_from_slice_with_header(
        buf: &[u8],
        message_offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Self::try_from_decoder(SnapshotDecoder::decode(buf, message_offset)?)
    }
}
impl SnapshotDomain {
    #[inline]
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, sbe_rt::EncodeError> {
        let mut enc = SnapshotEncoder::try_wrap_and_apply_header(buf, 0)?;
        enc.try_received_at(self.received_at)
            .map_err(|_| {
                sbe_rt::EncodeError::DomainConversionFailed {
                    field: "received_at",
                    reason: "try_* conversion rejected domain value",
                }
            })?;
        enc.try_exchange_timestamp(self.exchange_timestamp)
            .map_err(|_| {
                sbe_rt::EncodeError::DomainConversionFailed {
                    field: "exchange_timestamp",
                    reason: "try_* conversion rejected domain value",
                }
            })?;
        let count = <u16>::try_from(self.bids.len())
            .map_err(|_| {
                sbe_rt::EncodeError::ValueOutOfRange {
                    field: "group count",
                    min: 0,
                    max: u16::MAX as i128,
                    actual: self.bids.len() as i128,
                }
            })?;
        let enc = enc.bids(count, |g| g.bulk_add_domain(&self.bids))?;
        let count = <u16>::try_from(self.asks.len())
            .map_err(|_| {
                sbe_rt::EncodeError::ValueOutOfRange {
                    field: "group count",
                    min: 0,
                    max: u16::MAX as i128,
                    actual: self.asks.len() as i128,
                }
            })?;
        let enc = enc.asks(count, |g| g.bulk_add_domain(&self.asks))?;
        let enc = enc.symbol(self.symbol.as_bytes())?;
        Ok(enc.encoded_length() + SnapshotEncoder::HEADER_LENGTH)
    }
    /// Compute the exact SBE message body length from this domain object.
    /// Matches the length returned by [`Self::encode`].
    #[inline]
    pub fn encoded_length(&self) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 16;
        len = len.checked_add(4).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        for entry in &self.bids {
            len = len
                .checked_add(entry.length_contribution()?)
                .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        }
        len = len.checked_add(4).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        for entry in &self.asks {
            len = len
                .checked_add(entry.length_contribution()?)
                .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        }
        if self.symbol.len() > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: self.symbol.len(),
            });
        }
        len = len.checked_add(4).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        len = len
            .checked_add(self.symbol.len())
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        Ok(len)
    }
    /// Compute the exact SBE message length including the message header.
    /// Matches `encode()` return value for non-fixed messages.
    #[inline]
    pub fn encoded_length_with_header(&self) -> Result<usize, sbe_rt::EncodeError> {
        Ok(self.encoded_length()? + SnapshotEncoder::HEADER_LENGTH)
    }
}
///A full snapshot of the orderbook
#[doc = concat!(
    "Encoder stage `", "SnapshotEncoder", "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct SnapshotEncoder<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for SnapshotEncoder<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "SnapshotEncoder"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for SnapshotEncoder<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("SnapshotEncoder")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
#[doc = concat!(
    "Encoder stage `", "SnapshotAfterBids", "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct SnapshotAfterBids<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for SnapshotAfterBids<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "SnapshotAfterBids"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for SnapshotAfterBids<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("SnapshotAfterBids")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
#[doc = concat!(
    "Encoder stage `", "SnapshotAfterAsks", "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct SnapshotAfterAsks<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for SnapshotAfterAsks<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "SnapshotAfterAsks"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for SnapshotAfterAsks<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("SnapshotAfterAsks")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
#[doc = concat!(
    "Encoder stage `", "SnapshotComplete", "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct SnapshotComplete<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for SnapshotComplete<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "SnapshotComplete"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for SnapshotComplete<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match SnapshotDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("SnapshotComplete")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
/// Complete set of latest-version fixed fields for this message.
/// Required fields are concrete values; optional/versioned fields
/// are `Option<T>`. Constants are excluded.
///
/// This struct is **intentionally exhaustive** (not
/// `#[non_exhaustive]`): when the schema adds a fixed field, every
/// `fixed(&…)` call site must be updated. That is a feature — schema
/// changes surface as compile errors rather than silent defaults.
#[derive(Debug, Clone)]
pub struct SnapshotFixedFields {
    pub received_at: UTCTimestampMicrosEncoding,
    pub exchange_timestamp: UTCTimestampMicrosEncoding,
}
///Raw fixed-field writer. Individual field setters are available only on this writer. When done, embed the fields in a [`SnapshotFixedFields`] and call the encoder's `fixed()`.
#[must_use = "raw fixed writer must be embedded in FixedFields"]
pub struct SnapshotRawFixedWriter<'a> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
}
impl<'a> SnapshotEncoder<'a> {
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 1;
    pub const BLOCK_LENGTH: usize = 16;
    const _BLOCK_LEN: () = assert!(Self::BLOCK_LENGTH == 16);
    /// Schema-declared message header size in bytes.
    pub const HEADER_LENGTH: usize = 8;
    pub const HEADER_TEMPLATE: [u8; 8] = [16, 0, 1, 0, 1, 0, 2, 0];
    const _HEADER_TEMPLATE_LEN: () = assert!(Self::HEADER_TEMPLATE.len() == 8);
    /// Cold error constructor — never inlined into the hot path.
    #[cold]
    #[inline(never)]
    fn buffer_too_short(
        buf: &[u8],
        offset: usize,
        needed: usize,
    ) -> sbe_rt::EncodeError {
        sbe_rt::EncodeError::BufferTooShort {
            field: "message header+body",
            needed,
            available: buf.len().saturating_sub(offset),
        }
    }
    /// Wrap a mutable buffer for encoding with one bounds/overflow check.
    /// Does **not** write the message header (`HeaderAbsent`).
    ///
    /// `msg_offset` is the **message start** (first byte of the SBE frame),
    /// not the body. sbe-tool Rust `wrap` takes the body offset instead.
    ///
    /// Prefer [`Self::wrap_and_apply_header`] when encoding a full frame.
    #[inline]
    pub fn try_wrap(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> Result<SnapshotEncoder<'a, sbe_rt::HeaderAbsent>, sbe_rt::EncodeError> {
        if 24 > buf.len().saturating_sub(msg_offset) {
            return Err(Self::buffer_too_short(buf, msg_offset, 24));
        }
        Ok(unsafe { Self::wrap_unchecked(buf, msg_offset) })
    }
    /// Trusted body-only wrap. Proves header + fixed-body extent then
    /// constructs; **panics** if the buffer is too short. Field setters
    /// use unchecked writes justified by that proof.
    ///
    /// Prefer [`Self::try_wrap`] at untrusted boundaries.
    #[inline]
    pub fn wrap(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> SnapshotEncoder<'a, sbe_rt::HeaderAbsent> {
        if 24 > buf.len().saturating_sub(msg_offset) {
            panic!("{}", Self::buffer_too_short(buf, msg_offset, 24));
        }
        unsafe { Self::wrap_unchecked(buf, msg_offset) }
    }
    /// Zero-check body-only wrap — raw pointer ops, **UB** on OOB.
    /// Only for proven-tight hot loops where the panic machinery is
    /// measurable in the critical path.
    ///
    /// # Safety
    /// `msg_offset + HEADER_LENGTH + BLOCK_LENGTH` must not overflow
    /// and must be ≤ `buf.len()` for the lifetime of the encoder.
    #[inline]
    pub unsafe fn wrap_unchecked(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> SnapshotEncoder<'a, sbe_rt::HeaderAbsent> {
        let body_offset = msg_offset + 8;
        SnapshotEncoder {
            buf,
            msg_offset,
            offset: body_offset + 16,
            _header: core::marker::PhantomData,
        }
    }
    /// Wrap a mutable buffer, write the header, with one bounds/overflow check.
    /// `offset` is the **message start** (see [`Self::wrap`]).
    ///
    /// Optional-field nullification is **not** applied by default — call
    /// `apply_nulls()` if you want null sentinels.
    #[inline]
    pub fn try_wrap_and_apply_header(
        buf: &'a mut [u8],
        offset: usize,
    ) -> Result<SnapshotEncoder<'a, sbe_rt::HeaderPresent>, sbe_rt::EncodeError> {
        if 24 > buf.len().saturating_sub(offset) {
            return Err(Self::buffer_too_short(buf, offset, 24));
        }
        Ok(unsafe { Self::wrap_and_apply_header_unchecked(buf, offset) })
    }
    /// Trusted full-frame wrap + header. Proves header + fixed-body extent
    /// then writes the header; **panics** if the buffer is too short.
    /// Field setters use unchecked writes justified by that proof.
    ///
    /// Prefer [`Self::try_wrap_and_apply_header`] at untrusted boundaries.
    /// Call [`Self::wrap_and_apply_header_unchecked`] only with a proven
    /// extent when even panic machinery must be avoided.
    #[inline]
    pub fn wrap_and_apply_header(
        buf: &'a mut [u8],
        offset: usize,
    ) -> SnapshotEncoder<'a, sbe_rt::HeaderPresent> {
        if 24 > buf.len().saturating_sub(offset) {
            panic!("{}", Self::buffer_too_short(buf, offset, 24));
        }
        unsafe { Self::wrap_and_apply_header_unchecked(buf, offset) }
    }
    /// Zero-check full-frame wrap + header — `copy_nonoverlapping`, **UB**
    /// on OOB. Only for proven-tight hot loops.
    ///
    /// # Safety
    /// `offset + HEADER_LENGTH + BLOCK_LENGTH` must not overflow and must be
    /// ≤ `buf.len()` for the lifetime of the encoder.
    #[inline]
    pub unsafe fn wrap_and_apply_header_unchecked(
        buf: &'a mut [u8],
        offset: usize,
    ) -> SnapshotEncoder<'a, sbe_rt::HeaderPresent> {
        unsafe {
            core::ptr::copy_nonoverlapping(
                Self::HEADER_TEMPLATE.as_ptr(),
                buf.as_mut_ptr().add(offset),
                8,
            );
        }
        let body_offset = offset + 8;
        SnapshotEncoder {
            buf,
            msg_offset: offset,
            offset: body_offset + 16,
            _header: core::marker::PhantomData,
        }
    }
    pub const RECEIVED_AT_ID: u16 = 1;
    pub const RECEIVED_AT_SINCE_VERSION: u16 = 0;
    pub const RECEIVED_AT_ENCODING_OFFSET: usize = 0;
    pub const RECEIVED_AT_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn received_at_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const EXCHANGE_TIMESTAMP_ID: u16 = 2;
    pub const EXCHANGE_TIMESTAMP_SINCE_VERSION: u16 = 0;
    pub const EXCHANGE_TIMESTAMP_ENCODING_OFFSET: usize = 8;
    pub const EXCHANGE_TIMESTAMP_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn exchange_timestamp_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    /// Compute the exact SBE message body length before encoding.
    /// Parameters: one `usize` per group (entry count) and one `usize`
    /// per var-data field (byte length).
    #[inline]
    pub const fn compute_encoded_length(
        bids_count: usize,
        asks_count: usize,
        symbol_len: usize,
    ) -> usize {
        let mut len = 16;
        len += 4 + bids_count * 16;
        len += 4 + asks_count * 16;
        len += 4 + symbol_len;
        len
    }
    /// Compute the exact SBE message length including the schema-declared
    /// message header.
    #[inline]
    pub const fn compute_encoded_length_with_message_header(
        bids_count: usize,
        asks_count: usize,
        symbol_len: usize,
    ) -> usize {
        8usize + Self::compute_encoded_length(bids_count, asks_count, symbol_len)
    }
    /// Short alias for [`Self::compute_encoded_length_with_message_header`].
    #[inline]
    pub const fn compute_length_with_header(
        bids_count: usize,
        asks_count: usize,
        symbol_len: usize,
    ) -> usize {
        Self::compute_encoded_length_with_message_header(
            bids_count,
            asks_count,
            symbol_len,
        )
    }
    /// Compute the exact SBE message body length with checked arithmetic.
    /// Group counts use the wire type (`u16` or `u8`); var-data lengths
    /// use `usize`.
    #[inline]
    pub fn try_compute_encoded_length(
        bids_count: u16,
        asks_count: u16,
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 16;
        let entries_len = match (16 as usize).checked_mul(bids_count as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(4 as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(entries_len) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        let entries_len = match (16 as usize).checked_mul(asks_count as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(4 as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(entries_len) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        if symbol_len > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: symbol_len,
            });
        }
        len = match len.checked_add(4 as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(symbol_len) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        Ok(len)
    }
    /// Compute the exact SBE message length including the header, with
    /// checked arithmetic.
    #[inline]
    pub fn try_compute_encoded_length_with_header(
        bids_count: u16,
        asks_count: u16,
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        let body = Self::try_compute_encoded_length(bids_count, asks_count, symbol_len)?;
        body.checked_add(8usize).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)
    }
    /// Short alias for [`Self::try_compute_encoded_length_with_header`].
    #[inline]
    pub fn try_compute_length_with_header(
        bids_count: u16,
        asks_count: u16,
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        Self::try_compute_encoded_length_with_header(bids_count, asks_count, symbol_len)
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotEncoder<'a, H> {
    #[inline]
    pub fn received_at_wire(&mut self, val: UTCTimestampMicrosEncoding) -> &mut Self {
        let offset = self.msg_offset + 8;
        self.buf[offset..offset + 8].copy_from_slice(&val.0);
        self
    }
    #[inline]
    pub fn exchange_timestamp_wire(
        &mut self,
        val: UTCTimestampMicrosEncoding,
    ) -> &mut Self {
        let offset = self.msg_offset + 16;
        self.buf[offset..offset + 8].copy_from_slice(&val.0);
        self
    }
    /**Set all fixed fields at once from a [`SnapshotFixedFields`] value.

Required fields are always written; optional fields are written when `Some`. Returns the encoder for tail methods.*/
    #[inline]
    #[must_use]
    pub fn fixed(mut self, fixed: &SnapshotFixedFields) -> Self {
        self.received_at_wire(fixed.received_at);
        self.exchange_timestamp_wire(fixed.exchange_timestamp);
        self
    }
    ///Return a dedicated raw fixed-field writer. All individual field setters are available on the writer. To advance to tail stages, collect the values into a [`SnapshotFixedFields`] and call `fixed()`.
    #[inline]
    #[must_use]
    pub fn raw_fixed(self) -> SnapshotRawFixedWriter<'a> {
        let body_start = self.msg_offset + 8;
        SnapshotRawFixedWriter {
            buf: &mut self.buf[body_start..],
            msg_offset: 0,
            offset: self.offset - body_start,
        }
    }
}
/// Buffer-placement metadata. Holds a reference to the parent encoder
/// — zero-copy. Utility methods live here so no schema field can
/// collide with them.
#[derive(Clone, Copy)]
pub struct SnapshotEncoderMetadata<
    'm,
    'a,
    H: sbe_rt::HeaderState = sbe_rt::HeaderPresent,
> {
    encoder: &'m SnapshotEncoder<'a, H>,
}
impl<'m, 'a, H: sbe_rt::HeaderState> SnapshotEncoderMetadata<'m, 'a, H> {
    /// Fixed-block body bytes only (groups/var-data not yet written).
    /// For a complete frame use the terminal stage's
    /// `as_bytes_with_header`.
    #[inline]
    pub fn as_fixed_body_bytes(&self) -> &[u8] {
        &self.encoder.buf[self.encoder.msg_offset + 8..self.encoder.offset]
    }
    /// Header + fixed block only — **not** a complete SBE message when
    /// groups or var-data remain. Prefer the complete stage's
    /// `as_bytes_with_header`.
    #[inline]
    pub fn as_fixed_region_with_header(&self) -> &[u8] {
        &self.encoder.buf[self.encoder.msg_offset..self.encoder.offset]
    }
    /// Absolute offset of this message within the original buffer
    /// (the `msg_offset` argument passed to `wrap`).
    #[inline]
    pub const fn message_offset(&self) -> usize {
        self.encoder.msg_offset
    }
    /// Absolute current write cursor within the original buffer.
    #[inline]
    pub const fn limit(&self) -> usize {
        self.encoder.offset
    }
    /// The complete original buffer this encoder wraps.
    #[inline]
    pub const fn buffer(&self) -> &[u8] {
        self.encoder.buf
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotEncoder<'a, H> {
    /// Metadata accessor: buffer positions, wire-frame boundaries.
    /// Returns a zero-copy reference to the parent encoder.
    /// All utility methods are scoped here so no schema field name
    /// can collide with them.
    #[inline]
    pub fn get_metadata(&self) -> SnapshotEncoderMetadata<'_, 'a, H> {
        SnapshotEncoderMetadata {
            encoder: self,
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotEncoder<'a, H> {
    /// Encode this group with a known count up front.
    /// Closures return [`sbe_rt::GroupResult`]
    /// (`Result<(), EncodeError>`); `?` works — there is no
    /// separate `try_*` method name.
    #[inline]
    #[must_use]
    pub fn bids<F>(
        mut self,
        count: u16,
        f: F,
    ) -> Result<SnapshotAfterBids<'a, H>, sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotBidsEncoder<'a>) -> sbe_rt::GroupResult,
    {
        if self.offset + 4 > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(bids),
                    needed: 4,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        self.buf[self.offset..self.offset + 4]
            .copy_from_slice(&SnapshotBidsEncoder::GROUP_DIM_TEMPLATE);
        self.buf[self.offset + 2..self.offset + 2 + 2]
            .copy_from_slice(&count.to_le_bytes());
        let mut group = SnapshotBidsEncoder::wrap(self.buf, self.offset + 4, count);
        f(&mut group)?;
        let written = group.written();
        if written != count {
            return Err(sbe_rt::EncodeError::GroupCountMismatch {
                declared: count as u32,
                actual: written as u32,
            });
        }
        Ok(SnapshotAfterBids {
            buf: group.buf,
            msg_offset: self.msg_offset,
            offset: group.offset,
            _header: core::marker::PhantomData,
        })
    }
    /**Encode this group without knowing the count up front.

The dimension header is written with a zero placeholder; after the closure returns, the actual entry count is back-patched into the header. No `GroupFull` check — overflow is the caller's responsibility.

Prefer [`Self::bids`] when the count is known at compile time or from a small input.*/
    #[inline]
    #[must_use]
    pub fn bids_unknown_size<F>(
        mut self,
        f: F,
    ) -> Result<SnapshotAfterBids<'a, H>, sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotBidsEncoder<'a>) -> sbe_rt::GroupResult,
    {
        if self.offset + 4 > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(bids),
                    needed: 4,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        self.buf[self.offset..self.offset + 4]
            .copy_from_slice(&SnapshotBidsEncoder::GROUP_DIM_TEMPLATE);
        let count_offset = self.offset + 2;
        self.buf[count_offset..count_offset + 2].fill(0);
        let (buf, offset, actual) = {
            let mut group = SnapshotBidsEncoder::wrap(
                self.buf,
                self.offset + 4,
                u16::MAX,
            );
            f(&mut group)?;
            let n = group.written();
            (group.buf, group.offset, n)
        };
        buf[count_offset..count_offset + 2].copy_from_slice(&actual.to_le_bytes());
        Ok(SnapshotAfterBids {
            buf,
            msg_offset: self.msg_offset,
            offset,
            _header: core::marker::PhantomData,
        })
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotAfterBids<'a, H> {
    /// Encode this group with a known count up front.
    /// Closures return [`sbe_rt::GroupResult`]
    /// (`Result<(), EncodeError>`); `?` works — there is no
    /// separate `try_*` method name.
    #[inline]
    #[must_use]
    pub fn asks<F>(
        mut self,
        count: u16,
        f: F,
    ) -> Result<SnapshotAfterAsks<'a, H>, sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotAsksEncoder<'a>) -> sbe_rt::GroupResult,
    {
        if self.offset + 4 > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(asks),
                    needed: 4,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        self.buf[self.offset..self.offset + 4]
            .copy_from_slice(&SnapshotAsksEncoder::GROUP_DIM_TEMPLATE);
        self.buf[self.offset + 2..self.offset + 2 + 2]
            .copy_from_slice(&count.to_le_bytes());
        let mut group = SnapshotAsksEncoder::wrap(self.buf, self.offset + 4, count);
        f(&mut group)?;
        let written = group.written();
        if written != count {
            return Err(sbe_rt::EncodeError::GroupCountMismatch {
                declared: count as u32,
                actual: written as u32,
            });
        }
        Ok(SnapshotAfterAsks {
            buf: group.buf,
            msg_offset: self.msg_offset,
            offset: group.offset,
            _header: core::marker::PhantomData,
        })
    }
    /**Encode this group without knowing the count up front.

The dimension header is written with a zero placeholder; after the closure returns, the actual entry count is back-patched into the header. No `GroupFull` check — overflow is the caller's responsibility.

Prefer [`Self::asks`] when the count is known at compile time or from a small input.*/
    #[inline]
    #[must_use]
    pub fn asks_unknown_size<F>(
        mut self,
        f: F,
    ) -> Result<SnapshotAfterAsks<'a, H>, sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotAsksEncoder<'a>) -> sbe_rt::GroupResult,
    {
        if self.offset + 4 > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(asks),
                    needed: 4,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        self.buf[self.offset..self.offset + 4]
            .copy_from_slice(&SnapshotAsksEncoder::GROUP_DIM_TEMPLATE);
        let count_offset = self.offset + 2;
        self.buf[count_offset..count_offset + 2].fill(0);
        let (buf, offset, actual) = {
            let mut group = SnapshotAsksEncoder::wrap(
                self.buf,
                self.offset + 4,
                u16::MAX,
            );
            f(&mut group)?;
            let n = group.written();
            (group.buf, group.offset, n)
        };
        buf[count_offset..count_offset + 2].copy_from_slice(&actual.to_le_bytes());
        Ok(SnapshotAfterAsks {
            buf,
            msg_offset: self.msg_offset,
            offset,
            _header: core::marker::PhantomData,
        })
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotAfterAsks<'a, H> {
    #[inline]
    #[must_use]
    pub fn symbol(
        mut self,
        data: &[u8],
    ) -> Result<SnapshotComplete<'a, H>, sbe_rt::EncodeError> {
        if data.len() > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: data.len(),
            });
        }
        let needed = 4 + data.len();
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: stringify!(symbol),
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let wire_length = <u32>::try_from(data.len())
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: data.len(),
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        self.buf[start..start + data.len()].copy_from_slice(data);
        Ok(SnapshotComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + data.len(),
            _header: core::marker::PhantomData,
        })
    }
    #[inline]
    #[must_use]
    pub fn symbol_unchecked(
        mut self,
        data: &[u8],
    ) -> Result<SnapshotComplete<'a, H>, sbe_rt::EncodeError> {
        let needed = 4 + data.len();
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: stringify!(symbol),
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let wire_length = <u32>::try_from(data.len())
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: data.len(),
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        self.buf[start..start + data.len()].copy_from_slice(data);
        Ok(SnapshotComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + data.len(),
            _header: core::marker::PhantomData,
        })
    }
    /// Lend exactly `exact_len` bytes of the var-data region
    /// to a closure for nested-message encoding. Zero-copy:
    /// the closure writes directly into the outer buffer.
    ///
    /// Canonical nested-SBE pattern (AppMessage → L2Book):
    /// ```text
    /// let inner_len = InnerEncoder::compute_length_with_header(...);
    /// after.payload_with(inner_len, |payload| {
    ///     let len = InnerEncoder::wrap_and_apply_header(payload, 0)?
    ///         .field(value)
    ///         // continue the single encoder chain through all tail stages
    ///         .encoded_length_with_header();
    ///     debug_assert_eq!(len, inner_len);
    ///     Ok(())
    /// })?;
    /// ```
    /// Returns the next stage on success; on failure the
    /// caller error propagates unchanged and no partial
    /// data is published.
    #[inline]
    #[must_use]
    pub fn symbol_with<E, F>(
        mut self,
        exact_len: usize,
        f: F,
    ) -> Result<SnapshotComplete<'a, H>, E>
    where
        E: From<sbe_rt::EncodeError>,
        F: FnOnce(&mut [u8]) -> Result<(), E>,
    {
        if exact_len > 1073741824 {
            return Err(
                sbe_rt::EncodeError::VarDataTooLong {
                    field: "symbol",
                    max_length: 1073741824,
                    actual: exact_len,
                }
                    .into(),
            );
        }
        let needed = 4 + exact_len;
        if self.offset + needed > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(symbol),
                    needed,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        let wire_length = <u32>::try_from(exact_len)
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: exact_len,
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        f(&mut self.buf[start..start + exact_len])?;
        Ok(SnapshotComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + exact_len,
            _header: core::marker::PhantomData,
        })
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotComplete<'a, H> {
    /// SBE message body bytes (excluding the message header).
    #[inline]
    pub fn as_body_bytes(&self) -> &[u8] {
        let body_start = self.msg_offset + 8;
        &self.buf[body_start..self.offset]
    }
    /// SBE message body length (excluding the message header).
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.offset - self.msg_offset - 8
    }
    /// Total SBE message length including the header region.
    /// Pure arithmetic — available for body-only wraps too.
    #[inline]
    pub fn encoded_length_with_header(&self) -> usize {
        self.offset - self.msg_offset
    }
    /// Unwritten region after this message's write cursor to the end of
    /// the original buffer. Use for multi-message packing, e.g.
    /// `NextEncoder::wrap_and_apply_header(remaining, 0)`. This is **not**
    /// the payload of the current message — for the absolute write
    /// cursor while keeping the encoder alive, use
    /// `get_metadata().limit()`.
    #[inline]
    pub fn into_remaining_mut(self) -> &'a mut [u8] {
        &mut self.buf[self.offset..]
    }
}
impl<'a> SnapshotComplete<'a, sbe_rt::HeaderPresent> {
    /// Header-inclusive bytes. Only available when the encoder was
    /// constructed via `wrap_and_apply_header` (not raw `wrap`).
    #[inline]
    pub fn as_bytes_with_header(&self) -> &[u8] {
        &self.buf[self.msg_offset..self.offset]
    }
}
impl<'a> __sbe_message_sealed::Sealed for SnapshotEncoder<'a> {}
impl<'a> sbe_rt::SbeMessage for SnapshotEncoder<'a> {
    const TEMPLATE_ID: u16 = 1;
    const BLOCK_LENGTH: usize = 16;
    const SCHEMA_ID: u16 = 1;
    const SCHEMA_VERSION: u16 = 2;
}
#[doc = concat!(
    "Encoder for the `", stringify!(SnapshotBidsEncoder),
    "` group — call `add()` to write entries."
)]
#[must_use = "group encoder must call add() to write entries"]
pub struct SnapshotBidsEncoder<'a> {
    buf: &'a mut [u8],
    offset: usize,
    count: u16,
    written: u16,
}
impl<'a> SnapshotBidsEncoder<'a> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    pub const GROUP_DIM_TEMPLATE: [u8; 4] = [16, 0, 0, 0];
    const _GROUP_DIM_TEMPLATE_LEN: () = assert!(Self::GROUP_DIM_TEMPLATE.len() == 4);
    #[inline]
    pub fn wrap(buf: &'a mut [u8], offset: usize, count: u16) -> Self {
        Self {
            buf,
            offset,
            count,
            written: 0,
        }
    }
    /// Write one group entry. The closure may return `()` or
    /// `Result<(), sbe_rt::EncodeError>` (both satisfy
    /// [`sbe_rt::GroupResult`]), so `?` works without a `try_add`.
    #[inline]
    #[must_use]
    pub fn add<'b, F>(&'b mut self, f: F) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotBidsEntryEncoder<'b>) -> sbe_rt::GroupResult,
    {
        if self.written >= self.count {
            return Err(
                sbe_rt::EncodeError::GroupFull {
                    declared: self.count as u32,
                    attempted: self.written as u32 + 1,
                }
                    .into(),
            );
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: "group entry",
                    needed: block_len,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        {
            let __buf: &'a mut [u8] = unsafe { &mut *(self.buf as *mut [u8]) };
            let mut __entry = unsafe {
                SnapshotBidsEntryEncoder::wrap(__buf, self.offset)
            };
            f(&mut __entry)?;
            self.offset = __entry.offset;
        }
        self.written += 1;
        Ok(())
    }
    /**Write one group entry, proving completeness in the type system.

The closure takes the entry encoder **by value** and must return `SnapshotBidsEntryComplete` — reachable only by writing every required tail in wire order. An entry that skips, reorders, or repeats a tail cannot produce that type, so it fails to compile rather than producing a short entry at run time.

[`Self::add`] stays available for entries whose tails are already checked elsewhere.*/
    #[inline]
    pub fn add_checked<'b, F>(&'b mut self, f: F) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnOnce(
            SnapshotBidsEntryEncoder<'b>,
        ) -> Result<SnapshotBidsEntryComplete<'b>, sbe_rt::EncodeError>,
    {
        if self.written >= self.count {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: self.written as u32 + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        {
            let __buf: &'a mut [u8] = unsafe { &mut *(self.buf as *mut [u8]) };
            let __entry = unsafe { SnapshotBidsEntryEncoder::wrap(__buf, self.offset) };
            let __complete = f(__entry)?;
            self.offset = __complete.into_cursor();
        }
        self.written += 1;
        Ok(())
    }
    /// Manual entry creation: returns a borrowed entry encoder.
    /// The entry writes fixed fields directly into the group buffer.
    /// Drop the entry or let it go out of scope to commit it.
    /// The group position is pre-advanced, so fields are written
    /// to the correct offset.
    #[must_use]
    #[inline]
    pub fn start_entry(
        &mut self,
    ) -> Result<SnapshotBidsEntryEncoder<'_>, sbe_rt::EncodeError> {
        if self.written as u32 >= self.count as u32 {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: (self.written as u32) + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self
            .offset
            .checked_add(block_len)
            .map(|end| end > self.buf.len())
            .unwrap_or(true)
        {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let entry_offset = self.offset;
        self.offset += block_len;
        self.written += 1;
        Ok(unsafe {
            SnapshotBidsEntryEncoder::wrap(&mut self.buf[entry_offset..self.offset], 0)
        })
    }
}
impl<'a> SnapshotBidsEncoder<'a> {
    /// Number of entries written so far (for `_unknown_size` back-patch).
    #[inline]
    pub fn written(&self) -> u16 {
        self.written
    }
}
/// Value struct for this group's entries.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotBidsEntry {
    pub price_level: i64,
    pub quantity: i64,
}
impl<'a> SnapshotBidsEncoder<'a> {
    /// Write one entry from a struct. Faster than [`Self::add`] when
    /// the entry has no nested groups or var-data.
    #[inline]
    pub fn add_struct(
        &mut self,
        entry: &SnapshotBidsEntry,
    ) -> Result<(), sbe_rt::EncodeError> {
        if self.written as u32 >= self.count as u32 {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: (self.written as u32) + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let offset = self.offset;
        self.offset += block_len;
        self.written += 1;
        self.buf[offset + 0..offset + 0 + 8]
            .copy_from_slice(&entry.price_level.to_le_bytes());
        self.buf[offset + 8..offset + 8 + 8]
            .copy_from_slice(&entry.quantity.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn bulk_add_with<T, F>(
        &mut self,
        entries: &[T],
        mut write_entry: F,
    ) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnMut(&T, &mut [u8]) -> Result<(), sbe_rt::EncodeError>,
    {
        let count = entries.len();
        if count == 0 {
            return Ok(());
        }
        let attempted = (self.written as usize)
            .checked_add(count)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        if attempted > self.count as usize {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: attempted.min(u32::MAX as usize) as u32,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if block_len == 0 {
            self.written = attempted as u16;
            return Ok(());
        }
        let needed = count
            .checked_mul(block_len)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        let end = self
            .offset
            .checked_add(needed)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        if end > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        {
            let region = &mut self.buf[self.offset..end];
            for (entry, slot) in entries.iter().zip(region.chunks_exact_mut(block_len)) {
                write_entry(entry, slot)?;
            }
        }
        self.offset = end;
        self.written = attempted as u16;
        Ok(())
    }
    /// Encode a slice of fixed-size entries after validating the
    /// complete destination region once.
    #[inline]
    pub fn bulk_add(
        &mut self,
        entries: &[SnapshotBidsEntry],
    ) -> Result<(), sbe_rt::EncodeError> {
        self.bulk_add_with(
            entries,
            |entry, slot| {
                slot[0..0 + 8].copy_from_slice(&entry.price_level.to_le_bytes());
                slot[8..8 + 8].copy_from_slice(&entry.quantity.to_le_bytes());
                Ok(())
            },
        )
    }
}
#[doc = concat!(
    "Proven-complete entry for the `", stringify!(SnapshotBidsEntryComplete), "` group."
)]
pub struct SnapshotBidsEntryComplete<'a> {
    buf: &'a mut [u8],
    entry_start: usize,
    offset: usize,
}
impl<'a> SnapshotBidsEntryComplete<'a> {
    pub(crate) fn into_cursor(self) -> usize {
        self.offset
    }
}
#[doc = concat!(
    "Entry encoder for the `", stringify!(SnapshotBidsEntryEncoder),
    "` group — set fields then call `complete()`."
)]
#[must_use = "entry encoder fields must be set before the next entry"]
pub struct SnapshotBidsEntryEncoder<'a> {
    buf: &'a mut [u8],
    entry_start: usize,
    offset: usize,
}
impl<'a> SnapshotBidsEntryEncoder<'a> {
    /**Finish a flat entry, producing the `SnapshotBidsEntryComplete` that [`SnapshotBidsEncoder::add_checked`] requires.

Only for entries with no required tails — an entry that has them reaches this type through its last tail method instead.*/
    #[inline]
    pub fn complete(self) -> SnapshotBidsEntryComplete<'a> {
        SnapshotBidsEntryComplete {
            buf: self.buf,
            entry_start: self.entry_start,
            offset: self.offset,
        }
    }
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Private entry wrap after the group encoder proved the fixed block
    /// region fits (via `add` / `start_entry` capacity checks).
    ///
    /// # Safety
    /// `offset + ENTRY_BLOCK_LENGTH` must not overflow and must be ≤ `buf.len()`
    /// for the lifetime of the returned encoder.
    #[inline]
    unsafe fn wrap(buf: &'a mut [u8], offset: usize) -> Self {
        Self {
            buf,
            entry_start: offset,
            offset: offset + Self::ENTRY_BLOCK_LENGTH,
        }
    }
    #[inline]
    pub fn price_level(&mut self, val: i64) -> &mut Self {
        let offset = self.entry_start + 0;
        self.buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
        self
    }
    #[inline]
    pub fn quantity(&mut self, val: i64) -> &mut Self {
        let offset = self.entry_start + 8;
        self.buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
        self
    }
}
#[doc = concat!(
    "Encoder for the `", stringify!(SnapshotAsksEncoder),
    "` group — call `add()` to write entries."
)]
#[must_use = "group encoder must call add() to write entries"]
pub struct SnapshotAsksEncoder<'a> {
    buf: &'a mut [u8],
    offset: usize,
    count: u16,
    written: u16,
}
impl<'a> SnapshotAsksEncoder<'a> {
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    pub const GROUP_DIM_TEMPLATE: [u8; 4] = [16, 0, 0, 0];
    const _GROUP_DIM_TEMPLATE_LEN: () = assert!(Self::GROUP_DIM_TEMPLATE.len() == 4);
    #[inline]
    pub fn wrap(buf: &'a mut [u8], offset: usize, count: u16) -> Self {
        Self {
            buf,
            offset,
            count,
            written: 0,
        }
    }
    /// Write one group entry. The closure may return `()` or
    /// `Result<(), sbe_rt::EncodeError>` (both satisfy
    /// [`sbe_rt::GroupResult`]), so `?` works without a `try_add`.
    #[inline]
    #[must_use]
    pub fn add<'b, F>(&'b mut self, f: F) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnOnce(&mut SnapshotAsksEntryEncoder<'b>) -> sbe_rt::GroupResult,
    {
        if self.written >= self.count {
            return Err(
                sbe_rt::EncodeError::GroupFull {
                    declared: self.count as u32,
                    attempted: self.written as u32 + 1,
                }
                    .into(),
            );
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: "group entry",
                    needed: block_len,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        {
            let __buf: &'a mut [u8] = unsafe { &mut *(self.buf as *mut [u8]) };
            let mut __entry = unsafe {
                SnapshotAsksEntryEncoder::wrap(__buf, self.offset)
            };
            f(&mut __entry)?;
            self.offset = __entry.offset;
        }
        self.written += 1;
        Ok(())
    }
    /**Write one group entry, proving completeness in the type system.

The closure takes the entry encoder **by value** and must return `SnapshotAsksEntryComplete` — reachable only by writing every required tail in wire order. An entry that skips, reorders, or repeats a tail cannot produce that type, so it fails to compile rather than producing a short entry at run time.

[`Self::add`] stays available for entries whose tails are already checked elsewhere.*/
    #[inline]
    pub fn add_checked<'b, F>(&'b mut self, f: F) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnOnce(
            SnapshotAsksEntryEncoder<'b>,
        ) -> Result<SnapshotAsksEntryComplete<'b>, sbe_rt::EncodeError>,
    {
        if self.written >= self.count {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: self.written as u32 + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        {
            let __buf: &'a mut [u8] = unsafe { &mut *(self.buf as *mut [u8]) };
            let __entry = unsafe { SnapshotAsksEntryEncoder::wrap(__buf, self.offset) };
            let __complete = f(__entry)?;
            self.offset = __complete.into_cursor();
        }
        self.written += 1;
        Ok(())
    }
    /// Manual entry creation: returns a borrowed entry encoder.
    /// The entry writes fixed fields directly into the group buffer.
    /// Drop the entry or let it go out of scope to commit it.
    /// The group position is pre-advanced, so fields are written
    /// to the correct offset.
    #[must_use]
    #[inline]
    pub fn start_entry(
        &mut self,
    ) -> Result<SnapshotAsksEntryEncoder<'_>, sbe_rt::EncodeError> {
        if self.written as u32 >= self.count as u32 {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: (self.written as u32) + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self
            .offset
            .checked_add(block_len)
            .map(|end| end > self.buf.len())
            .unwrap_or(true)
        {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let entry_offset = self.offset;
        self.offset += block_len;
        self.written += 1;
        Ok(unsafe {
            SnapshotAsksEntryEncoder::wrap(&mut self.buf[entry_offset..self.offset], 0)
        })
    }
}
impl<'a> SnapshotAsksEncoder<'a> {
    /// Number of entries written so far (for `_unknown_size` back-patch).
    #[inline]
    pub fn written(&self) -> u16 {
        self.written
    }
}
/// Value struct for this group's entries.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotAsksEntry {
    pub price_level: i64,
    pub quantity: i64,
}
impl<'a> SnapshotAsksEncoder<'a> {
    /// Write one entry from a struct. Faster than [`Self::add`] when
    /// the entry has no nested groups or var-data.
    #[inline]
    pub fn add_struct(
        &mut self,
        entry: &SnapshotAsksEntry,
    ) -> Result<(), sbe_rt::EncodeError> {
        if self.written as u32 >= self.count as u32 {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: (self.written as u32) + 1,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if self.offset + block_len > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed: block_len,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let offset = self.offset;
        self.offset += block_len;
        self.written += 1;
        self.buf[offset + 0..offset + 0 + 8]
            .copy_from_slice(&entry.price_level.to_le_bytes());
        self.buf[offset + 8..offset + 8 + 8]
            .copy_from_slice(&entry.quantity.to_le_bytes());
        Ok(())
    }
    #[inline]
    fn bulk_add_with<T, F>(
        &mut self,
        entries: &[T],
        mut write_entry: F,
    ) -> Result<(), sbe_rt::EncodeError>
    where
        F: FnMut(&T, &mut [u8]) -> Result<(), sbe_rt::EncodeError>,
    {
        let count = entries.len();
        if count == 0 {
            return Ok(());
        }
        let attempted = (self.written as usize)
            .checked_add(count)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        if attempted > self.count as usize {
            return Err(sbe_rt::EncodeError::GroupFull {
                declared: self.count as u32,
                attempted: attempted.min(u32::MAX as usize) as u32,
            });
        }
        let block_len = Self::ENTRY_BLOCK_LENGTH;
        if block_len == 0 {
            self.written = attempted as u16;
            return Ok(());
        }
        let needed = count
            .checked_mul(block_len)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        let end = self
            .offset
            .checked_add(needed)
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        if end > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: "group entry",
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        {
            let region = &mut self.buf[self.offset..end];
            for (entry, slot) in entries.iter().zip(region.chunks_exact_mut(block_len)) {
                write_entry(entry, slot)?;
            }
        }
        self.offset = end;
        self.written = attempted as u16;
        Ok(())
    }
    /// Encode a slice of fixed-size entries after validating the
    /// complete destination region once.
    #[inline]
    pub fn bulk_add(
        &mut self,
        entries: &[SnapshotAsksEntry],
    ) -> Result<(), sbe_rt::EncodeError> {
        self.bulk_add_with(
            entries,
            |entry, slot| {
                slot[0..0 + 8].copy_from_slice(&entry.price_level.to_le_bytes());
                slot[8..8 + 8].copy_from_slice(&entry.quantity.to_le_bytes());
                Ok(())
            },
        )
    }
}
#[doc = concat!(
    "Proven-complete entry for the `", stringify!(SnapshotAsksEntryComplete), "` group."
)]
pub struct SnapshotAsksEntryComplete<'a> {
    buf: &'a mut [u8],
    entry_start: usize,
    offset: usize,
}
impl<'a> SnapshotAsksEntryComplete<'a> {
    pub(crate) fn into_cursor(self) -> usize {
        self.offset
    }
}
#[doc = concat!(
    "Entry encoder for the `", stringify!(SnapshotAsksEntryEncoder),
    "` group — set fields then call `complete()`."
)]
#[must_use = "entry encoder fields must be set before the next entry"]
pub struct SnapshotAsksEntryEncoder<'a> {
    buf: &'a mut [u8],
    entry_start: usize,
    offset: usize,
}
impl<'a> SnapshotAsksEntryEncoder<'a> {
    /**Finish a flat entry, producing the `SnapshotAsksEntryComplete` that [`SnapshotAsksEncoder::add_checked`] requires.

Only for entries with no required tails — an entry that has them reaches this type through its last tail method instead.*/
    #[inline]
    pub fn complete(self) -> SnapshotAsksEntryComplete<'a> {
        SnapshotAsksEntryComplete {
            buf: self.buf,
            entry_start: self.entry_start,
            offset: self.offset,
        }
    }
    pub const ENTRY_BLOCK_LENGTH: usize = 16;
    /// Private entry wrap after the group encoder proved the fixed block
    /// region fits (via `add` / `start_entry` capacity checks).
    ///
    /// # Safety
    /// `offset + ENTRY_BLOCK_LENGTH` must not overflow and must be ≤ `buf.len()`
    /// for the lifetime of the returned encoder.
    #[inline]
    unsafe fn wrap(buf: &'a mut [u8], offset: usize) -> Self {
        Self {
            buf,
            entry_start: offset,
            offset: offset + Self::ENTRY_BLOCK_LENGTH,
        }
    }
    #[inline]
    pub fn price_level(&mut self, val: i64) -> &mut Self {
        let offset = self.entry_start + 0;
        self.buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
        self
    }
    #[inline]
    pub fn quantity(&mut self, val: i64) -> &mut Self {
        let offset = self.entry_start + 8;
        self.buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
        self
    }
}
impl<'a> SnapshotDecoder<'a> {
    #[inline]
    pub fn try_received_at(
        &self,
    ) -> Result<
        chrono::DateTime<chrono::Utc>,
        <chrono::DateTime<chrono::Utc> as TryFromSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        <chrono::DateTime<
            chrono::Utc,
        > as TryFromSbe<
            UTCTimestampMicrosEncoding,
        >>::try_from_sbe(self.received_at_value())
    }
    #[inline]
    pub fn try_exchange_timestamp(
        &self,
    ) -> Result<
        chrono::DateTime<chrono::Utc>,
        <chrono::DateTime<chrono::Utc> as TryFromSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        <chrono::DateTime<
            chrono::Utc,
        > as TryFromSbe<
            UTCTimestampMicrosEncoding,
        >>::try_from_sbe(self.exchange_timestamp_value())
    }
}
impl<'a, H: sbe_rt::HeaderState> SnapshotEncoder<'a, H> {
    #[inline]
    pub fn try_received_at(
        &mut self,
        value: chrono::DateTime<chrono::Utc>,
    ) -> Result<
        &mut Self,
        <chrono::DateTime<chrono::Utc> as TryToSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        let wire = <chrono::DateTime<
            chrono::Utc,
        > as TryToSbe<UTCTimestampMicrosEncoding>>::try_to_sbe(&value)?;
        self.received_at_wire(wire);
        Ok(self)
    }
    #[inline]
    pub fn try_exchange_timestamp(
        &mut self,
        value: chrono::DateTime<chrono::Utc>,
    ) -> Result<
        &mut Self,
        <chrono::DateTime<chrono::Utc> as TryToSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        let wire = <chrono::DateTime<
            chrono::Utc,
        > as TryToSbe<UTCTimestampMicrosEncoding>>::try_to_sbe(&value)?;
        self.exchange_timestamp_wire(wire);
        Ok(self)
    }
}
pub mod snapshot_field_meta {
    pub struct FieldInfo {
        pub name: &'static str,
        pub id: u16,
        pub offset: usize,
        pub since_version: u16,
        pub field_type: &'static str,
        pub presence: &'static str,
        pub null_value: Option<&'static str>,
        pub semantic_type: Option<&'static str>,
        pub description: Option<&'static str>,
    }
    pub const FIELDS: &[FieldInfo] = &[
        FieldInfo {
            name: "received_at",
            id: 1,
            offset: 0,
            since_version: 0,
            field_type: "UTCTimestampMicrosEncoding",
            presence: "required",
            null_value: None,
            semantic_type: None,
            description: None,
        },
        FieldInfo {
            name: "exchange_timestamp",
            id: 2,
            offset: 8,
            since_version: 0,
            field_type: "UTCTimestampMicrosEncoding",
            presence: "required",
            null_value: None,
            semantic_type: None,
            description: None,
        },
    ];
}
#[doc = concat!(
    "Schema constants for the `", stringify!(PriceLevelUpdateSchema),
    "` message: `SCHEMA_ID`, `SCHEMA_VERSION`, `TEMPLATE_ID`, `BLOCK_LENGTH`, `HEADER_LENGTH`."
)]
pub struct PriceLevelUpdateSchema;
impl PriceLevelUpdateSchema {
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 2;
    pub const BLOCK_LENGTH: usize = 33;
    pub const HEADER_LENGTH: usize = 8;
    /// Full structural verification of a buffer: validates header,
    /// block-length extent, group dimension headers, entry strides,
    /// and var-data bounds. Use **before** construction when the
    /// entire frame must be proven valid without building a decoder.
    #[inline]
    pub fn verify(buf: &[u8]) -> Result<(), sbe_rt::VerifyError> {
        PriceLevelUpdateDecoder::verify(buf)
    }
}
///An update to a single price level, on a single orderbook
#[must_use = "decoder must be read or advanced; dropping is fine only after use"]
pub struct PriceLevelUpdateDecoder<'a> {
    pub(crate) buf: &'a [u8],
    /// Byte offset of the message body within `self.buf`.
    pub(crate) offset: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
/// Buffer-placement and wire-frame metadata. Holds a reference to the
/// parent decoder — zero-copy. Utility methods live here so no schema
/// field can collide with them. Byte views on this facet span the
/// **acting fixed block only**; complete frames use the complete stage
/// or the decoder's tail-rescan helpers when the message has groups
/// or var-data.
#[derive(Clone, Copy)]
pub struct PriceLevelUpdateDecoderMetadata<'m, 'a> {
    decoder: &'m PriceLevelUpdateDecoder<'a>,
}
impl<'m, 'a> PriceLevelUpdateDecoderMetadata<'m, 'a> {
    /// Absolute offset of this message's frame start (first header byte)
    /// within the underlying buffer.
    #[inline]
    pub fn message_offset(&self) -> usize {
        self.decoder.byte_offset().saturating_sub(PriceLevelUpdateDecoder::HEADER_LENGTH)
    }
    /// End of the **acting fixed block** (body start + acting block length).
    /// Not the full message end when groups/var-data follow — use a complete
    /// stage or inherent `encoded_length_with_header` after walking tails.
    #[inline]
    pub fn limit(&self) -> usize {
        self.decoder.byte_offset() + self.decoder.acting_block_length
    }
    /// The full underlying buffer slice this decoder was wrapped on.
    #[inline]
    pub fn buffer(&self) -> &'a [u8] {
        self.decoder.buf
    }
    /// Bytes after the acting fixed block end. May still contain unread
    /// groups/var-data of **this** message until the consuming walk finishes.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        let end = (self.decoder.byte_offset() + self.decoder.acting_block_length)
            .min(self.decoder.buf.len());
        &self.decoder.buf[end..]
    }
    /// Fixed-block body only (groups/var-data not included).
    /// For a complete frame walk tails then use the complete stage's
    /// `as_bytes_with_header`, or the decoder's inherent
    /// `as_bytes_with_header` which rescans tails without consuming
    /// the stage.
    #[inline]
    pub fn as_fixed_body_bytes(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let start = self.decoder.byte_offset();
        let end = self.decoder.byte_offset() + self.decoder.acting_block_length;
        if start > self.decoder.buf.len() || end > self.decoder.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "body",
                needed: end.saturating_sub(start),
                available: self.decoder.buf.len().saturating_sub(start),
            });
        }
        Ok(&self.decoder.buf[start..end])
    }
    /// Header + fixed block only — **not** a complete SBE message when
    /// groups or var-data remain. Prefer the complete stage's
    /// `as_bytes_with_header` after finishing the walk.
    #[inline]
    pub fn as_fixed_region_with_header(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let start = self.message_offset();
        let end = self.decoder.byte_offset() + self.decoder.acting_block_length;
        if start > self.decoder.buf.len() || end > self.decoder.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "frame",
                needed: end.saturating_sub(start),
                available: self.decoder.buf.len().saturating_sub(start),
            });
        }
        Ok(&self.decoder.buf[start..end])
    }
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub fn acting_version(&self) -> u16 {
        self.decoder.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub fn acting_block_length(&self) -> usize {
        self.decoder.acting_block_length
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Metadata accessor: buffer positions, wire-frame boundaries,
    /// version/block-length state. Returns a zero-copy reference to
    /// the parent decoder — no fields are copied. All utility methods
    /// are scoped here so no schema field name can collide with them.
    #[inline]
    pub fn get_metadata(&self) -> PriceLevelUpdateDecoderMetadata<'_, 'a> {
        PriceLevelUpdateDecoderMetadata {
            decoder: self,
        }
    }
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 2;
    pub const BLOCK_LENGTH: usize = 33;
    const _BLOCK_LEN: () = assert!(Self::BLOCK_LENGTH == 33);
    /// Schema-declared message header size in bytes.
    pub const HEADER_LENGTH: usize = 8;
    /// Minimum body bytes needed to safely read every fixed field present
    /// at `acting_version` (version-aware; not always full `BLOCK_LENGTH`).
    #[inline]
    pub const fn min_readable_fixed_extent(acting_version: u16) -> usize {
        let mut m = 33;
        m
    }
    /// Wrap a buffer for decoding at **message start** with bounds checks.
    /// Fields are at `message_offset + HEADER_LENGTH + field_offset`.
    ///
    /// Validates that the body holds `max(acting_block_length,
    /// min_readable_fixed_extent(acting_version))` bytes so required
    /// accessors never read out of bounds from safe code.
    ///
    /// # Migration from sbe-tool
    ///
    /// sbe-tool Rust `wrap` takes the **body** offset (usually
    /// `message_start + 8`). ergo-sbe takes the **message** start so the
    /// same offset works for `wrap`, `decode`, and claim buffers.
    #[inline]
    pub fn try_wrap(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Result<Self, sbe_rt::DecodeError> {
        let Some(body_offset) = message_offset.checked_add(Self::HEADER_LENGTH) else {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: Self::HEADER_LENGTH,
                available: buf.len().saturating_sub(message_offset),
            });
        };
        let available_body = buf.len().saturating_sub(body_offset);
        let min_fixed = Self::min_readable_fixed_extent(acting_version);
        let body_need = if acting_block_length > min_fixed {
            acting_block_length
        } else {
            min_fixed
        };
        if body_need > available_body {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message body",
                needed: Self::HEADER_LENGTH.saturating_add(body_need),
                available: buf.len().saturating_sub(message_offset),
            });
        }
        Ok(unsafe {
            Self::wrap_unchecked(
                buf,
                message_offset,
                acting_block_length,
                acting_version,
            )
        })
    }
    /// Trusted external-metadata wrap. Proves version-aware fixed extent
    /// then constructs; **panics** if the buffer is too short. Field
    /// accessors use unchecked reads justified by that proof.
    ///
    /// Prefer [`Self::try_wrap`] at untrusted boundaries. Uses a direct
    /// extent check (not `try_wrap` + match) so the hot success path does
    /// not construct a `Result` — same contract as encoder bare `wrap`.
    #[inline]
    pub fn wrap(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        let Some(body_offset) = message_offset.checked_add(Self::HEADER_LENGTH) else {
            panic!("buffer too short for message header");
        };
        let available_body = buf.len().saturating_sub(body_offset);
        let min_fixed = Self::min_readable_fixed_extent(acting_version);
        let body_need = if acting_block_length > min_fixed {
            acting_block_length
        } else {
            min_fixed
        };
        if body_need > available_body {
            panic!("buffer too short for message body");
        }
        unsafe {
            Self::wrap_unchecked(
                buf,
                message_offset,
                acting_block_length,
                acting_version,
            )
        }
    }
    /// Zero-check wrap — raw pointer accessors, **UB** on OOB.
    /// Only for proven-tight hot loops after an external extent proof.
    ///
    /// # Safety
    /// `message_offset + HEADER_LENGTH + max(acting_block_length,
    /// min_readable_fixed_extent(acting_version))` must not overflow
    /// and must be ≤ `buf.len()`.
    #[inline]
    pub unsafe fn wrap_unchecked(
        buf: &'a [u8],
        message_offset: usize,
        acting_block_length: usize,
        acting_version: u16,
    ) -> Self {
        let body_offset = message_offset + Self::HEADER_LENGTH;
        Self {
            buf,
            offset: body_offset,
            acting_block_length,
            acting_version,
        }
    }
    /// Decode a framed message at **message start** (`offset` = first
    /// byte of the header). Validates header fields and the
    /// version-aware fixed body extent. See [`Self::wrap`] for the
    /// message-start coordinate system.
    #[inline]
    pub fn try_decode(
        buf: &'a [u8],
        offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        if 8 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: 8,
                available: buf.len().saturating_sub(offset),
            });
        }
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "PriceLevelUpdate",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Self::try_wrap(buf, offset, acting_block_length, acting_version)
    }
    /// Trusted framed decode — **hybrid return** (freeze-friendly):
    ///
    /// - **Extent (short buffer):** panics after the same proof as
    ///   [`Self::wrap`] (trusted tier).
    /// - **Identity (wrong template/schema):** still returns `Err`
    ///   so session demux can recover without catch_unwind.
    ///
    /// Signature therefore looks like [`Self::try_decode`], but short
    /// buffers do **not** yield `BufferTooShort` — they panic. Prefer
    /// [`Self::try_decode`] at untrusted boundaries when every failure
    /// must be a `Result`.
    #[inline]
    pub fn decode(buf: &'a [u8], offset: usize) -> Result<Self, sbe_rt::DecodeError> {
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "PriceLevelUpdate",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Ok(Self::wrap(buf, offset, acting_block_length, acting_version))
    }
    /// Unchecked **extent**, checked **identity**.
    ///
    /// Header/body bytes are read without bounds checks (**UB** if the
    /// caller has not proven the frame fits). Template/schema identity
    /// still returns `Err` (same hybrid policy as [`Self::decode`]).
    ///
    /// # Safety
    /// Header and version-readable fixed body for this template must
    /// be fully in-bounds at `offset`.
    #[inline]
    pub unsafe fn decode_unchecked(
        buf: &'a [u8],
        offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        let header_bytes: [u8; 8] = unsafe { read_bytes_unchecked::<8>(buf, offset) };
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        if template_id != Self::TEMPLATE_ID {
            return Err(sbe_rt::DecodeError::WrongTemplate {
                expected: Self::TEMPLATE_ID,
                actual: template_id,
                expected_name: "PriceLevelUpdate",
            });
        }
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        if schema_id != Self::SCHEMA_ID {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: Self::SCHEMA_ID,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        let acting_block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let acting_version = sbe_rt::checked_header_u16(
            "version",
            header.version() as u64,
        )?;
        Ok(unsafe {
            Self::wrap_unchecked(buf, offset, acting_block_length, acting_version)
        })
    }
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
    #[inline]
    pub fn received_at_wire(&self) -> UTCTimestampMicrosEncodingDecoder<'_> {
        UTCTimestampMicrosEncodingDecoder {
            buf: self.buf,
            offset: self.offset + 0,
        }
    }
    #[inline]
    pub fn received_at_value(&self) -> UTCTimestampMicrosEncoding {
        UTCTimestampMicrosEncoding(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 0)
        })
    }
    pub const RECEIVED_AT_ID: u16 = 1;
    pub const RECEIVED_AT_SINCE_VERSION: u16 = 0;
    pub const RECEIVED_AT_ENCODING_OFFSET: usize = 0;
    pub const RECEIVED_AT_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn received_at_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    #[inline]
    pub fn exchange_timestamp_wire(&self) -> UTCTimestampMicrosEncodingDecoder<'_> {
        UTCTimestampMicrosEncodingDecoder {
            buf: self.buf,
            offset: self.offset + 8,
        }
    }
    #[inline]
    pub fn exchange_timestamp_value(&self) -> UTCTimestampMicrosEncoding {
        UTCTimestampMicrosEncoding(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 8)
        })
    }
    pub const EXCHANGE_TIMESTAMP_ID: u16 = 2;
    pub const EXCHANGE_TIMESTAMP_SINCE_VERSION: u16 = 0;
    pub const EXCHANGE_TIMESTAMP_ENCODING_OFFSET: usize = 8;
    pub const EXCHANGE_TIMESTAMP_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn exchange_timestamp_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    #[inline]
    pub fn price_level(&self) -> i64 {
        i64::from_le_bytes(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 16)
        })
    }
    pub const PRICE_LEVEL_ID: u16 = 4;
    pub const PRICE_LEVEL_SINCE_VERSION: u16 = 0;
    pub const PRICE_LEVEL_ENCODING_OFFSET: usize = 16;
    pub const PRICE_LEVEL_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn price_level_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const PRICE_LEVEL_NULL: i64 = -9223372036854775808_i64;
    pub const PRICE_LEVEL_MIN: i64 = -9223372036854775807_i64;
    pub const PRICE_LEVEL_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    pub fn quantity(&self) -> i64 {
        i64::from_le_bytes(unsafe {
            read_bytes_unchecked::<8>(self.buf, self.offset + 24)
        })
    }
    pub const QUANTITY_ID: u16 = 5;
    pub const QUANTITY_SINCE_VERSION: u16 = 0;
    pub const QUANTITY_ENCODING_OFFSET: usize = 24;
    pub const QUANTITY_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn quantity_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const QUANTITY_NULL: i64 = -9223372036854775808_i64;
    pub const QUANTITY_MIN: i64 = -9223372036854775807_i64;
    pub const QUANTITY_MAX: i64 = 9223372036854775807_i64;
    #[inline]
    pub fn side_wire(&self) -> SideEncodingDecoder<'_> {
        SideEncodingDecoder {
            buf: self.buf,
            offset: self.offset + 32,
        }
    }
    #[inline]
    pub fn side_value(&self) -> SideEncoding {
        SideEncoding(unsafe { read_bytes_unchecked::<1>(self.buf, self.offset + 32) })
    }
    pub const SIDE_ID: u16 = 6;
    pub const SIDE_SINCE_VERSION: u16 = 0;
    pub const SIDE_ENCODING_OFFSET: usize = 32;
    pub const SIDE_ENCODING_LENGTH: usize = 1;
    #[inline]
    pub const fn side_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    /// Byte offset of the message body within `self.buf`.
    #[inline]
    fn byte_offset(&self) -> usize {
        self.offset
    }
    #[inline]
    fn tail_offset_0(&self) -> Result<usize, sbe_rt::DecodeError> {
        Ok(self.byte_offset() + self.acting_block_length)
    }
    #[inline]
    fn tail_offset_1(&self) -> Result<usize, sbe_rt::DecodeError> {
        let start = self.tail_offset_0()?;
        if 4 > self.buf.len().saturating_sub(start) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(start),
            });
        }
        let bytes: [u8; 4] = read_bytes::<4>(self.buf, start);
        let header = VarStringEncoding(bytes);
        let wire_length = header.length() as u64;
        let (_, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            start,
            4,
            wire_length,
            self.buf.len(),
        )?;
        Ok(data_end)
    }
    #[inline]
    pub fn symbol(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let offset = self.tail_offset_0()?;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: stringify!(symbol),
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let header = VarStringEncoding(bytes);
        let wire_length = header.length() as u64;
        if wire_length > 1073741824 as u64 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: stringify!(symbol),
                length: wire_length,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            stringify!(symbol),
            offset,
            4,
            wire_length,
            self.buf.len(),
        )?;
        Ok(&self.buf[data_start..data_end])
    }
    /// View this UTF-8 var-data field as `&str`.
    #[inline]
    pub fn symbol_as_str(&self) -> Result<&'a str, sbe_rt::DecodeError> {
        let bytes = self.symbol()?;
        core::str::from_utf8(bytes)
            .map_err(|e| sbe_rt::DecodeError::InvalidUtf8 {
                field: "symbol",
                error: e,
            })
    }
    /// View this text var-data field as `&str` without UTF-8
    /// validation.
    ///
    /// # Safety
    ///
    /// The wire bytes must be valid UTF-8.
    #[inline]
    pub unsafe fn symbol_as_str_unchecked(&self) -> &'a str {
        let bytes = unsafe { self.symbol().unwrap() };
        unsafe { core::str::from_utf8_unchecked(bytes) }
    }
    /// Consume this stage and return a fresh decoder at the initial
    /// message position. The consumed stage cannot be reused.
    #[inline]
    pub fn rewind(self) -> Self {
        self
    }
    #[inline]
    pub fn encoded_length(&self) -> Result<usize, sbe_rt::DecodeError> {
        let end = self.tail_offset_1()?;
        Ok(end - self.byte_offset())
    }
    #[inline]
    pub fn encoded_length_with_header(&self) -> Result<usize, sbe_rt::DecodeError> {
        let len = self.encoded_length()?;
        Ok(len + 8)
    }
    #[inline]
    pub fn as_body_bytes(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let end = self.tail_offset_1()?;
        let start = self.byte_offset();
        Ok(&self.buf[start..end])
    }
    #[inline]
    pub fn as_bytes_with_header(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let end = self.tail_offset_1()?;
        let start = self.byte_offset().saturating_sub(Self::HEADER_LENGTH);
        Ok(&self.buf[start..end])
    }
    #[inline]
    pub fn verify(buf: &[u8]) -> Result<(), sbe_rt::VerifyError> {
        if buf.len() < 8 {
            return Err(sbe_rt::VerifyError::HeaderTooShort);
        }
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, 0);
        let header = MessageHeader(header_bytes);
        let block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        if block_length < Self::BLOCK_LENGTH {
            return Err(sbe_rt::VerifyError::InvalidBlockLength {
                expected_min: Self::BLOCK_LENGTH,
                actual: block_length,
            });
        }
        let body_end = (8 as usize)
            .checked_add(block_length)
            .ok_or(sbe_rt::VerifyError::MessageTooShort {
                needed: usize::MAX,
                available: buf.len(),
            })?;
        if body_end > buf.len() {
            return Err(sbe_rt::VerifyError::MessageTooShort {
                needed: body_end,
                available: buf.len(),
            });
        }
        let mut offset = body_end;
        {
            if 4 > buf.len().saturating_sub(offset) {
                return Err(sbe_rt::VerifyError::VarDataOutOfBounds {
                    field: "symbol",
                    offset,
                    length: 0,
                });
            }
            let bytes: [u8; 4] = read_bytes::<4>(buf, offset);
            let var_header = VarStringEncoding(bytes);
            let len = var_header.length() as u64;
            let (_, data_end) = match sbe_rt::checked_var_data_bounds(
                "symbol",
                offset,
                4,
                len,
                buf.len(),
            ) {
                Ok(bounds) => bounds,
                Err(_) => {
                    return Err(sbe_rt::VerifyError::VarDataOutOfBounds {
                        field: "symbol",
                        offset,
                        length: len,
                    });
                }
            };
            offset = data_end;
        }
        Ok(())
    }
}
impl<'a> TryFrom<&'a [u8]> for PriceLevelUpdateDecoder<'a> {
    type Error = sbe_rt::DecodeError;
    #[inline]
    fn try_from(buf: &'a [u8]) -> Result<Self, Self::Error> {
        Self::try_decode(buf, 0)
    }
}
impl<'a> __sbe_message_sealed::Sealed for PriceLevelUpdateDecoder<'a> {}
impl<'a> sbe_rt::SbeMessage for PriceLevelUpdateDecoder<'a> {
    const TEMPLATE_ID: u16 = 2;
    const BLOCK_LENGTH: usize = 33;
    const SCHEMA_ID: u16 = 1;
    const SCHEMA_VERSION: u16 = 2;
}
impl<'a> core::fmt::Display for PriceLevelUpdateDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}
impl<'a> core::fmt::Debug for PriceLevelUpdateDecoder<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("PriceLevelUpdateDecoder");
        if self.byte_offset().saturating_add(8) <= self.buf.len()
            && 8 <= self.acting_block_length
        {
            if let Ok(v) = self.try_received_at() {
                d.field("received_at", &format_args!("{}", v));
            }
        }
        if self.byte_offset().saturating_add(16) <= self.buf.len()
            && 16 <= self.acting_block_length
        {
            if let Ok(v) = self.try_exchange_timestamp() {
                d.field("exchange_timestamp", &format_args!("{}", v));
            }
        }
        if self.byte_offset().saturating_add(24) <= self.buf.len()
            && 24 <= self.acting_block_length
        {
            let v = self.price_level();
            d.field("price_level", &v);
        }
        if self.byte_offset().saturating_add(32) <= self.buf.len()
            && 32 <= self.acting_block_length
        {
            let v = self.quantity();
            d.field("quantity", &v);
        }
        if self.byte_offset().saturating_add(33) <= self.buf.len()
            && 33 <= self.acting_block_length
        {
            if let Ok(v) = self.try_side() {
                d.field("side", &format_args!("{}", v));
            }
        }
        if let Ok(_data) = self.symbol() {
            match std::str::from_utf8(_data) {
                Ok(_s) => d.field("symbol", &_s),
                Err(_) => d.field("symbol", &format!("<{} bytes>", _data.len())),
            };
        }
        d.finish()
    }
}
/// Consuming decoder stage — drop without `into_*` / `finish` skips
/// remaining wire tails.
#[must_use = "decoder stage must be advanced with into_*/finish or tails are skipped"]
pub struct PriceLevelUpdateDecoderComplete<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) offset: usize,
    pub(crate) tail_start: usize,
    pub(crate) acting_version: u16,
    pub(crate) acting_block_length: usize,
}
impl<'a> PriceLevelUpdateDecoderComplete<'a> {
    /// Schema version from the message header (or wrap args), not the
    /// compiled schema constant. Fields with `sinceVersion` and optional
    /// presence depend on this value.
    #[inline]
    pub const fn acting_version(&self) -> u16 {
        self.acting_version
    }
    /// Block length from the wire header / wrap args. Tail offsets use
    /// this acting length, not only the compiled `BLOCK_LENGTH`.
    #[inline]
    pub const fn acting_block_length(&self) -> usize {
        self.acting_block_length
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Consume this stage, read the next var-data field, and advance
    /// to the following stage. Wire order is enforced by consumption.
    #[inline]
    pub fn into_symbol(
        self,
    ) -> Result<(&'a [u8], PriceLevelUpdateDecoderComplete<'a>), sbe_rt::DecodeError> {
        let offset = self.byte_offset() + self.acting_block_length;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let len = u32::from_le_bytes(bytes) as u64;
        if len > 1073741824 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: "symbol",
                length: len,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            offset,
            4,
            len,
            self.buf.len(),
        )?;
        let data = &self.buf[data_start..data_end];
        let next = PriceLevelUpdateDecoderComplete {
            buf: self.buf,
            offset: self.byte_offset(),
            tail_start: data_end,
            acting_version: self.acting_version,
            acting_block_length: self.acting_block_length,
        };
        Ok((data, next))
    }
    /**Non-consuming variant: read this var-data field as `&[u8]` without advancing or constructing the next stage.

Cheaper than [`Self::into_symbol`] when only the bytes are needed.*/
    #[inline]
    pub fn symbol_slice(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        let offset = self.byte_offset() + self.acting_block_length;
        if offset + 4 > self.buf.len() {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "symbol",
                needed: 4,
                available: self.buf.len().saturating_sub(offset),
            });
        }
        let bytes: [u8; 4] = unsafe {
            core::ptr::read_unaligned(self.buf.as_ptr().add(offset) as *const [u8; 4])
        };
        let len = u32::from_le_bytes(bytes) as u64;
        if len > 1073741824 {
            return Err(sbe_rt::DecodeError::InvalidVarDataLength {
                field: "symbol",
                length: len,
                max_length: 1073741824 as u64,
            });
        }
        let (data_start, data_end) = sbe_rt::checked_var_data_bounds(
            "symbol",
            offset,
            4,
            len,
            self.buf.len(),
        )?;
        Ok(&self.buf[data_start..data_end])
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Consume this stage, read the next UTF-8 var-data
    /// field as a validated `&str`, and advance.
    #[inline]
    pub fn into_symbol_as_str(
        self,
    ) -> Result<(&'a str, PriceLevelUpdateDecoderComplete<'a>), sbe_rt::DecodeError> {
        let (bytes, next) = self.into_symbol()?;
        let s = core::str::from_utf8(bytes)
            .map_err(|e| {
                sbe_rt::DecodeError::InvalidUtf8 {
                    field: "symbol",
                    error: e,
                }
            })?;
        Ok((s, next))
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Consume this stage, read the next text var-data field as
    /// a `&str` without encoding validation, and advance.
    ///
    /// # Safety
    /// The wire bytes must be valid for the schema-declared
    /// character encoding (UTF-8 or ASCII).
    #[inline]
    pub unsafe fn into_symbol_as_str_unchecked(
        self,
    ) -> (&'a str, PriceLevelUpdateDecoderComplete<'a>) {
        let (bytes, next) = unsafe { self.into_symbol().unwrap() };
        let s = unsafe { core::str::from_utf8_unchecked(bytes) };
        (s, next)
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Fallible scoped var-data accessor. Calls the closure with
    /// the decoded bytes and returns the next stage on success.
    #[inline]
    pub fn try_symbol<E, F>(self, f: F) -> Result<PriceLevelUpdateDecoderComplete<'a>, E>
    where
        E: From<sbe_rt::DecodeError>,
        F: FnOnce(&[u8]) -> Result<(), E>,
    {
        let (data, next) = self.into_symbol()?;
        f(data)?;
        Ok(next)
    }
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    /// Consume this stage, decode the var-data field as a nested
    /// SBE message via `AnyMessage::decode_frame`, and advance
    /// to the next stage.
    #[inline]
    pub fn into_symbol_as_message(
        self,
    ) -> Result<
        (DecodedFrame<'a>, PriceLevelUpdateDecoderComplete<'a>),
        sbe_rt::DecodeError,
    > {
        let (data, next) = self.into_symbol()?;
        let frame = AnyMessage::decode_frame(data, 0, data.len())?;
        Ok((frame, next))
    }
    /// Fallible scoped nested-message accessor.
    #[inline]
    pub fn try_symbol_as_message<E, F>(
        self,
        f: F,
    ) -> Result<PriceLevelUpdateDecoderComplete<'a>, E>
    where
        E: From<sbe_rt::DecodeError>,
        F: FnOnce(DecodedFrame<'a>) -> Result<(), E>,
    {
        let (frame, next) = self.into_symbol_as_message()?;
        f(frame)?;
        Ok(next)
    }
}
impl<'a> PriceLevelUpdateDecoderComplete<'a> {
    /// Body bytes (excluding the message header; for entries this is the
    /// complete entry bytes).
    #[inline]
    pub fn as_body_bytes(&self) -> &'a [u8] {
        &self.buf[self.offset..self.tail_start]
    }
    /// Complete SBE frame (header + body) for message stages.
    /// For entry stages (`HEADER_LENGTH == 0`) this equals [`Self::as_body_bytes`].
    #[inline]
    pub fn as_bytes_with_header(&self) -> &'a [u8] {
        &self.buf[self.offset - 8..self.tail_start]
    }
    /// Body length (excluding header).
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.tail_start - self.offset
    }
    /// Total message length including the schema-declared header.
    /// Pure arithmetic: body length + `HEADER_LENGTH`.
    #[inline]
    pub fn encoded_length_with_header(&self) -> usize {
        self.tail_start - self.offset + 8
    }
    /// Bytes after this message/entry.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        &self.buf[self.tail_start..]
    }
}
/// Owned domain object — application-layer counterpart to the flyweight decoder.
///
/// Materialise with [`Self::try_from_decoder`] (from a decoder) or
/// [`Self::try_from_slice_with_header`] (from framed bytes).
/// These are inherent methods, not `TryFrom`/`From`: there are two fallible
/// sources, and conversion is never infallible (groups, var-data, converters).
#[derive(Debug, Clone, PartialEq)]
pub struct PriceLevelUpdateDomain {
    pub received_at: chrono::DateTime<chrono::Utc>,
    pub exchange_timestamp: chrono::DateTime<chrono::Utc>,
    pub price_level: i64,
    pub quantity: i64,
    pub side: crate::Side,
    pub symbol: String,
}
impl PriceLevelUpdateDomain {
    /// Fallible conversion from a flyweight decoder.
    ///
    /// Propagates decode errors from malformed group entries and var-data
    /// instead of panicking. Prefer this over `From`/`TryFrom`: the companion
    /// entry point is [`Self::try_from_slice_with_header`] (when generated),
    /// and named methods make the two sources unambiguous.
    #[inline]
    pub fn try_from_decoder(
        dec: PriceLevelUpdateDecoder<'_>,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Ok(Self {
            received_at: dec
                .try_received_at()
                .map_err(|_| {
                    sbe_rt::DecodeError::DomainConversionFailed {
                        field: "received_at",
                        reason: "try_* conversion rejected wire value",
                    }
                })?,
            exchange_timestamp: dec
                .try_exchange_timestamp()
                .map_err(|_| {
                    sbe_rt::DecodeError::DomainConversionFailed {
                        field: "exchange_timestamp",
                        reason: "try_* conversion rejected wire value",
                    }
                })?,
            price_level: dec.price_level(),
            quantity: dec.quantity(),
            side: dec
                .try_side()
                .map_err(|_| {
                    sbe_rt::DecodeError::DomainConversionFailed {
                        field: "side",
                        reason: "try_* conversion rejected wire value",
                    }
                })?,
            symbol: match dec.symbol() {
                Ok(data) => {
                    match core::str::from_utf8(data) {
                        Ok(s) => s.to_owned(),
                        Err(error) => {
                            return Err(sbe_rt::DecodeError::InvalidUtf8 {
                                field: "symbol",
                                error,
                            });
                        }
                    }
                }
                Err(e) => return Err(e),
            },
        })
    }
    /// Decode from a framed byte slice: validate the message header, then
    /// materialise the full domain object.
    ///
    /// Distinct from [`Self::try_from_decoder`]: this path owns header
    /// validation + offset; that path starts from an already-wrapped decoder.
    /// Named methods (not `TryFrom`/`From`) keep the two sources obvious.
    #[inline]
    pub fn try_from_slice_with_header(
        buf: &[u8],
        message_offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        Self::try_from_decoder(PriceLevelUpdateDecoder::decode(buf, message_offset)?)
    }
}
impl PriceLevelUpdateDomain {
    #[inline]
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, sbe_rt::EncodeError> {
        let mut enc = PriceLevelUpdateEncoder::try_wrap_and_apply_header(buf, 0)?;
        enc.try_received_at(self.received_at)
            .map_err(|_| {
                sbe_rt::EncodeError::DomainConversionFailed {
                    field: "received_at",
                    reason: "try_* conversion rejected domain value",
                }
            })?;
        enc.try_exchange_timestamp(self.exchange_timestamp)
            .map_err(|_| {
                sbe_rt::EncodeError::DomainConversionFailed {
                    field: "exchange_timestamp",
                    reason: "try_* conversion rejected domain value",
                }
            })?;
        {
            let __v = self.price_level as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "price_level",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.price_level(self.price_level);
        {
            let __v = self.quantity as i128;
            if __v < -9223372036854775807 || __v > 9223372036854775807 {
                return Err(sbe_rt::EncodeError::ValueOutOfRange {
                    field: "quantity",
                    min: -9223372036854775807,
                    max: 9223372036854775807,
                    actual: __v,
                });
            }
        }
        enc.quantity(self.quantity);
        enc.try_side(self.side)
            .map_err(|_| {
                sbe_rt::EncodeError::DomainConversionFailed {
                    field: "side",
                    reason: "try_* conversion rejected domain value",
                }
            })?;
        let enc = enc.symbol(self.symbol.as_bytes())?;
        Ok(enc.encoded_length() + PriceLevelUpdateEncoder::HEADER_LENGTH)
    }
    /// Compute the exact SBE message body length from this domain object.
    /// Matches the length returned by [`Self::encode`].
    #[inline]
    pub fn encoded_length(&self) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 33;
        if self.symbol.len() > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: self.symbol.len(),
            });
        }
        len = len.checked_add(4).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        len = len
            .checked_add(self.symbol.len())
            .ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)?;
        Ok(len)
    }
    /// Compute the exact SBE message length including the message header.
    /// Matches `encode()` return value for non-fixed messages.
    #[inline]
    pub fn encoded_length_with_header(&self) -> Result<usize, sbe_rt::EncodeError> {
        Ok(self.encoded_length()? + PriceLevelUpdateEncoder::HEADER_LENGTH)
    }
}
///An update to a single price level, on a single orderbook
#[doc = concat!(
    "Encoder stage `", "PriceLevelUpdateEncoder",
    "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct PriceLevelUpdateEncoder<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for PriceLevelUpdateEncoder<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match PriceLevelUpdateDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "PriceLevelUpdateEncoder"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for PriceLevelUpdateEncoder<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match PriceLevelUpdateDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("PriceLevelUpdateEncoder")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
#[doc = concat!(
    "Encoder stage `", "PriceLevelUpdateComplete",
    "` — write tail elements in wire order."
)]
#[must_use = "encoder must be consumed to write the message"]
pub struct PriceLevelUpdateComplete<'a, H: sbe_rt::HeaderState = sbe_rt::HeaderPresent> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
    _header: core::marker::PhantomData<H>,
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Display for PriceLevelUpdateComplete<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match PriceLevelUpdateDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Display::fmt(&dec, f),
            Err(_) => write!(f, "<partial {}>", "PriceLevelUpdateComplete"),
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> core::fmt::Debug for PriceLevelUpdateComplete<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match PriceLevelUpdateDecoder::decode(self.buf, self.msg_offset) {
            Ok(dec) => core::fmt::Debug::fmt(&dec, f),
            Err(_) => {
                f.debug_struct("PriceLevelUpdateComplete")
                    .field("msg_offset", &self.msg_offset)
                    .field("offset", &self.offset)
                    .field("buf_len", &self.buf.len())
                    .finish()
            }
        }
    }
}
/// Complete set of latest-version fixed fields for this message.
/// Required fields are concrete values; optional/versioned fields
/// are `Option<T>`. Constants are excluded.
///
/// This struct is **intentionally exhaustive** (not
/// `#[non_exhaustive]`): when the schema adds a fixed field, every
/// `fixed(&…)` call site must be updated. That is a feature — schema
/// changes surface as compile errors rather than silent defaults.
#[derive(Debug, Clone)]
pub struct PriceLevelUpdateFixedFields {
    pub received_at: UTCTimestampMicrosEncoding,
    pub exchange_timestamp: UTCTimestampMicrosEncoding,
    pub price_level: i64,
    pub quantity: i64,
    pub side: SideEncoding,
}
///Raw fixed-field writer. Individual field setters are available only on this writer. When done, embed the fields in a [`PriceLevelUpdateFixedFields`] and call the encoder's `fixed()`.
#[must_use = "raw fixed writer must be embedded in FixedFields"]
pub struct PriceLevelUpdateRawFixedWriter<'a> {
    buf: &'a mut [u8],
    msg_offset: usize,
    offset: usize,
}
impl<'a> PriceLevelUpdateEncoder<'a> {
    pub const SCHEMA_ID: u16 = 1;
    pub const SCHEMA_VERSION: u16 = 2;
    pub const TEMPLATE_ID: u16 = 2;
    pub const BLOCK_LENGTH: usize = 33;
    const _BLOCK_LEN: () = assert!(Self::BLOCK_LENGTH == 33);
    /// Schema-declared message header size in bytes.
    pub const HEADER_LENGTH: usize = 8;
    pub const HEADER_TEMPLATE: [u8; 8] = [33, 0, 2, 0, 1, 0, 2, 0];
    const _HEADER_TEMPLATE_LEN: () = assert!(Self::HEADER_TEMPLATE.len() == 8);
    /// Cold error constructor — never inlined into the hot path.
    #[cold]
    #[inline(never)]
    fn buffer_too_short(
        buf: &[u8],
        offset: usize,
        needed: usize,
    ) -> sbe_rt::EncodeError {
        sbe_rt::EncodeError::BufferTooShort {
            field: "message header+body",
            needed,
            available: buf.len().saturating_sub(offset),
        }
    }
    /// Wrap a mutable buffer for encoding with one bounds/overflow check.
    /// Does **not** write the message header (`HeaderAbsent`).
    ///
    /// `msg_offset` is the **message start** (first byte of the SBE frame),
    /// not the body. sbe-tool Rust `wrap` takes the body offset instead.
    ///
    /// Prefer [`Self::wrap_and_apply_header`] when encoding a full frame.
    #[inline]
    pub fn try_wrap(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> Result<PriceLevelUpdateEncoder<'a, sbe_rt::HeaderAbsent>, sbe_rt::EncodeError> {
        if 41 > buf.len().saturating_sub(msg_offset) {
            return Err(Self::buffer_too_short(buf, msg_offset, 41));
        }
        Ok(unsafe { Self::wrap_unchecked(buf, msg_offset) })
    }
    /// Trusted body-only wrap. Proves header + fixed-body extent then
    /// constructs; **panics** if the buffer is too short. Field setters
    /// use unchecked writes justified by that proof.
    ///
    /// Prefer [`Self::try_wrap`] at untrusted boundaries.
    #[inline]
    pub fn wrap(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> PriceLevelUpdateEncoder<'a, sbe_rt::HeaderAbsent> {
        if 41 > buf.len().saturating_sub(msg_offset) {
            panic!("{}", Self::buffer_too_short(buf, msg_offset, 41));
        }
        unsafe { Self::wrap_unchecked(buf, msg_offset) }
    }
    /// Zero-check body-only wrap — raw pointer ops, **UB** on OOB.
    /// Only for proven-tight hot loops where the panic machinery is
    /// measurable in the critical path.
    ///
    /// # Safety
    /// `msg_offset + HEADER_LENGTH + BLOCK_LENGTH` must not overflow
    /// and must be ≤ `buf.len()` for the lifetime of the encoder.
    #[inline]
    pub unsafe fn wrap_unchecked(
        buf: &'a mut [u8],
        msg_offset: usize,
    ) -> PriceLevelUpdateEncoder<'a, sbe_rt::HeaderAbsent> {
        let body_offset = msg_offset + 8;
        PriceLevelUpdateEncoder {
            buf,
            msg_offset,
            offset: body_offset + 33,
            _header: core::marker::PhantomData,
        }
    }
    /// Wrap a mutable buffer, write the header, with one bounds/overflow check.
    /// `offset` is the **message start** (see [`Self::wrap`]).
    ///
    /// Optional-field nullification is **not** applied by default — call
    /// `apply_nulls()` if you want null sentinels.
    #[inline]
    pub fn try_wrap_and_apply_header(
        buf: &'a mut [u8],
        offset: usize,
    ) -> Result<
        PriceLevelUpdateEncoder<'a, sbe_rt::HeaderPresent>,
        sbe_rt::EncodeError,
    > {
        if 41 > buf.len().saturating_sub(offset) {
            return Err(Self::buffer_too_short(buf, offset, 41));
        }
        Ok(unsafe { Self::wrap_and_apply_header_unchecked(buf, offset) })
    }
    /// Trusted full-frame wrap + header. Proves header + fixed-body extent
    /// then writes the header; **panics** if the buffer is too short.
    /// Field setters use unchecked writes justified by that proof.
    ///
    /// Prefer [`Self::try_wrap_and_apply_header`] at untrusted boundaries.
    /// Call [`Self::wrap_and_apply_header_unchecked`] only with a proven
    /// extent when even panic machinery must be avoided.
    #[inline]
    pub fn wrap_and_apply_header(
        buf: &'a mut [u8],
        offset: usize,
    ) -> PriceLevelUpdateEncoder<'a, sbe_rt::HeaderPresent> {
        if 41 > buf.len().saturating_sub(offset) {
            panic!("{}", Self::buffer_too_short(buf, offset, 41));
        }
        unsafe { Self::wrap_and_apply_header_unchecked(buf, offset) }
    }
    /// Zero-check full-frame wrap + header — `copy_nonoverlapping`, **UB**
    /// on OOB. Only for proven-tight hot loops.
    ///
    /// # Safety
    /// `offset + HEADER_LENGTH + BLOCK_LENGTH` must not overflow and must be
    /// ≤ `buf.len()` for the lifetime of the encoder.
    #[inline]
    pub unsafe fn wrap_and_apply_header_unchecked(
        buf: &'a mut [u8],
        offset: usize,
    ) -> PriceLevelUpdateEncoder<'a, sbe_rt::HeaderPresent> {
        unsafe {
            core::ptr::copy_nonoverlapping(
                Self::HEADER_TEMPLATE.as_ptr(),
                buf.as_mut_ptr().add(offset),
                8,
            );
        }
        let body_offset = offset + 8;
        PriceLevelUpdateEncoder {
            buf,
            msg_offset: offset,
            offset: body_offset + 33,
            _header: core::marker::PhantomData,
        }
    }
    pub const RECEIVED_AT_ID: u16 = 1;
    pub const RECEIVED_AT_SINCE_VERSION: u16 = 0;
    pub const RECEIVED_AT_ENCODING_OFFSET: usize = 0;
    pub const RECEIVED_AT_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn received_at_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const EXCHANGE_TIMESTAMP_ID: u16 = 2;
    pub const EXCHANGE_TIMESTAMP_SINCE_VERSION: u16 = 0;
    pub const EXCHANGE_TIMESTAMP_ENCODING_OFFSET: usize = 8;
    pub const EXCHANGE_TIMESTAMP_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn exchange_timestamp_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const PRICE_LEVEL_ID: u16 = 4;
    pub const PRICE_LEVEL_SINCE_VERSION: u16 = 0;
    pub const PRICE_LEVEL_ENCODING_OFFSET: usize = 16;
    pub const PRICE_LEVEL_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn price_level_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const PRICE_LEVEL_NULL: i64 = -9223372036854775808_i64;
    pub const PRICE_LEVEL_MIN: i64 = -9223372036854775807_i64;
    pub const PRICE_LEVEL_MAX: i64 = 9223372036854775807_i64;
    pub const QUANTITY_ID: u16 = 5;
    pub const QUANTITY_SINCE_VERSION: u16 = 0;
    pub const QUANTITY_ENCODING_OFFSET: usize = 24;
    pub const QUANTITY_ENCODING_LENGTH: usize = 8;
    #[inline]
    pub const fn quantity_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    pub const QUANTITY_NULL: i64 = -9223372036854775808_i64;
    pub const QUANTITY_MIN: i64 = -9223372036854775807_i64;
    pub const QUANTITY_MAX: i64 = 9223372036854775807_i64;
    pub const SIDE_ID: u16 = 6;
    pub const SIDE_SINCE_VERSION: u16 = 0;
    pub const SIDE_ENCODING_OFFSET: usize = 32;
    pub const SIDE_ENCODING_LENGTH: usize = 1;
    #[inline]
    pub const fn side_meta_attribute(
        attr: sbe_rt::MetaAttribute,
    ) -> Option<&'static str> {
        match attr {
            sbe_rt::MetaAttribute::Epoch => None,
            sbe_rt::MetaAttribute::TimeUnit => None,
            sbe_rt::MetaAttribute::SemanticType => None,
            sbe_rt::MetaAttribute::Presence => Some("required"),
        }
    }
    /// Compute the exact SBE message body length before encoding.
    /// Parameters: one `usize` per group (entry count) and one `usize`
    /// per var-data field (byte length).
    #[inline]
    pub const fn compute_encoded_length(symbol_len: usize) -> usize {
        let mut len = 33;
        len += 4 + symbol_len;
        len
    }
    /// Compute the exact SBE message length including the schema-declared
    /// message header.
    #[inline]
    pub const fn compute_encoded_length_with_message_header(symbol_len: usize) -> usize {
        8usize + Self::compute_encoded_length(symbol_len)
    }
    /// Short alias for [`Self::compute_encoded_length_with_message_header`].
    #[inline]
    pub const fn compute_length_with_header(symbol_len: usize) -> usize {
        Self::compute_encoded_length_with_message_header(symbol_len)
    }
    /// Compute the exact SBE message body length with checked arithmetic.
    /// Group counts use the wire type (`u16` or `u8`); var-data lengths
    /// use `usize`.
    #[inline]
    pub fn try_compute_encoded_length(
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        let mut len: usize = 33;
        if symbol_len > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: symbol_len,
            });
        }
        len = match len.checked_add(4 as usize) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        len = match len.checked_add(symbol_len) {
            Some(v) => v,
            None => return Err(sbe_rt::EncodeError::EncodedLengthOverflow),
        };
        Ok(len)
    }
    /// Compute the exact SBE message length including the header, with
    /// checked arithmetic.
    #[inline]
    pub fn try_compute_encoded_length_with_header(
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        let body = Self::try_compute_encoded_length(symbol_len)?;
        body.checked_add(8usize).ok_or(sbe_rt::EncodeError::EncodedLengthOverflow)
    }
    /// Short alias for [`Self::try_compute_encoded_length_with_header`].
    #[inline]
    pub fn try_compute_length_with_header(
        symbol_len: usize,
    ) -> Result<usize, sbe_rt::EncodeError> {
        Self::try_compute_encoded_length_with_header(symbol_len)
    }
}
impl<'a, H: sbe_rt::HeaderState> PriceLevelUpdateEncoder<'a, H> {
    #[inline]
    pub fn received_at_wire(&mut self, val: UTCTimestampMicrosEncoding) -> &mut Self {
        let offset = self.msg_offset + 8;
        self.buf[offset..offset + 8].copy_from_slice(&val.0);
        self
    }
    #[inline]
    pub fn exchange_timestamp_wire(
        &mut self,
        val: UTCTimestampMicrosEncoding,
    ) -> &mut Self {
        let offset = self.msg_offset + 16;
        self.buf[offset..offset + 8].copy_from_slice(&val.0);
        self
    }
    #[inline]
    pub fn price_level(&mut self, val: i64) -> &mut Self {
        let offset = self.msg_offset + 24;
        unsafe {
            self.buf
                .get_unchecked_mut(offset..offset + 8)
                .copy_from_slice(&val.to_le_bytes());
        }
        self
    }
    #[inline]
    pub fn quantity(&mut self, val: i64) -> &mut Self {
        let offset = self.msg_offset + 32;
        unsafe {
            self.buf
                .get_unchecked_mut(offset..offset + 8)
                .copy_from_slice(&val.to_le_bytes());
        }
        self
    }
    #[inline]
    pub fn side_wire(&mut self, val: SideEncoding) -> &mut Self {
        let offset = self.msg_offset + 40;
        self.buf[offset..offset + 1].copy_from_slice(&val.0);
        self
    }
    /**Set all fixed fields at once from a [`PriceLevelUpdateFixedFields`] value.

Required fields are always written; optional fields are written when `Some`. Returns the encoder for tail methods.*/
    #[inline]
    #[must_use]
    pub fn fixed(mut self, fixed: &PriceLevelUpdateFixedFields) -> Self {
        self.received_at_wire(fixed.received_at);
        self.exchange_timestamp_wire(fixed.exchange_timestamp);
        self.price_level(fixed.price_level);
        self.quantity(fixed.quantity);
        self.side_wire(fixed.side);
        self
    }
    ///Return a dedicated raw fixed-field writer. All individual field setters are available on the writer. To advance to tail stages, collect the values into a [`PriceLevelUpdateFixedFields`] and call `fixed()`.
    #[inline]
    #[must_use]
    pub fn raw_fixed(self) -> PriceLevelUpdateRawFixedWriter<'a> {
        let body_start = self.msg_offset + 8;
        PriceLevelUpdateRawFixedWriter {
            buf: &mut self.buf[body_start..],
            msg_offset: 0,
            offset: self.offset - body_start,
        }
    }
}
/// Buffer-placement metadata. Holds a reference to the parent encoder
/// — zero-copy. Utility methods live here so no schema field can
/// collide with them.
#[derive(Clone, Copy)]
pub struct PriceLevelUpdateEncoderMetadata<
    'm,
    'a,
    H: sbe_rt::HeaderState = sbe_rt::HeaderPresent,
> {
    encoder: &'m PriceLevelUpdateEncoder<'a, H>,
}
impl<'m, 'a, H: sbe_rt::HeaderState> PriceLevelUpdateEncoderMetadata<'m, 'a, H> {
    /// Fixed-block body bytes only (groups/var-data not yet written).
    /// For a complete frame use the terminal stage's
    /// `as_bytes_with_header`.
    #[inline]
    pub fn as_fixed_body_bytes(&self) -> &[u8] {
        &self.encoder.buf[self.encoder.msg_offset + 8..self.encoder.offset]
    }
    /// Header + fixed block only — **not** a complete SBE message when
    /// groups or var-data remain. Prefer the complete stage's
    /// `as_bytes_with_header`.
    #[inline]
    pub fn as_fixed_region_with_header(&self) -> &[u8] {
        &self.encoder.buf[self.encoder.msg_offset..self.encoder.offset]
    }
    /// Absolute offset of this message within the original buffer
    /// (the `msg_offset` argument passed to `wrap`).
    #[inline]
    pub const fn message_offset(&self) -> usize {
        self.encoder.msg_offset
    }
    /// Absolute current write cursor within the original buffer.
    #[inline]
    pub const fn limit(&self) -> usize {
        self.encoder.offset
    }
    /// The complete original buffer this encoder wraps.
    #[inline]
    pub const fn buffer(&self) -> &[u8] {
        self.encoder.buf
    }
}
impl<'a, H: sbe_rt::HeaderState> PriceLevelUpdateEncoder<'a, H> {
    /// Metadata accessor: buffer positions, wire-frame boundaries.
    /// Returns a zero-copy reference to the parent encoder.
    /// All utility methods are scoped here so no schema field name
    /// can collide with them.
    #[inline]
    pub fn get_metadata(&self) -> PriceLevelUpdateEncoderMetadata<'_, 'a, H> {
        PriceLevelUpdateEncoderMetadata {
            encoder: self,
        }
    }
}
impl<'a, H: sbe_rt::HeaderState> PriceLevelUpdateEncoder<'a, H> {
    #[inline]
    #[must_use]
    pub fn symbol(
        mut self,
        data: &[u8],
    ) -> Result<PriceLevelUpdateComplete<'a, H>, sbe_rt::EncodeError> {
        if data.len() > 1073741824 {
            return Err(sbe_rt::EncodeError::VarDataTooLong {
                field: "symbol",
                max_length: 1073741824,
                actual: data.len(),
            });
        }
        let needed = 4 + data.len();
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: stringify!(symbol),
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let wire_length = <u32>::try_from(data.len())
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: data.len(),
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        self.buf[start..start + data.len()].copy_from_slice(data);
        Ok(PriceLevelUpdateComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + data.len(),
            _header: core::marker::PhantomData,
        })
    }
    #[inline]
    #[must_use]
    pub fn symbol_unchecked(
        mut self,
        data: &[u8],
    ) -> Result<PriceLevelUpdateComplete<'a, H>, sbe_rt::EncodeError> {
        let needed = 4 + data.len();
        if self.offset + needed > self.buf.len() {
            return Err(sbe_rt::EncodeError::BufferTooShort {
                field: stringify!(symbol),
                needed,
                available: self.buf.len().saturating_sub(self.offset),
            });
        }
        let wire_length = <u32>::try_from(data.len())
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: data.len(),
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        self.buf[start..start + data.len()].copy_from_slice(data);
        Ok(PriceLevelUpdateComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + data.len(),
            _header: core::marker::PhantomData,
        })
    }
    /// Lend exactly `exact_len` bytes of the var-data region
    /// to a closure for nested-message encoding. Zero-copy:
    /// the closure writes directly into the outer buffer.
    ///
    /// Canonical nested-SBE pattern (AppMessage → L2Book):
    /// ```text
    /// let inner_len = InnerEncoder::compute_length_with_header(...);
    /// after.payload_with(inner_len, |payload| {
    ///     let len = InnerEncoder::wrap_and_apply_header(payload, 0)?
    ///         .field(value)
    ///         // continue the single encoder chain through all tail stages
    ///         .encoded_length_with_header();
    ///     debug_assert_eq!(len, inner_len);
    ///     Ok(())
    /// })?;
    /// ```
    /// Returns the next stage on success; on failure the
    /// caller error propagates unchanged and no partial
    /// data is published.
    #[inline]
    #[must_use]
    pub fn symbol_with<E, F>(
        mut self,
        exact_len: usize,
        f: F,
    ) -> Result<PriceLevelUpdateComplete<'a, H>, E>
    where
        E: From<sbe_rt::EncodeError>,
        F: FnOnce(&mut [u8]) -> Result<(), E>,
    {
        if exact_len > 1073741824 {
            return Err(
                sbe_rt::EncodeError::VarDataTooLong {
                    field: "symbol",
                    max_length: 1073741824,
                    actual: exact_len,
                }
                    .into(),
            );
        }
        let needed = 4 + exact_len;
        if self.offset + needed > self.buf.len() {
            return Err(
                sbe_rt::EncodeError::BufferTooShort {
                    field: stringify!(symbol),
                    needed,
                    available: self.buf.len().saturating_sub(self.offset),
                }
                    .into(),
            );
        }
        let wire_length = <u32>::try_from(exact_len)
            .map_err(|_| {
                sbe_rt::EncodeError::VarDataTooLong {
                    field: stringify!(symbol),
                    max_length: <u32>::MAX as usize,
                    actual: exact_len,
                }
            })?;
        let len_bytes = wire_length.to_le_bytes();
        self.buf[self.offset..self.offset + 4].copy_from_slice(&len_bytes);
        let start = self.offset + 4;
        f(&mut self.buf[start..start + exact_len])?;
        Ok(PriceLevelUpdateComplete {
            buf: self.buf,
            msg_offset: self.msg_offset,
            offset: start + exact_len,
            _header: core::marker::PhantomData,
        })
    }
}
impl<'a, H: sbe_rt::HeaderState> PriceLevelUpdateComplete<'a, H> {
    /// SBE message body bytes (excluding the message header).
    #[inline]
    pub fn as_body_bytes(&self) -> &[u8] {
        let body_start = self.msg_offset + 8;
        &self.buf[body_start..self.offset]
    }
    /// SBE message body length (excluding the message header).
    #[inline]
    pub fn encoded_length(&self) -> usize {
        self.offset - self.msg_offset - 8
    }
    /// Total SBE message length including the header region.
    /// Pure arithmetic — available for body-only wraps too.
    #[inline]
    pub fn encoded_length_with_header(&self) -> usize {
        self.offset - self.msg_offset
    }
    /// Unwritten region after this message's write cursor to the end of
    /// the original buffer. Use for multi-message packing, e.g.
    /// `NextEncoder::wrap_and_apply_header(remaining, 0)`. This is **not**
    /// the payload of the current message — for the absolute write
    /// cursor while keeping the encoder alive, use
    /// `get_metadata().limit()`.
    #[inline]
    pub fn into_remaining_mut(self) -> &'a mut [u8] {
        &mut self.buf[self.offset..]
    }
}
impl<'a> PriceLevelUpdateComplete<'a, sbe_rt::HeaderPresent> {
    /// Header-inclusive bytes. Only available when the encoder was
    /// constructed via `wrap_and_apply_header` (not raw `wrap`).
    #[inline]
    pub fn as_bytes_with_header(&self) -> &[u8] {
        &self.buf[self.msg_offset..self.offset]
    }
}
impl<'a> __sbe_message_sealed::Sealed for PriceLevelUpdateEncoder<'a> {}
impl<'a> sbe_rt::SbeMessage for PriceLevelUpdateEncoder<'a> {
    const TEMPLATE_ID: u16 = 2;
    const BLOCK_LENGTH: usize = 33;
    const SCHEMA_ID: u16 = 1;
    const SCHEMA_VERSION: u16 = 2;
}
impl<'a> PriceLevelUpdateDecoder<'a> {
    #[inline]
    pub fn try_received_at(
        &self,
    ) -> Result<
        chrono::DateTime<chrono::Utc>,
        <chrono::DateTime<chrono::Utc> as TryFromSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        <chrono::DateTime<
            chrono::Utc,
        > as TryFromSbe<
            UTCTimestampMicrosEncoding,
        >>::try_from_sbe(self.received_at_value())
    }
    #[inline]
    pub fn try_exchange_timestamp(
        &self,
    ) -> Result<
        chrono::DateTime<chrono::Utc>,
        <chrono::DateTime<chrono::Utc> as TryFromSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        <chrono::DateTime<
            chrono::Utc,
        > as TryFromSbe<
            UTCTimestampMicrosEncoding,
        >>::try_from_sbe(self.exchange_timestamp_value())
    }
    #[inline]
    pub fn try_side(
        &self,
    ) -> Result<crate::Side, <crate::Side as TryFromSbe<SideEncoding>>::Error> {
        <crate::Side as TryFromSbe<SideEncoding>>::try_from_sbe(self.side_value())
    }
}
impl<'a, H: sbe_rt::HeaderState> PriceLevelUpdateEncoder<'a, H> {
    #[inline]
    pub fn try_received_at(
        &mut self,
        value: chrono::DateTime<chrono::Utc>,
    ) -> Result<
        &mut Self,
        <chrono::DateTime<chrono::Utc> as TryToSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        let wire = <chrono::DateTime<
            chrono::Utc,
        > as TryToSbe<UTCTimestampMicrosEncoding>>::try_to_sbe(&value)?;
        self.received_at_wire(wire);
        Ok(self)
    }
    #[inline]
    pub fn try_exchange_timestamp(
        &mut self,
        value: chrono::DateTime<chrono::Utc>,
    ) -> Result<
        &mut Self,
        <chrono::DateTime<chrono::Utc> as TryToSbe<UTCTimestampMicrosEncoding>>::Error,
    > {
        let wire = <chrono::DateTime<
            chrono::Utc,
        > as TryToSbe<UTCTimestampMicrosEncoding>>::try_to_sbe(&value)?;
        self.exchange_timestamp_wire(wire);
        Ok(self)
    }
    #[inline]
    pub fn try_side(
        &mut self,
        value: crate::Side,
    ) -> Result<&mut Self, <crate::Side as TryToSbe<SideEncoding>>::Error> {
        let wire = <crate::Side as TryToSbe<SideEncoding>>::try_to_sbe(&value)?;
        self.side_wire(wire);
        Ok(self)
    }
}
pub mod price_level_update_field_meta {
    pub struct FieldInfo {
        pub name: &'static str,
        pub id: u16,
        pub offset: usize,
        pub since_version: u16,
        pub field_type: &'static str,
        pub presence: &'static str,
        pub null_value: Option<&'static str>,
        pub semantic_type: Option<&'static str>,
        pub description: Option<&'static str>,
    }
    pub const FIELDS: &[FieldInfo] = &[
        FieldInfo {
            name: "received_at",
            id: 1,
            offset: 0,
            since_version: 0,
            field_type: "UTCTimestampMicrosEncoding",
            presence: "required",
            null_value: None,
            semantic_type: None,
            description: None,
        },
        FieldInfo {
            name: "exchange_timestamp",
            id: 2,
            offset: 8,
            since_version: 0,
            field_type: "UTCTimestampMicrosEncoding",
            presence: "required",
            null_value: None,
            semantic_type: None,
            description: None,
        },
        FieldInfo {
            name: "price_level",
            id: 4,
            offset: 16,
            since_version: 0,
            field_type: "i64",
            presence: "required",
            null_value: Some("9223372036854775808"),
            semantic_type: None,
            description: None,
        },
        FieldInfo {
            name: "quantity",
            id: 5,
            offset: 24,
            since_version: 0,
            field_type: "i64",
            presence: "required",
            null_value: Some("9223372036854775808"),
            semantic_type: None,
            description: None,
        },
        FieldInfo {
            name: "side",
            id: 6,
            offset: 32,
            since_version: 0,
            field_type: "SideEncoding",
            presence: "required",
            null_value: None,
            semantic_type: None,
            description: None,
        },
    ];
}
pub const SCHEMA_HASH: u64 = 12864351005427035222;
pub const SCHEMA_SHA256: [u8; 32] = [
    0x00, 0xfe, 0x01, 0xdb, 0xa9, 0x84, 0xcd, 0xd3, 0xf5, 0xc8, 0x68, 0x25, 0x16, 0x05,
    0x7c, 0x0e, 0xbf, 0xbf, 0x3c, 0x6b, 0xa7, 0x62, 0x5e, 0x34, 0xff, 0xf1, 0xc4, 0xa8,
    0xb3, 0xef, 0x14, 0x26,
];
pub const SCHEMA_SHA256_HEX: &str = "00fe01dba984cdd3f5c8682516057c0ebfbf3c6ba7625e34fff1c4a8b3ef1426";
pub const SCHEMA_ID: u16 = 1;
pub const SCHEMA_VERSION: u16 = 2;
pub mod prelude {
    pub use super::sbe_rt::{
        DecodeError, EncodeError, VerifyError, MetaAttribute, SbeMessage,
    };
    pub use super::{
        AnyMessage, DecodedFrame, FrameCursor, FramingPolicy, MessageVisitor,
        MessageHeader, MessageHeaderDecoder, GroupSizeEncoding, GroupSizeEncodingDecoder,
        VarStringEncoding, VarStringEncodingDecoder, UTCTimestampMicrosEncoding,
        UTCTimestampMicrosEncodingDecoder, UTCTimestampMicrosNullableEncoding,
        UTCTimestampMicrosNullableEncodingDecoder, SideEncoding, SideEncodingDecoder,
        BooleanType, SnapshotDecoder, SnapshotEncoder, PriceLevelUpdateDecoder,
        PriceLevelUpdateEncoder,
    };
}
/// Read `N` bytes from `buf` at `offset` into a fixed-size array.
///
/// Bounds-checked slice indexing. LLVM elides the check when the
/// slice length is known (stack buffer with visible size).
#[inline]
pub fn read_bytes<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
    buf[offset..offset + N].try_into().expect("read_bytes: buffer too short")
}
#[inline]
pub fn write_bytes<const N: usize>(buf: &mut [u8], offset: usize, bytes: &[u8; N]) {
    buf[offset..offset + N].copy_from_slice(bytes);
}
/// Unchecked companion to [`read_bytes`] — zero bounds checks.
///
/// # Safety
/// Caller guarantees `offset + N` does not overflow and
/// `offset + N <= buf.len()`.
#[inline]
#[allow(dead_code)]
unsafe fn read_bytes_unchecked<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
    unsafe { core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const [u8; N]) }
}
/// Unchecked companion to [`write_bytes`] — zero bounds checks.
///
/// # Safety
/// Caller guarantees `offset + N` does not overflow and
/// `offset + N <= buf.len()`.
#[inline]
#[allow(dead_code)]
unsafe fn write_bytes_unchecked<const N: usize>(
    buf: &mut [u8],
    offset: usize,
    bytes: &[u8; N],
) {
    unsafe {
        core::ptr::write_unaligned(buf.as_mut_ptr().add(offset) as *mut [u8; N], *bytes)
    }
}
#[inline]
pub fn schema_id_from_header(buf: &[u8]) -> Option<u16> {
    if buf.len() < 4 + 2 {
        return None;
    }
    let bytes = read_bytes::<2>(buf, 4);
    let value = u16::from_le_bytes(bytes) as u64;
    u16::try_from(value).ok()
}
/// Tagged union of every message type in the schema — decode once,
/// then `match` to access the typed decoder.
#[non_exhaustive]
pub enum AnyMessage<'a> {
    Snapshot(SnapshotDecoder<'a>),
    PriceLevelUpdate(PriceLevelUpdateDecoder<'a>),
    Unknown {
        header: MessageHeader,
        /// The complete frame: schema-declared message header
        /// followed by the unparsed body. Not the body alone.
        frame: &'a [u8],
    },
}
/// One decoded message with its buffer range and length.
pub struct DecodedFrame<'a> {
    pub message: AnyMessage<'a>,
    pub range: core::ops::Range<usize>,
    pub len: usize,
}
/// How frames are delimited in a stream: length-prefixed or fixed-size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramingPolicy {
    LengthPrefixU32Le,
    LengthPrefixU16Le,
    Fixed(usize),
}
/// Iterator that yields [`DecodedFrame`]s from a byte buffer according
/// to a [`FramingPolicy`].
pub struct FrameCursor<'a> {
    buf: &'a [u8],
    offset: usize,
    framing: FramingPolicy,
}
impl<'a> FrameCursor<'a> {
    #[inline]
    pub const fn new(buf: &'a [u8], framing: FramingPolicy) -> Self {
        Self { buf, offset: 0, framing }
    }
}
impl<'a> core::iter::FusedIterator for FrameCursor<'a> {}
impl<'a> Iterator for FrameCursor<'a> {
    type Item = Result<DecodedFrame<'a>, sbe_rt::DecodeError>;
    /// Fused after the first error.
    ///
    /// A framing error means the boundary between this frame and the
    /// next is no longer known, so every later offset would be a guess.
    /// The cursor moves to the terminal boundary instead: one `Err`,
    /// then permanent `None`. Re-polling a broken stream used to return
    /// the same error forever.
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buf.len() {
            return None;
        }
        let terminal = self.buf.len();
        let (header_len, frame_len) = match self.framing {
            FramingPolicy::LengthPrefixU32Le => {
                if 4 > self.buf.len().saturating_sub(self.offset) {
                    let available = self.buf.len().saturating_sub(self.offset);
                    self.offset = terminal;
                    return Some(
                        Err(sbe_rt::DecodeError::BufferTooShort {
                            field: "length prefix",
                            needed: 4,
                            available,
                        }),
                    );
                }
                let bytes: [u8; 4] = read_bytes::<4>(self.buf, self.offset);
                let len = u32::from_le_bytes(bytes) as usize;
                (4, len)
            }
            FramingPolicy::LengthPrefixU16Le => {
                if 2 > self.buf.len().saturating_sub(self.offset) {
                    let available = self.buf.len().saturating_sub(self.offset);
                    self.offset = terminal;
                    return Some(
                        Err(sbe_rt::DecodeError::BufferTooShort {
                            field: "length prefix",
                            needed: 2,
                            available,
                        }),
                    );
                }
                let bytes: [u8; 2] = read_bytes::<2>(self.buf, self.offset);
                let len = u16::from_le_bytes(bytes) as usize;
                (2, len)
            }
            FramingPolicy::Fixed(len) => (0, len),
        };
        let available = self.buf.len().saturating_sub(self.offset);
        let frame_start = match self.offset.checked_add(header_len) {
            Some(value) => value,
            None => {
                self.offset = terminal;
                return Some(
                    Err(sbe_rt::DecodeError::BufferTooShort {
                        field: "frame bounds",
                        needed: usize::MAX,
                        available,
                    }),
                );
            }
        };
        let frame_end = match frame_start.checked_add(frame_len) {
            Some(value) => value,
            None => {
                self.offset = terminal;
                return Some(
                    Err(sbe_rt::DecodeError::BufferTooShort {
                        field: "frame bounds",
                        needed: usize::MAX,
                        available,
                    }),
                );
            }
        };
        if frame_end > self.buf.len() {
            self.offset = terminal;
            return Some(
                Err(sbe_rt::DecodeError::BufferTooShort {
                    field: "frame bounds",
                    needed: header_len.saturating_add(frame_len),
                    available,
                }),
            );
        }
        match AnyMessage::decode_frame(self.buf, frame_start, frame_len) {
            Ok(frame) => {
                self.offset = frame_end;
                Some(Ok(frame))
            }
            Err(e) => {
                self.offset = terminal;
                Some(Err(e))
            }
        }
    }
}
impl<'a> AnyMessage<'a> {
    /// Dispatch a framed message with header + version-aware fixed-extent checks.
    #[inline]
    pub fn try_decode(
        buf: &'a [u8],
        offset: usize,
    ) -> Result<Self, sbe_rt::DecodeError> {
        if 8 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: 8,
                available: buf.len().saturating_sub(offset),
            });
        }
        let header_bytes = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        let version = sbe_rt::checked_header_u16("version", header.version() as u64)?;
        let block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        if schema_id != 1 {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: 1,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        match template_id {
            SnapshotSchema::TEMPLATE_ID => {
                Ok(
                    Self::Snapshot(
                        SnapshotDecoder::try_wrap(buf, offset, block_length, version)?,
                    ),
                )
            }
            PriceLevelUpdateSchema::TEMPLATE_ID => {
                Ok(
                    Self::PriceLevelUpdate(
                        PriceLevelUpdateDecoder::try_wrap(
                            buf,
                            offset,
                            block_length,
                            version,
                        )?,
                    ),
                )
            }
            _ => {
                Err(sbe_rt::DecodeError::UnknownTemplateLength {
                    template_id,
                })
            }
        }
    }
    /// Trusted multi-template dispatch. Same checks as
    /// [`Self::try_decode`]; prefer `try_decode` at untrusted
    /// boundaries. Dynamic tails remain checked on consume.
    #[inline]
    pub fn decode(buf: &'a [u8], offset: usize) -> Result<Self, sbe_rt::DecodeError> {
        Self::try_decode(buf, offset)
    }
}
impl<'a> AnyMessage<'a> {
    #[inline]
    pub fn decode_frame(
        buf: &'a [u8],
        offset: usize,
        frame_len: usize,
    ) -> Result<DecodedFrame<'a>, sbe_rt::DecodeError> {
        if 8 > buf.len().saturating_sub(offset) {
            return Err(sbe_rt::DecodeError::BufferTooShort {
                field: "message header",
                needed: 8,
                available: buf.len().saturating_sub(offset),
            });
        }
        let header_bytes: [u8; 8] = read_bytes::<8>(buf, offset);
        let header = MessageHeader(header_bytes);
        let template_id = sbe_rt::checked_header_u16(
            "templateId",
            header.template_id() as u64,
        )?;
        let schema_id = sbe_rt::checked_header_u16(
            "schemaId",
            header.schema_id() as u64,
        )?;
        let version = sbe_rt::checked_header_u16("version", header.version() as u64)?;
        let block_length = sbe_rt::checked_header_usize(
            "blockLength",
            header.block_length() as u64,
        )?;
        let body_offset = offset + 8;
        if schema_id != 1 {
            return Err(sbe_rt::DecodeError::WrongSchema {
                expected: 1,
                actual: schema_id,
                expected_name: "challenge_orderbook_messages",
            });
        }
        match template_id {
            SnapshotSchema::TEMPLATE_ID => {
                let frame_end = offset
                    .checked_add(frame_len)
                    .ok_or(sbe_rt::DecodeError::BufferTooShort {
                        field: "Snapshot",
                        needed: frame_len,
                        available: buf.len().saturating_sub(offset),
                    })?;
                if frame_end > buf.len() {
                    return Err(sbe_rt::DecodeError::BufferTooShort {
                        field: "Snapshot",
                        needed: frame_len,
                        available: buf.len().saturating_sub(offset),
                    });
                }
                let decoder = SnapshotDecoder::try_decode(&buf[..frame_end], offset)?;
                Ok(DecodedFrame {
                    message: Self::Snapshot(decoder),
                    range: offset..frame_end,
                    len: frame_len,
                })
            }
            PriceLevelUpdateSchema::TEMPLATE_ID => {
                let frame_end = offset
                    .checked_add(frame_len)
                    .ok_or(sbe_rt::DecodeError::BufferTooShort {
                        field: "PriceLevelUpdate",
                        needed: frame_len,
                        available: buf.len().saturating_sub(offset),
                    })?;
                if frame_end > buf.len() {
                    return Err(sbe_rt::DecodeError::BufferTooShort {
                        field: "PriceLevelUpdate",
                        needed: frame_len,
                        available: buf.len().saturating_sub(offset),
                    });
                }
                let decoder = PriceLevelUpdateDecoder::try_decode(
                    &buf[..frame_end],
                    offset,
                )?;
                Ok(DecodedFrame {
                    message: Self::PriceLevelUpdate(decoder),
                    range: offset..frame_end,
                    len: frame_len,
                })
            }
            _ => {
                if frame_len > buf.len().saturating_sub(offset) {
                    return Err(sbe_rt::DecodeError::BufferTooShort {
                        field: "template body",
                        needed: frame_len,
                        available: buf.len().saturating_sub(offset),
                    });
                }
                let frame = &buf[offset..offset + frame_len];
                Ok(DecodedFrame {
                    message: Self::Unknown { header, frame },
                    range: offset..offset + frame_len,
                    len: frame_len,
                })
            }
        }
    }
}
impl<'a> AnyMessage<'a> {
    #[inline]
    pub fn encoded_length_with_header(&self) -> Result<usize, sbe_rt::DecodeError> {
        match self {
            Self::Snapshot(d) => d.encoded_length_with_header(),
            Self::PriceLevelUpdate(d) => d.encoded_length_with_header(),
            Self::Unknown { frame, .. } => Ok(frame.len()),
        }
    }
}
impl<'a> AnyMessage<'a> {
    /// Complete SBE frame (message header + body) — for
    /// [`Self::Unknown`] this is the same header-plus-body range
    /// the cursor matched.
    #[inline]
    pub fn as_bytes(&self) -> Result<&'a [u8], sbe_rt::DecodeError> {
        match self {
            Self::Snapshot(d) => d.as_bytes_with_header(),
            Self::PriceLevelUpdate(d) => d.as_bytes_with_header(),
            Self::Unknown { frame, .. } => Ok(frame),
        }
    }
}
impl<'a> AnyMessage<'a> {
    #[inline]
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, sbe_rt::EncodeError> {
        match self {
            Self::Snapshot(d) => {
                let len = d.encoded_length_with_header()?;
                if len > buf.len() {
                    return Err(sbe_rt::EncodeError::BufferTooShort {
                        field: "AnyMessage::encode",
                        needed: len,
                        available: buf.len(),
                    });
                }
                let bytes = d.as_bytes_with_header()?;
                buf[..len].copy_from_slice(bytes);
                Ok(len)
            }
            Self::PriceLevelUpdate(d) => {
                let len = d.encoded_length_with_header()?;
                if len > buf.len() {
                    return Err(sbe_rt::EncodeError::BufferTooShort {
                        field: "AnyMessage::encode",
                        needed: len,
                        available: buf.len(),
                    });
                }
                let bytes = d.as_bytes_with_header()?;
                buf[..len].copy_from_slice(bytes);
                Ok(len)
            }
            Self::Unknown { frame, .. } => {
                if frame.len() > buf.len() {
                    return Err(sbe_rt::EncodeError::BufferTooShort {
                        field: "AnyMessage::encode",
                        needed: frame.len(),
                        available: buf.len(),
                    });
                }
                buf[..frame.len()].copy_from_slice(frame);
                Ok(frame.len())
            }
        }
    }
}
pub trait MessageVisitor {
    type Output;
    fn visit_snapshot(&mut self, decoder: &SnapshotDecoder<'_>) -> Self::Output;
    fn visit_price_level_update(
        &mut self,
        decoder: &PriceLevelUpdateDecoder<'_>,
    ) -> Self::Output;
    /// Called for unknown template IDs (not in this schema).
    ///
    /// `header` is the parsed schema-declared MessageHeader.
    /// `frame` is the complete frame — message header followed by
    /// the unparsed body — not the body alone. Must be implemented;
    /// there is no panicking default, because an unknown template
    /// is application policy rather than a crash.
    fn visit_unknown(&mut self, header: &MessageHeader, frame: &[u8]) -> Self::Output;
}
impl<'a> AnyMessage<'a> {
    #[inline]
    pub fn visit<V: MessageVisitor>(&self, visitor: &mut V) -> V::Output {
        match self {
            Self::Snapshot(d) => visitor.visit_snapshot(d),
            Self::PriceLevelUpdate(d) => visitor.visit_price_level_update(d),
            Self::Unknown { header, frame } => visitor.visit_unknown(header, frame),
        }
    }
}
