//! Strict Kotlin-to-Rust translation crate for Tiqian.
//!
//! Translation modules are added only after their corresponding Kotlin source
//! files and direct dependencies have been mapped.

pub mod common;

#[allow(non_snake_case)]
pub mod org {
    #[allow(non_snake_case)]
    pub mod tiqian {
        #[allow(non_snake_case)]
        pub mod core {
            pub mod EastAsianSpacing;
            pub mod EastAsianSpacingData;
            pub mod Geometry;
            pub mod IntRange;
            pub mod LayoutModel;
            pub mod LayoutQueries;
            pub mod SourceInteractionBoundaries;
            pub mod Text;
            pub mod TextModel;
            pub mod UnicodeScriptEvidence;
            pub mod UnicodeWordCharacter;
            pub mod Units;
        }

        #[allow(non_snake_case)]
        pub mod clreq {
            pub mod BopomofoReading;
            pub mod ClreqProfile;
            pub mod NumberSymbolCohesion;
        }

        #[allow(non_snake_case)]
        pub mod font {
            pub mod FontMetrics;
            pub mod FontPolicy;
            pub mod UnicodeEmojiStyleVariationData;
        }

        #[allow(non_snake_case)]
        pub mod linebreak {
            pub mod EnglishHyphenation;
            pub mod Hyphenation;
            pub mod LineBreak;
            pub mod UnicodePunctuationLineBreak;
        }

        #[allow(non_snake_case)]
        pub mod shaping {
            pub mod ReplayableFontBackend;
            pub mod TextShaper;
        }

        #[allow(non_snake_case)]
        pub mod layout {
            pub mod AnnotationGeometryStage;
            pub mod ClusterRoleResolution;
            pub mod ContextualQuoteRoleResolver;
            pub mod DefaultHyphenator;
            pub mod Justifier;
            pub mod KinsokuRule;
            pub mod LayoutDebugAssembly;
            pub mod LineAdjustmentStage;
            pub mod LineBreakPlanningStage;
            pub mod LineBreaker;
            pub mod LineGeometryStage;
            pub mod LineOptimization;
            pub mod LineRepair;
            pub mod ParagraphDpLineBreaker;
            pub mod ParagraphLayoutEngine;
            pub mod ParagraphShapingStage;
            pub mod PreparedParagraph;
            pub mod ProgressiveBreakDecisions;
            pub mod PunctuationGeometryLedger;
            pub mod PunctuationGeometryStage;
            pub mod PunctuationModel;
            pub mod QuotePairAnalyzer;
            pub mod UnicodePunctuationBoundaryResolver;
            pub mod WidthIndependentAnnotationCache;
        }
    }
}
