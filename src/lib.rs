//! Strict Kotlin-to-Rust translation crate for Tiqian.
//!
//! Translation modules are added only after their corresponding Kotlin source
//! files and direct dependencies have been mapped.

pub mod common;
#[cfg(not(target_arch = "wasm32"))]
mod mimalloc;

pub mod core {
    pub mod east_asian_spacing;
    pub mod east_asian_spacing_data;
    pub mod geometry;
    pub mod int_range;
    pub mod layout_model;
    pub mod layout_queries;
    pub mod source_interaction_boundaries;
    pub mod text;
    pub mod text_model;
    pub mod unicode_script_evidence;
    pub mod unicode_word_character;
    pub mod units;
}

pub mod clreq {
    pub mod bopomofo_reading;
    pub mod clreq_profile;
    pub mod number_symbol_cohesion;
}

pub mod font {
    pub mod font_metrics;
    pub mod font_policy;
    pub mod unicode_emoji_style_variation_data;
}

pub mod linebreak {
    pub mod english_hyphenation;
    pub mod hyphenation;
    pub mod line_break;
    pub mod unicode_punctuation_line_break;
}

pub mod shaping {
    pub mod replayable_font_backend;
    pub mod text_shaper;
}

pub mod layout {
    pub mod annotation_geometry_stage;
    pub mod cluster_role_resolution;
    pub mod contextual_quote_role_resolver;
    pub mod default_hyphenator;
    pub mod justifier;
    pub mod kinsoku_rule;
    pub mod layout_debug_assembly;
    pub mod line_adjustment_stage;
    pub mod line_break_planning_stage;
    pub mod line_breaker;
    pub mod line_geometry_stage;
    pub mod line_optimization;
    pub mod line_repair;
    pub mod paragraph_dp_line_breaker;
    pub mod paragraph_layout_engine;
    pub mod paragraph_shaping_stage;
    pub mod prepared_paragraph;
    pub mod progressive_break_decisions;
    pub mod punctuation_geometry_ledger;
    pub mod punctuation_geometry_stage;
    pub mod punctuation_model;
    pub mod quote_pair_analyzer;
    pub mod unicode_punctuation_boundary_resolver;
    pub mod width_independent_annotation_cache;
}
