use super::TokenKind;

impl TokenKind {
    /// Returns `true` if this token is trivia (whitespace, comment, or pragma).
    #[inline]
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::Pragma
        )
    }

    /// Returns `true` if this token is a keyword.
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::KwProgram
                | Self::KwEndProgram
                | Self::KwTestProgram
                | Self::KwEndTestProgram
                | Self::KwFunction
                | Self::KwEndFunction
                | Self::KwFunctionBlock
                | Self::KwEndFunctionBlock
                | Self::KwTestFunctionBlock
                | Self::KwEndTestFunctionBlock
                | Self::KwClass
                | Self::KwEndClass
                | Self::KwMethod
                | Self::KwEndMethod
                | Self::KwProperty
                | Self::KwEndProperty
                | Self::KwInterface
                | Self::KwEndInterface
                | Self::KwNamespace
                | Self::KwEndNamespace
                | Self::KwUsing
                | Self::KwAction
                | Self::KwEndAction
                | Self::KwVar
                | Self::KwEndVar
                | Self::KwVarInput
                | Self::KwVarOutput
                | Self::KwVarInOut
                | Self::KwVarTemp
                | Self::KwVarGlobal
                | Self::KwVarExternal
                | Self::KwVarAccess
                | Self::KwVarConfig
                | Self::KwVarStat
                | Self::KwConstant
                | Self::KwRetain
                | Self::KwNonRetain
                | Self::KwPersistent
                | Self::KwPublic
                | Self::KwPrivate
                | Self::KwProtected
                | Self::KwInternal
                | Self::KwFinal
                | Self::KwAbstract
                | Self::KwOverride
                | Self::KwType
                | Self::KwEndType
                | Self::KwStruct
                | Self::KwEndStruct
                | Self::KwOverlap
                | Self::KwUnion
                | Self::KwEndUnion
                | Self::KwArray
                | Self::KwOf
                | Self::KwString
                | Self::KwWString
                | Self::KwPointer
                | Self::KwRef
                | Self::KwRefTo
                | Self::KwTo
                | Self::KwExtends
                | Self::KwImplements
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwIf
                | Self::KwThen
                | Self::KwElsif
                | Self::KwElse
                | Self::KwEndIf
                | Self::KwCase
                | Self::KwEndCase
                | Self::KwFor
                | Self::KwEndFor
                | Self::KwBy
                | Self::KwDo
                | Self::KwWhile
                | Self::KwEndWhile
                | Self::KwRepeat
                | Self::KwUntil
                | Self::KwEndRepeat
                | Self::KwReturn
                | Self::KwExit
                | Self::KwContinue
                | Self::KwJmp
                | Self::KwStep
                | Self::KwEndStep
                | Self::KwInitialStep
                | Self::KwTransition
                | Self::KwEndTransition
                | Self::KwFrom
                | Self::KwAnd
                | Self::KwOr
                | Self::KwXor
                | Self::KwNot
                | Self::KwMod
                | Self::KwBool
                | Self::KwSInt
                | Self::KwInt
                | Self::KwDInt
                | Self::KwLInt
                | Self::KwUSInt
                | Self::KwUInt
                | Self::KwUDInt
                | Self::KwULInt
                | Self::KwReal
                | Self::KwLReal
                | Self::KwByte
                | Self::KwWord
                | Self::KwDWord
                | Self::KwLWord
                | Self::KwTime
                | Self::KwLTime
                | Self::KwDate
                | Self::KwLDate
                | Self::KwTimeOfDay
                | Self::KwLTimeOfDay
                | Self::KwDateAndTime
                | Self::KwLDateAndTime
                | Self::KwChar
                | Self::KwWChar
                | Self::KwAny
                | Self::KwAnyDerived
                | Self::KwAnyElementary
                | Self::KwAnyMagnitude
                | Self::KwAnyInt
                | Self::KwAnyUnsigned
                | Self::KwAnySigned
                | Self::KwAnyReal
                | Self::KwAnyNum
                | Self::KwAnyDuration
                | Self::KwAnyBit
                | Self::KwAnyChars
                | Self::KwAnyString
                | Self::KwAnyChar
                | Self::KwAnyDate
                | Self::KwTrue
                | Self::KwFalse
                | Self::KwNull
                | Self::KwConfiguration
                | Self::KwEndConfiguration
                | Self::KwResource
                | Self::KwEndResource
                | Self::KwOn
                | Self::KwReadWrite
                | Self::KwReadOnly
                | Self::KwTask
                | Self::KwWith
                | Self::KwAt
                | Self::KwEn
                | Self::KwEno
                | Self::KwREdge
                | Self::KwFEdge
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::KwGet
                | Self::KwEndGet
                | Self::KwSet
                | Self::KwEndSet
        )
    }

    /// Returns `true` if this token is a type keyword.
    pub fn is_type_keyword(self) -> bool {
        matches!(
            self,
            Self::KwBool
                | Self::KwSInt
                | Self::KwInt
                | Self::KwDInt
                | Self::KwLInt
                | Self::KwUSInt
                | Self::KwUInt
                | Self::KwUDInt
                | Self::KwULInt
                | Self::KwReal
                | Self::KwLReal
                | Self::KwByte
                | Self::KwWord
                | Self::KwDWord
                | Self::KwLWord
                | Self::KwTime
                | Self::KwLTime
                | Self::KwDate
                | Self::KwLDate
                | Self::KwTimeOfDay
                | Self::KwLTimeOfDay
                | Self::KwDateAndTime
                | Self::KwLDateAndTime
                | Self::KwString
                | Self::KwWString
                | Self::KwChar
                | Self::KwWChar
                | Self::KwAny
                | Self::KwAnyDerived
                | Self::KwAnyElementary
                | Self::KwAnyMagnitude
                | Self::KwAnyInt
                | Self::KwAnyUnsigned
                | Self::KwAnySigned
                | Self::KwAnyReal
                | Self::KwAnyNum
                | Self::KwAnyDuration
                | Self::KwAnyBit
                | Self::KwAnyChars
                | Self::KwAnyString
                | Self::KwAnyChar
                | Self::KwAnyDate
        )
    }
}
