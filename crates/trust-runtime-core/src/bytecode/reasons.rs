//! Typed section rejection reasons with backward-compatible diagnostics.

use super::BytecodeError;
use core::fmt;

/// Named section validation failures. Conversion preserves the existing error variant,
/// stable machine code and diagnostic text; callers need not write string literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Internal validator flow work item has no retained entry state.
    MissingBlockEntryState,
    /// Type kind and its payload variant disagree.
    TypePayloadMismatch,
    /// Section rejection: arithmetic opcode expects numeric operands.
    ArithmeticOpcodeExpectsNumericOperands,
    /// Section rejection: array constant size overflow.
    ArrayConstantSizeOverflow,
    /// Section rejection: CALL_NATIVE arg_count out of range.
    CallNativeArgCountOutOfRange,
    /// Section rejection: CALL_NATIVE kind out of range.
    CallNativeKindOutOfRange,
    /// Section rejection: POU code position exceeds signed jump representation.
    CodePositionOverflow,
    /// Section rejection: conditional jump expects BOOL operand.
    ConditionalJumpExpectsBoolOperand,
    /// Section rejection: const child payload length.
    ConstChildPayloadLength,
    /// Section rejection: const payload length.
    ConstPayloadLength,
    /// Section rejection: const type recursion overflow.
    ConstTypeRecursionOverflow,
    /// Section rejection: constant type is incompatible with STORE_REF target.
    ConstantTypeIsIncompatibleWithStoreRefTarget,
    /// Section rejection: debug map code offset out of bounds.
    DebugMapCodeOffsetOutOfBounds,
    /// Section rejection: decoded module instruction count overflow.
    DecodedModuleInstructionCountOverflow,
    /// Section rejection: duplicate VAR_META name.
    DuplicateVarMetaName,
    /// Section rejection: dynamic load expects reference operand.
    DynamicLoadExpectsReferenceOperand,
    /// Section rejection: dynamic store expects reference operand.
    DynamicStoreExpectsReferenceOperand,
    /// Section rejection: field reference expects reference or instance operand.
    FieldReferenceExpectsReferenceOrInstanceOperand,
    /// Section rejection: frame-local reference cannot be stored through non-local reference.
    FrameLocalReferenceCannotBeStoredThroughNonLocalReference,
    /// Section rejection: frame-local reference cannot be stored to longer-lived storage.
    FrameLocalReferenceCannotBeStoredToLongerLivedStorage,
    /// Section rejection: inconsistent operand stack depth at control-flow merge.
    InconsistentOperandStackDepthAtControlFlowMerge,
    /// Section rejection: indexed reference expects numeric index operand.
    IndexedReferenceExpectsNumericIndexOperand,
    /// Section rejection: indexed reference expects reference operand.
    IndexedReferenceExpectsReferenceOperand,
    /// Section rejection: interface mapping expects interface type.
    InterfaceMappingExpectsInterfaceType,
    /// Section rejection: interface mapping slot mismatch.
    InterfaceMappingSlotMismatch,
    /// Section rejection: invalid array bounds.
    InvalidArrayBounds,
    /// Section rejection: invalid IO area.
    InvalidIoArea,
    /// Section rejection: invalid local VAR_META POU id.
    InvalidLocalVarMetaPouId,
    /// Section rejection: invalid local VAR_META slot.
    InvalidLocalVarMetaSlot,
    /// Section rejection: invalid pou kind.
    InvalidPouKind,
    /// Section rejection: invalid ref location.
    InvalidRefLocation,
    /// Section rejection: invalid ref segment.
    InvalidRefSegment,
    /// Section rejection: invalid retain policy.
    InvalidRetainPolicy,
    /// Section rejection: invalid type kind.
    InvalidTypeKind,
    /// Section rejection: invalid utf-8.
    InvalidUtf8,
    /// Section rejection: invalid WSTRING const payload length.
    InvalidWstringConstPayloadLength,
    /// Section rejection: unsupported legacy CALL opcode 0x05; use CALL_NATIVE.
    LegacyCall,
    /// Section rejection: local ref outside POU local range.
    LocalRefOutsidePouLocalRange,
    /// Section rejection: local VAR_META must describe a base local ref.
    LocalVarMetaMustDescribeABaseLocalRef,
    /// Section rejection: local VAR_META name is missing its display label.
    LocalVarMetaNameIsMissingItsDisplayLabel,
    /// Section rejection: local VAR_META name must use the reserved @local scope.
    LocalVarMetaNameMustUseTheReservedLocalScope,
    /// Section rejection: local VAR_META scope does not match its POU local ref.
    LocalVarMetaScopeDoesNotMatchItsPouLocalRef,
    /// Section rejection: local VAR_META type disagrees with the POU signature.
    LocalVarMetaTypeDisagreesWithThePouSignature,
    /// Section rejection: POU body references multiple instance owners.
    MultipleInstanceOwners,
    /// Section rejection: operand stack underflow on DUP.
    OperandStackUnderflowOnDup,
    /// Section rejection: operand stack underflow on SWAP.
    OperandStackUnderflowOnSwap,
    /// Section rejection: partial-access index out of range.
    PartialAccessIndexOutOfRange,
    /// Section rejection: partial-access operand out of range.
    PartialAccessOperandOutOfRange,
    /// Section rejection: POU body leaves values on operand stack.
    PouBodyLeavesValuesOnOperandStack,
    /// Section rejection: POU code out of bounds.
    PouCodeOutOfBounds,
    /// Section rejection: POU code range overflow.
    PouCodeRangeOverflow,
    /// Section rejection: POU_INDEX local reference count exceeds fixed resource limit.
    PouIndexLocalReferenceCountExceedsFixedResourceLimit,
    /// Section rejection: POU local ref range contains multiple frame owners.
    PouLocalRefRangeContainsMultipleFrameOwners,
    /// Section rejection: POU local ref range contains non-contiguous local offset.
    PouLocalRefRangeContainsNonContiguousLocalOffset,
    /// Section rejection: POU local ref range contains non-local ref.
    PouLocalRefRangeContainsNonLocalRef,
    /// Section rejection: POU local ref range contains path ref.
    PouLocalRefRangeContainsPathRef,
    /// Section rejection: POU local ref range out of bounds.
    PouLocalRefRangeOutOfBounds,
    /// Section rejection: POU local ref range overflow.
    PouLocalRefRangeOverflow,
    /// Section rejection: POU local ref ranges overlap.
    PouLocalRefRangesOverlap,
    /// Section rejection: POU local ref ranges share a frame owner.
    PouLocalRefRangesShareAFrameOwner,
    /// Section rejection: REFERENCE_ATTEMPT expects reference, interface instance, or NULL operand.
    ReferenceAttemptExpectsReferenceInterfaceInstanceOrNullOperand,
    /// Section rejection: REFERENCE_ATTEMPT expects reference or interface target type.
    ReferenceAttemptExpectsReferenceOrInterfaceTargetType,
    /// Section rejection: REFERENCE const payload must encode NULL.
    ReferenceConstPayloadMustEncodeNull,
    /// Section rejection: reserved local VAR_META name requires a local ref.
    ReservedLocalVarMetaNameRequiresALocalRef,
    /// Section rejection: section id and payload disagree.
    SectionPayloadMismatch,
    /// Section rejection: struct/union constant count mismatch.
    StructUnionConstantCountMismatch,
    /// Section rejection: type entry length mismatch.
    TypeEntryLengthMismatch,
    /// Section rejection: type reference recursion overflow.
    TypeReferenceRecursionOverflow,
    /// Section rejection: type table offset out of bounds.
    TypeTableOffsetOutOfBounds,
    /// Section rejection: type table offsets not sorted.
    TypeTableOffsetsNotSorted,
    /// Section rejection: unknown primitive.
    UnknownPrimitive,
    /// Section rejection: unsupported const type.
    UnsupportedConstType,
    /// Section rejection: validation analysis storage limit exceeded.
    ValidationStorageLimit,
    /// Section rejection: validation analysis work limit exceeded.
    ValidationWorkLimit,
}

impl RejectionReason {
    /// Existing human-readable diagnostic for this reason.
    pub const fn message(self) -> &'static str {
        match self {
            Self::TypePayloadMismatch => "type kind and payload disagree",
            Self::ArithmeticOpcodeExpectsNumericOperands => {
                "arithmetic opcode expects numeric operands"
            }
            Self::ArrayConstantSizeOverflow => "array constant size overflow",
            Self::CallNativeArgCountOutOfRange => "CALL_NATIVE arg_count out of range",
            Self::CallNativeKindOutOfRange => "CALL_NATIVE kind out of range",
            Self::MissingBlockEntryState => "missing validator block entry state",
            Self::CodePositionOverflow => "POU code position exceeds signed jump representation",
            Self::ConditionalJumpExpectsBoolOperand => "conditional jump expects BOOL operand",
            Self::ConstChildPayloadLength => "const child payload length",
            Self::ConstPayloadLength => "const payload length",
            Self::ConstTypeRecursionOverflow => "const type recursion overflow",
            Self::ConstantTypeIsIncompatibleWithStoreRefTarget => {
                "constant type is incompatible with STORE_REF target"
            }
            Self::DebugMapCodeOffsetOutOfBounds => "debug map code offset out of bounds",
            Self::DecodedModuleInstructionCountOverflow => {
                "decoded module instruction count overflow"
            }
            Self::DuplicateVarMetaName => "duplicate VAR_META name",
            Self::DynamicLoadExpectsReferenceOperand => "dynamic load expects reference operand",
            Self::DynamicStoreExpectsReferenceOperand => "dynamic store expects reference operand",
            Self::FieldReferenceExpectsReferenceOrInstanceOperand => {
                "field reference expects reference or instance operand"
            }
            Self::FrameLocalReferenceCannotBeStoredThroughNonLocalReference => {
                "frame-local reference cannot be stored through non-local reference"
            }
            Self::FrameLocalReferenceCannotBeStoredToLongerLivedStorage => {
                "frame-local reference cannot be stored to longer-lived storage"
            }
            Self::InconsistentOperandStackDepthAtControlFlowMerge => {
                "inconsistent operand stack depth at control-flow merge"
            }
            Self::IndexedReferenceExpectsNumericIndexOperand => {
                "indexed reference expects numeric index operand"
            }
            Self::IndexedReferenceExpectsReferenceOperand => {
                "indexed reference expects reference operand"
            }
            Self::InterfaceMappingExpectsInterfaceType => {
                "interface mapping expects interface type"
            }
            Self::InterfaceMappingSlotMismatch => "interface mapping slot mismatch",
            Self::InvalidArrayBounds => "invalid array bounds",
            Self::InvalidIoArea => "invalid IO area",
            Self::InvalidLocalVarMetaPouId => "invalid local VAR_META POU id",
            Self::InvalidLocalVarMetaSlot => "invalid local VAR_META slot",
            Self::InvalidPouKind => "invalid pou kind",
            Self::InvalidRefLocation => "invalid ref location",
            Self::InvalidRefSegment => "invalid ref segment",
            Self::InvalidRetainPolicy => "invalid retain policy",
            Self::InvalidTypeKind => "invalid type kind",
            Self::InvalidUtf8 => "invalid utf-8",
            Self::InvalidWstringConstPayloadLength => "invalid WSTRING const payload length",
            Self::LegacyCall => "unsupported legacy CALL opcode 0x05; use CALL_NATIVE",
            Self::LocalRefOutsidePouLocalRange => "local ref outside POU local range",
            Self::LocalVarMetaMustDescribeABaseLocalRef => {
                "local VAR_META must describe a base local ref"
            }
            Self::LocalVarMetaNameIsMissingItsDisplayLabel => {
                "local VAR_META name is missing its display label"
            }
            Self::LocalVarMetaNameMustUseTheReservedLocalScope => {
                "local VAR_META name must use the reserved @local scope"
            }
            Self::LocalVarMetaScopeDoesNotMatchItsPouLocalRef => {
                "local VAR_META scope does not match its POU local ref"
            }
            Self::LocalVarMetaTypeDisagreesWithThePouSignature => {
                "local VAR_META type disagrees with the POU signature"
            }
            Self::MultipleInstanceOwners => "POU body references multiple instance owners",
            Self::OperandStackUnderflowOnDup => "operand stack underflow on DUP",
            Self::OperandStackUnderflowOnSwap => "operand stack underflow on SWAP",
            Self::PartialAccessIndexOutOfRange => "partial-access index out of range",
            Self::PartialAccessOperandOutOfRange => "partial-access operand out of range",
            Self::PouBodyLeavesValuesOnOperandStack => "POU body leaves values on operand stack",
            Self::PouCodeOutOfBounds => "POU code out of bounds",
            Self::PouCodeRangeOverflow => "POU code range overflow",
            Self::PouIndexLocalReferenceCountExceedsFixedResourceLimit => {
                "POU_INDEX local reference count exceeds fixed resource limit"
            }
            Self::PouLocalRefRangeContainsMultipleFrameOwners => {
                "POU local ref range contains multiple frame owners"
            }
            Self::PouLocalRefRangeContainsNonContiguousLocalOffset => {
                "POU local ref range contains non-contiguous local offset"
            }
            Self::PouLocalRefRangeContainsNonLocalRef => {
                "POU local ref range contains non-local ref"
            }
            Self::PouLocalRefRangeContainsPathRef => "POU local ref range contains path ref",
            Self::PouLocalRefRangeOutOfBounds => "POU local ref range out of bounds",
            Self::PouLocalRefRangeOverflow => "POU local ref range overflow",
            Self::PouLocalRefRangesOverlap => "POU local ref ranges overlap",
            Self::PouLocalRefRangesShareAFrameOwner => "POU local ref ranges share a frame owner",
            Self::ReferenceAttemptExpectsReferenceInterfaceInstanceOrNullOperand => {
                "REFERENCE_ATTEMPT expects reference, interface instance, or NULL operand"
            }
            Self::ReferenceAttemptExpectsReferenceOrInterfaceTargetType => {
                "REFERENCE_ATTEMPT expects reference or interface target type"
            }
            Self::ReferenceConstPayloadMustEncodeNull => "REFERENCE const payload must encode NULL",
            Self::ReservedLocalVarMetaNameRequiresALocalRef => {
                "reserved local VAR_META name requires a local ref"
            }
            Self::SectionPayloadMismatch => "section id and payload disagree",
            Self::StructUnionConstantCountMismatch => "struct/union constant count mismatch",
            Self::TypeEntryLengthMismatch => "type entry length mismatch",
            Self::TypeReferenceRecursionOverflow => "type reference recursion overflow",
            Self::TypeTableOffsetOutOfBounds => "type table offset out of bounds",
            Self::TypeTableOffsetsNotSorted => "type table offsets not sorted",
            Self::UnknownPrimitive => "unknown primitive",
            Self::UnsupportedConstType => "unsupported const type",
            Self::ValidationStorageLimit => "validation analysis storage limit exceeded",
            Self::ValidationWorkLimit => "validation analysis work limit exceeded",
        }
    }
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl From<RejectionReason> for BytecodeError {
    fn from(reason: RejectionReason) -> Self {
        Self::InvalidSection(reason.message().into())
    }
}
