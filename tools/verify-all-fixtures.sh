#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd)
fixtures=(
    basic-pause-stop
    ellipsis-and-dash
    nested-quotes
    contextual-dash-ellipsis
    parenthetical-dash-pairs
    adjacent-punctuation-spacing
    contextual-curly-quotes
    mixed-script-quote-paragraph-language
    adjacent-curly-quote-list-context
    mi10s-adjacent-curly-quote-wrap
    unmatched-curly-quotes
    fallback-roles
    ascii-brackets-in-cjk
    mi10s-western-bracket-citation-wrap
    bibliographic-numeric-locator-break
    greedy-multi-line
    kinsoku-carry-previous
    kinsoku-push-in
    lookahead-future-push-in
    lookahead-avoids-repair
    ascii-point-mark-in-cjk
    ascii-point-mark-impossible-measure
    line-end-kinsoku
    justify-cjk-paragraph
    justify-mixed-paragraph
    justify-unbreakable-number-symbol
    real-paragraph-1
    first-line-indent
    adaptive-short-line-indent
    indent-opening-quote
    latin-word-wrap
    latin-camelcase
    latin-existing-hyphen
    latin-hard-break
    latin-opaque-url-token
    zero-width-space-soft-break
    western-hyphenation
    progressive-technical-inline
    progressive-technical-hash-fill
    progressive-technical-alpha-numeric
    progressive-technical-current-line-emergency
    mandatory-single-newline
    mandatory-blank-lines
    mandatory-leading-trailing-newline
    mandatory-crlf
    mandatory-wraps-long-line
    emphasis-marks
    ruby-line-height
    bopomofo-tone-em-box
    interlinear-lines
    mourning-frame
)

for fixture in "${fixtures[@]}"; do
    bash "$script_dir/verify-fixture.sh" "$fixture"
done

printf 'all %d fixtures: golden matched\n' "${#fixtures[@]}"