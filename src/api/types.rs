use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParagraphScopeKind {
    TextStyle,
    Ruby,
    InlineBox,
    Decoration,
    Color,
    RichText,
    Link,
    Technical,
    InlineCode,
    AutoSpaceSuppressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParagraphPositionInsertionKind {
    HardBreak,
    InlineObject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParagraphBuildError {
    EmptyScopeStack,
    EmptyInlineObjectReplacementText,
    UnclosedScopes {
        scopes: Vec<ParagraphScopeKind>,
    },
    ClosureScopeBoundary {
        scope: ParagraphScopeKind,
    },
    EmptyScope {
        scope: ParagraphScopeKind,
    },
    ForbiddenPositionInsertion {
        scope: ParagraphScopeKind,
        insertion: ParagraphPositionInsertionKind,
    },
}

impl fmt::Display for ParagraphBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScopeStack => formatter.write_str("cannot close an empty paragraph scope stack"),
            Self::EmptyInlineObjectReplacementText => {
                formatter.write_str("inline object requires non-empty replacement text")
            }
            Self::UnclosedScopes { .. } => {
                formatter.write_str("paragraph builder contains unclosed scopes")
            }
            Self::ClosureScopeBoundary { .. } => {
                formatter.write_str("paragraph builder scope stack crossed a closure boundary")
            }
            Self::EmptyScope { scope } => {
                write!(formatter, "{scope:?} scope requires non-empty text")
            }
            Self::ForbiddenPositionInsertion { scope, insertion } => {
                write!(formatter, "cannot insert {insertion:?} inside {scope:?}")
            }
        }
    }
}

impl std::error::Error for ParagraphBuildError {}
