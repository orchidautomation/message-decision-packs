use crate::models::{Card, CardKind, Manifest};
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeSet;

pub(crate) const README_INVENTORY_CONTRACT: &str = "mdp.readme-inventory.v1";
pub(crate) const README_OWNERSHIP_BEGIN: &str = "<!-- mdp:readme-ownership v1 begin -->";
pub(crate) const README_OWNERSHIP_END: &str = "<!-- mdp:readme-ownership v1 end -->";
pub(crate) const README_INVENTORY_BEGIN: &str = "<!-- mdp:readme-inventory v1 begin -->";
pub(crate) const README_INVENTORY_END: &str = "<!-- mdp:readme-inventory v1 end -->";
pub(crate) const README_MARKER_DIAGNOSTIC: &str = "README machine-owned markers are malformed; expected zero or exactly one non-overlapping begin/end pair for each generated region";
pub(crate) const README_FENCE_DIAGNOSTIC: &str = "README refresh cannot append a missing machine-owned region after an unterminated Markdown fence";

pub(crate) fn render_pack_readme(
    manifest: &Manifest,
    cards: &[&Card],
    source_ledger: &Value,
    prompt_ids: &[String],
) -> String {
    let mut out = String::new();
    line(&mut out, &format!("# {}", manifest.name));
    line(&mut out, "");
    out.push_str(&render_ownership_block());
    line(&mut out, "");
    section(&mut out, "Authority");
    line(
        &mut out,
        "This README is orientation only. The manifest, referenced card entries, source ledger, contracts, and explicit gaps remain the machine authority. README prose cannot satisfy readiness or override structured authority.",
    );
    line(
        &mut out,
        "The inventory block below is a machine-generated projection of loaded structured authority; refresh it with `mdp readme refresh` and never hand-maintain its counts.",
    );

    section(&mut out, "Thesis");
    line(
        &mut out,
        manifest
            .description
            .as_deref()
            .unwrap_or("A local Message Decision Pack."),
    );
    if manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal")
    {
        line(
            &mut out,
            "This public sample is synthetic review support. It does not certify compliance, approve regulated-data handling, replace legal or procurement review, or authorize proposal submission.",
        );
    }

    section(&mut out, "Actors");
    for persona in &manifest.personas {
        bullet(&mut out, persona);
    }

    section(&mut out, "ICP and Fit Authority");
    let mut fit_rule_ids = BTreeSet::new();
    for card in cards.iter().filter(|card| card.kind == CardKind::FitRules) {
        if fit_rule_ids.insert(&card.id) {
            bullet(
                &mut out,
                &format!("`cards/{}.yaml`: {}", card.id, card.title),
            );
        }
    }
    if fit_rule_ids.is_empty() {
        bullet(
            &mut out,
            "No `fit-rules` card is declared; use the profile's structured decision criteria and explicit gaps.",
        );
    }

    section(&mut out, "Supported Jobs");
    for job in &manifest.jobs {
        bullet(
            &mut out,
            &format!(
                "`{}`: {}",
                job.id,
                job.label.as_deref().unwrap_or("Canonical pack job")
            ),
        );
    }

    section(&mut out, "Decision Flow");
    for step in [
        "Select one exact canonical job.",
        "Inspect its resolved product foundation and diagnostics.",
        "Load only the referenced cards, entries, contracts, sources, and gaps.",
        "Use detached prospect input only when the selected job has no direct or transitive Decision Input Contract; governed jobs require the exact normalized envelope and lineage artifacts.",
        "Treat raw prompt output as untrusted. Only a successful validation receipt bound to the exact pack, prompt, job when applicable, validator inputs, and output bytes may provide prompt-output decision-trace authority.",
        "Stop on blocked authority; never fill a gap from this README.",
        "Apply the job output and review boundaries before using the result.",
    ] {
        bullet(&mut out, step);
    }

    section(&mut out, "Boundaries");
    let proposal = manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal");
    let mut boundary_ids = BTreeSet::new();
    for card in cards.iter().filter(|card| {
        matches!(
            card.kind,
            CardKind::AvoidRules | CardKind::ChannelPolicies | CardKind::OutputRules
        ) || (proposal
            && matches!(
                card.id.as_str(),
                "proposal-boundaries" | "compliance-boundaries"
            ))
    }) {
        if boundary_ids.insert(&card.id) {
            bullet(
                &mut out,
                &format!("`cards/{}.yaml`: {}", card.id, card.title),
            );
        }
    }

    section(&mut out, "Sources");
    if let Some(sources) = source_ledger["sources"].as_array() {
        for source in sources {
            if let Some(id) = source["id"].as_str() {
                let locator = source["locator"].as_str().unwrap_or("locator not recorded");
                bullet(&mut out, &format!("`{id}`: {locator}"));
            }
        }
    }

    section(&mut out, "Prompts");
    for prompt_id in prompt_ids {
        bullet(&mut out, &format!("`{prompt_id}`"));
    }

    section(&mut out, "Commands");
    bullet(&mut out, "`mdp --json validate --dir .`");
    for job in &manifest.jobs {
        bullet(
            &mut out,
            &format!("`mdp --json skills --job {} --dir .`", job.id),
        );
        bullet(
            &mut out,
            &format!("`mdp --json requirements --job {} --dir .`", job.id),
        );
    }
    bullet(&mut out, "`mdp --json readme check --dir .`");
    bullet(&mut out, "`mdp --json readme refresh --dir .`");
    bullet(&mut out, "`mdp --json schema prompt-output-validation-v1`");

    section(&mut out, "Gaps");
    for card in cards.iter().filter(|card| card.kind == CardKind::Gaps) {
        for entry in &card.entries {
            bullet(&mut out, &format!("{}: {}", entry.title, entry.body));
        }
    }

    out.push('\n');
    out.push_str(&render_inventory_block(
        manifest,
        cards,
        source_ledger,
        prompt_ids,
    ));
    out
}

/// Render the small machine-owned ownership legend. Everything outside this
/// block and the inventory block is explicitly human-owned prose that refresh
/// preserves but does not semantically review.
pub(crate) fn render_ownership_block() -> String {
    let mut out = String::new();
    line(&mut out, README_OWNERSHIP_BEGIN);
    line(&mut out, "");
    line(&mut out, "## README ownership");
    line(&mut out, "");
    bullet(
        &mut out,
        "Machine-owned: this ownership legend and the marker-delimited Inventory block. `mdp readme refresh` may replace only those regions.",
    );
    bullet(
        &mut out,
        "Human-owned: every other README byte. Refresh preserves that prose without reviewing its thesis, claims, source interpretation, or gaps.",
    );
    line(&mut out, README_OWNERSHIP_END);
    out
}

/// Render the deterministic, marker-delimited inventory block. Along with the
/// ownership legend, this is a machine-owned README region; refresh preserves
/// arbitrary human orientation outside those two regions.
pub(crate) fn render_inventory_block(
    _manifest: &Manifest,
    cards: &[&Card],
    source_ledger: &Value,
    prompt_ids: &[String],
) -> String {
    let mut out = String::new();
    line(&mut out, README_INVENTORY_BEGIN);
    line(&mut out, "");
    line(&mut out, "## Inventory");
    line(&mut out, "");
    line(
        &mut out,
        "Machine-generated from loaded structured authority. Do not edit by hand; run `mdp readme refresh` to update. This block is a projection of the manifest, cards, sources, and prompts; it cannot satisfy a product-foundation facet, close a gap, or override structured authority.",
    );
    line(&mut out, "");

    let mut sorted_cards = cards.iter().collect::<Vec<_>>();
    sorted_cards.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| card_kind_token(&left.kind).cmp(&card_kind_token(&right.kind)))
    });
    let total_entries = sorted_cards
        .iter()
        .map(|card| card.entries.len())
        .sum::<usize>();
    let source_ids = source_ids(source_ledger);
    let mut sorted_prompts = prompt_ids.to_vec();
    sorted_prompts.sort();
    sorted_prompts.dedup();

    line(&mut out, &format!("- cards: {}", sorted_cards.len()));
    line(&mut out, &format!("- card entries: {total_entries}"));
    line(&mut out, &format!("- prompts: {}", sorted_prompts.len()));
    line(&mut out, &format!("- sources: {}", source_ids.len()));

    line(&mut out, "");
    line(&mut out, "### Card entries");
    line(&mut out, "");
    if sorted_cards.is_empty() {
        bullet(&mut out, "No cards declared.");
    }
    for card in &sorted_cards {
        bullet(
            &mut out,
            &format!(
                "`{}` ({}): {} entries",
                card.id,
                card_kind_token(&card.kind),
                card.entries.len()
            ),
        );
    }

    line(&mut out, "");
    line(&mut out, "### Sources");
    line(&mut out, "");
    if source_ids.is_empty() {
        bullet(&mut out, "No sources declared.");
    }
    for id in &source_ids {
        bullet(&mut out, &format!("`{id}`"));
    }

    line(&mut out, "");
    line(&mut out, "### Prompts");
    line(&mut out, "");
    if sorted_prompts.is_empty() {
        bullet(&mut out, "No prompts declared.");
    }
    for id in &sorted_prompts {
        bullet(&mut out, &format!("`{id}`"));
    }

    line(&mut out, README_INVENTORY_END);
    out
}

/// Extract the owned inventory block, including its begin and end markers and
/// the single line terminator after the end marker. Returns `None` when the
/// README is legacy orientation-only prose without the owned marker, leaving it
/// unassessed and unable to affect pack readiness. The captured string equals
/// the freshly rendered block byte-for-byte for unchanged authority.
pub(crate) fn extract_inventory_block(readme: &str) -> Option<String> {
    extract_marked_block(readme, README_INVENTORY_BEGIN, README_INVENTORY_END)
}

pub(crate) fn extract_ownership_block(readme: &str) -> Option<String> {
    extract_marked_block(readme, README_OWNERSHIP_BEGIN, README_OWNERSHIP_END)
}

/// Replace the owned inventory block in `readme` with `fresh_block`. When no
/// owned block is present, append the block at the end as an explicit migration
/// from legacy orientation-only prose.
pub(crate) fn replace_inventory_block(readme: &str, fresh_block: &str) -> String {
    replace_marked_block(
        readme,
        fresh_block,
        README_INVENTORY_BEGIN,
        README_INVENTORY_END,
    )
}

/// Refresh both machine-owned README regions. A legacy README is migrated by
/// prepending the ownership legend and appending the generated inventory; its
/// existing human prose remains byte-for-byte intact between those additions.
pub(crate) fn replace_readme_regions(
    readme: &str,
    fresh_inventory: &str,
) -> Result<String, &'static str> {
    validate_readme_regions(readme)?;
    if extract_inventory_block(readme).is_none() && open_fence_at_eof(readme).is_some() {
        // Inventory migration appends after every existing human-owned byte.
        // Appending while a valid fence is open would hide the generated
        // markers inside human code and make every later refresh append again.
        // Refuse before constructing or writing a changed README instead.
        return Err(README_FENCE_DIAGNOSTIC);
    }
    let ownership = render_ownership_block();
    let with_ownership = if extract_ownership_block(readme).is_some() {
        replace_marked_block(
            readme,
            &ownership,
            README_OWNERSHIP_BEGIN,
            README_OWNERSHIP_END,
        )
    } else {
        insert_ownership_block(readme, &ownership)
    };
    Ok(replace_inventory_block(&with_ownership, fresh_inventory))
}

pub(crate) fn human_owned_readme(readme: &str) -> String {
    let without_ownership =
        remove_marked_block(readme, README_OWNERSHIP_BEGIN, README_OWNERSHIP_END);
    remove_marked_block(
        &without_ownership,
        README_INVENTORY_BEGIN,
        README_INVENTORY_END,
    )
}

fn extract_marked_block(readme: &str, begin_marker: &str, end_marker: &str) -> Option<String> {
    let (begin, end) = inspect_marked_region(readme, begin_marker, end_marker)
        .ok()
        .flatten()?;
    Some(readme[begin..end].to_string())
}

/// Reject ambiguous marker layouts before any generated-region replacement.
/// This keeps malformed or duplicated markers from turning intervening human
/// prose into a machine-owned span on a later refresh.
pub(crate) fn validate_readme_regions(readme: &str) -> Result<(), &'static str> {
    let ownership = inspect_marked_region(readme, README_OWNERSHIP_BEGIN, README_OWNERSHIP_END)?;
    let inventory = inspect_marked_region(readme, README_INVENTORY_BEGIN, README_INVENTORY_END)?;
    if let (Some((left_begin, left_end)), Some((right_begin, right_end))) = (ownership, inventory)
        && left_begin < right_end
        && right_begin < left_end
    {
        return Err(README_MARKER_DIAGNOSTIC);
    }
    Ok(())
}

fn inspect_marked_region(
    readme: &str,
    begin_marker: &str,
    end_marker: &str,
) -> Result<Option<(usize, usize)>, &'static str> {
    let begins = standalone_marker_line_offsets(readme, begin_marker);
    let ends = standalone_marker_line_offsets(readme, end_marker);
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([(begin, _)], [(end, end_after_line)]) if begin < end => {
            Ok(Some((*begin, *end_after_line)))
        }
        _ => Err(README_MARKER_DIAGNOSTIC),
    }
}

/// Locate exact, standalone marker lines outside Markdown fenced code blocks.
/// LF, CRLF, and bare-CR line endings are recognized and included in replacement
/// offsets. Quoted, inline, indented, or fenced marker text remains human prose.
fn standalone_marker_line_offsets(readme: &str, marker: &str) -> Vec<(usize, usize)> {
    let mut offsets = Vec::new();
    let mut scanner = MarkdownFenceScanner::default();
    for (start, content_end, line_end) in markdown_line_offsets(readme) {
        let line = &readme[start..content_end];
        if !scanner.line_is_fenced(line) && line == marker {
            offsets.push((start, line_end));
        }
    }
    offsets
}

fn open_fence_at_eof(readme: &str) -> Option<(char, usize)> {
    let mut scanner = MarkdownFenceScanner::default();
    for (start, content_end, _) in markdown_line_offsets(readme) {
        scanner.line_is_fenced(&readme[start..content_end]);
    }
    scanner
        .fence
        .as_ref()
        .map(|(character, length, _)| (*character, *length))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MarkdownContainerSegment {
    Quote,
    List(usize),
}

type MarkdownContainer = Vec<MarkdownContainerSegment>;

#[derive(Default)]
struct MarkdownFenceScanner {
    fence: Option<(char, usize, MarkdownContainer)>,
    container: MarkdownContainer,
    blank_lines: usize,
    paragraph_open: bool,
    definition_title_pending: Option<MarkdownContainer>,
}

impl MarkdownFenceScanner {
    fn line_is_fenced(&mut self, line: &str) -> bool {
        let pending_definition_title = self.definition_title_pending.take();
        let blank = line.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
        let opening = self
            .fence
            .as_ref()
            .map(|(_, _, container)| container.clone());
        let fenced_projection = opening.as_ref().and_then(|container| {
            project_container_path(line, container).or_else(|| {
                (blank
                    && container
                        .iter()
                        .any(|segment| matches!(segment, MarkdownContainerSegment::List(_))))
                .then_some("")
            })
        });
        if opening.is_some() && fenced_projection.is_none() {
            self.fence = None;
        }
        let (block_content, container) = if let Some(content) = fenced_projection {
            (content, opening.expect("fenced projection has an opening"))
        } else if blank {
            self.blank_lines += 1;
            if self.blank_lines >= 2 {
                self.container.clear();
                self.blank_lines = 0;
            }
            ("", self.container.clone())
        } else {
            self.blank_lines = 0;
            let mut container = self.container.clone();
            let content = if container.is_empty() {
                line
            } else if let Some(content) = project_container_path(line, &container) {
                content
            } else {
                container.clear();
                line
            };
            let content =
                parse_new_container_prefixes(content, &mut container, self.paragraph_open);
            self.container = container.clone();
            (content, container)
        };
        let definition_title_continuation =
            pending_definition_title
                .as_ref()
                .is_some_and(|opening_container| {
                    *opening_container == container
                        && project_container_path(line, opening_container).is_some()
                        && is_link_title_continuation(block_content)
                });
        if self
            .fence
            .as_ref()
            .is_some_and(|(_, _, opening_container)| *opening_container != container)
        {
            self.fence = None;
        }
        let open = self
            .fence
            .as_ref()
            .map(|(character, length, _)| (*character, *length));
        if let Some((character, length, closing)) = fence_delimiter(block_content, open) {
            self.paragraph_open = false;
            if closing {
                self.fence = None;
                return false;
            }
            if self.fence.is_none() {
                self.fence = Some((character, length, container));
            }
            return true;
        }
        if self.fence.is_some() {
            self.paragraph_open = false;
            true
        } else {
            self.definition_title_pending =
                (link_reference_title_state(block_content) == Some(false)).then_some(container);
            self.paragraph_open =
                !definition_title_continuation && line_continues_paragraph(block_content);
            false
        }
    }
}

fn project_container_path<'a>(
    mut line: &'a str,
    container: &[MarkdownContainerSegment],
) -> Option<&'a str> {
    for segment in container {
        line = match segment {
            MarkdownContainerSegment::Quote => strip_one_blockquote_prefix(line)?,
            MarkdownContainerSegment::List(indent) => strip_indent_columns(line, *indent)?,
        };
    }
    Some(line)
}

fn parse_new_container_prefixes<'a>(
    mut line: &'a str,
    container: &mut MarkdownContainer,
    mut paragraph_open: bool,
) -> &'a str {
    loop {
        if let Some(content) = strip_one_blockquote_prefix(line) {
            container.push(MarkdownContainerSegment::Quote);
            line = content;
            paragraph_open = false;
        } else if let Some((indent, content)) = list_item_content(line, paragraph_open) {
            container.push(MarkdownContainerSegment::List(indent));
            line = content;
            paragraph_open = false;
        } else {
            return line;
        }
    }
}

fn strip_one_blockquote_prefix(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let spaces = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    if bytes.get(spaces) != Some(&b'>') {
        return None;
    }
    let mut consumed = spaces + 1;
    if matches!(bytes.get(consumed), Some(b' ' | b'\t')) {
        consumed += 1;
    }
    Some(&line[consumed..])
}

fn list_item_content(line: &str, paragraph_open: bool) -> Option<(usize, &str)> {
    let bytes = line.as_bytes();
    let leading = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    let (marker_end, ordered_start) = match bytes.get(leading)? {
        b'-' | b'+' | b'*' => (leading + 1, None),
        byte if byte.is_ascii_digit() => {
            let mut cursor = leading;
            while bytes.get(cursor).is_some_and(u8::is_ascii_digit) && cursor - leading < 9 {
                cursor += 1;
            }
            if !matches!(bytes.get(cursor), Some(b'.' | b')')) {
                return None;
            }
            (cursor + 1, Some(&line[leading..cursor]))
        }
        _ => return None,
    };
    if paragraph_open && ordered_start.is_some_and(|start| start != "1") {
        return None;
    }
    let mut whitespace_end = marker_end;
    let marker_column = marker_end;
    let mut content_column = marker_column;
    while let Some(byte @ (b' ' | b'\t')) = bytes.get(whitespace_end) {
        content_column = advance_markdown_column(content_column, *byte);
        whitespace_end += 1;
    }
    if whitespace_end == marker_end {
        return None;
    }
    let padding = content_column - marker_column;
    let (content_start, content_indent) = if padding <= 4 || whitespace_end == bytes.len() {
        (whitespace_end, content_column)
    } else {
        let first = bytes[marker_end];
        (
            marker_end + 1,
            advance_markdown_column(marker_column, first),
        )
    };
    Some((content_indent, &line[content_start..]))
}

fn strip_indent_columns(line: &str, required: usize) -> Option<&str> {
    let mut columns = 0;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' => columns += 1,
            b'\t' => columns += 4 - (columns % 4),
            _ => return (columns >= required).then_some(&line[index..]),
        }
        if columns >= required {
            return Some(&line[index + 1..]);
        }
    }
    (columns >= required).then_some("")
}

fn advance_markdown_column(column: usize, byte: u8) -> usize {
    if byte == b'\t' {
        column + 4 - (column % 4)
    } else {
        column + 1
    }
}

fn markdown_indent_columns(line: &str) -> usize {
    let mut columns = 0;
    for byte in line.bytes() {
        match byte {
            b' ' => columns += 1,
            b'\t' => columns += 4 - (columns % 4),
            _ => break,
        }
    }
    columns
}

fn line_continues_paragraph(line: &str) -> bool {
    let blank = line.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    !blank
        && markdown_indent_columns(line) < 4
        && !is_atx_heading(line)
        && !is_thematic_or_setext_line(line)
        && !is_link_reference_definition(line)
        && !line.trim_start_matches([' ', '\t']).starts_with("<!--")
}

fn is_atx_heading(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return false;
    }
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&hashes) && matches!(trimmed.as_bytes().get(hashes), None | Some(b' ' | b'\t'))
}

fn is_thematic_or_setext_line(line: &str) -> bool {
    let trimmed = line.trim_matches([' ', '\t']);
    if !trimmed.is_empty() && trimmed.bytes().all(|byte| byte == b'=') {
        return true;
    }
    for marker in [b'-', b'_', b'*'] {
        let count = trimmed
            .bytes()
            .filter(|byte| !matches!(byte, b' ' | b'\t'))
            .count();
        if count >= 3
            && trimmed
                .bytes()
                .all(|byte| byte == marker || matches!(byte, b' ' | b'\t'))
        {
            return true;
        }
    }
    false
}

fn is_link_reference_definition(line: &str) -> bool {
    link_reference_title_state(line).is_some()
}

fn link_reference_title_state(line: &str) -> Option<bool> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 || !trimmed.starts_with('[') {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut escaped = false;
    for index in 1..bytes.len().min(1001) {
        match bytes[index] {
            b'\\' if !escaped => escaped = true,
            b']' if !escaped => {
                return (index > 1 && bytes.get(index + 1) == Some(&b':'))
                    .then(|| link_definition_suffix_title_state(&trimmed[index + 2..]))
                    .flatten();
            }
            b'[' if !escaped => return None,
            _ => escaped = false,
        }
    }
    None
}

fn link_definition_suffix_title_state(suffix: &str) -> Option<bool> {
    let rest = suffix.trim_start_matches([' ', '\t']);
    if rest.is_empty() {
        return None;
    }
    let destination_end = if let Some(angle) = rest.strip_prefix('<') {
        let mut escaped = false;
        let mut closing = None;
        for (index, character) in angle.char_indices() {
            match character {
                '\\' if !escaped => escaped = true,
                '>' if !escaped => {
                    closing = Some(1 + index + character.len_utf8());
                    break;
                }
                '<' | '\n' | '\r' if !escaped => return None,
                character if character.is_whitespace() && !escaped => return None,
                _ => escaped = false,
            }
        }
        let Some(closing) = closing else {
            return None;
        };
        closing
    } else {
        let mut depth = 0usize;
        let mut escaped = false;
        let mut end = 0usize;
        for (index, character) in rest.char_indices() {
            if character.is_whitespace() && !escaped {
                break;
            }
            match character {
                '\\' if !escaped => escaped = true,
                '(' if !escaped => depth += 1,
                ')' if !escaped && depth > 0 => depth -= 1,
                ')' if !escaped => return None,
                character if character.is_control() && !escaped => return None,
                _ => escaped = false,
            }
            end = index + character.len_utf8();
        }
        if end == 0 || depth != 0 {
            return None;
        }
        end
    };
    let trailing = rest[destination_end..].trim_matches([' ', '\t']);
    if trailing.is_empty() {
        Some(false)
    } else {
        valid_link_title(trailing).then_some(true)
    }
}

fn valid_link_title(title: &str) -> bool {
    let Some(opening) = title.chars().next() else {
        return false;
    };
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return false,
    };
    let mut escaped = false;
    for (index, character) in title[opening.len_utf8()..].char_indices() {
        match character {
            '\\' if !escaped => escaped = true,
            character if character == closing && !escaped => {
                return title[opening.len_utf8() + index + character.len_utf8()..]
                    .trim_matches([' ', '\t'])
                    .is_empty();
            }
            '\n' | '\r' if !escaped => return false,
            _ => escaped = false,
        }
    }
    false
}

fn is_link_title_continuation(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    line.len() - trimmed.len() <= 3 && valid_link_title(trimmed)
}

/// Return byte ranges `(start, content_end, line_end)` for every Markdown line.
/// The content range excludes its line ending while `line_end` includes the
/// exact LF, CRLF, or bare-CR bytes so generated-region replacement is lossless.
pub(crate) fn markdown_line_offsets(markdown: &str) -> Vec<(usize, usize, usize)> {
    let bytes = markdown.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\r' => {
                let line_end = if bytes.get(cursor + 1) == Some(&b'\n') {
                    cursor + 2
                } else {
                    cursor + 1
                };
                lines.push((start, cursor, line_end));
                start = line_end;
                cursor = line_end;
            }
            b'\n' => {
                let line_end = cursor + 1;
                lines.push((start, cursor, line_end));
                start = line_end;
                cursor = line_end;
            }
            _ => cursor += 1,
        }
    }
    if start < bytes.len() {
        lines.push((start, bytes.len(), bytes.len()));
    }
    lines
}

pub(crate) fn fence_delimiter(
    line: &str,
    open: Option<(char, usize)>,
) -> Option<(char, usize, bool)> {
    let leading_spaces = line
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if leading_spaces > 3 {
        return None;
    }
    let rest = &line[leading_spaces..];
    let character = rest.chars().next()?;
    if !matches!(character, '`' | '~') {
        return None;
    }
    let length = rest
        .chars()
        .take_while(|candidate| *candidate == character)
        .count();
    if length < 3 {
        return None;
    }
    match open {
        Some((open_character, open_length)) => {
            let suffix = &rest[length..];
            let closing = character == open_character
                && length >= open_length
                && suffix.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
            closing.then_some((character, length, true))
        }
        None => {
            let info = &rest[length..];
            if character == '`' && info.contains('`') {
                // CommonMark forbids backticks in a backtick fence's info
                // string. Treat this as ordinary prose so subsequent marker
                // lines remain visible instead of being hidden by a fake fence.
                None
            } else {
                Some((character, length, false))
            }
        }
    }
}

fn insert_ownership_block(readme: &str, ownership: &str) -> String {
    if readme.starts_with("# ") {
        if let Some(line_end) = readme.find('\n') {
            let insertion = line_end + 1;
            let mut out = String::with_capacity(readme.len() + ownership.len() + 1);
            out.push_str(&readme[..insertion]);
            out.push('\n');
            out.push_str(ownership);
            out.push('\n');
            out.push_str(&readme[insertion..]);
            return out;
        }
    }
    let mut out = ownership.to_string();
    if !readme.is_empty() {
        out.push('\n');
        out.push_str(readme);
    }
    out
}

fn replace_marked_block(
    readme: &str,
    fresh_block: &str,
    begin_marker: &str,
    end_marker: &str,
) -> String {
    match marked_block_offsets(readme, begin_marker, end_marker) {
        Some((begin, end)) => {
            let mut out = String::with_capacity(readme.len() + fresh_block.len());
            out.push_str(&readme[..begin]);
            out.push_str(fresh_block);
            out.push_str(&readme[end..]);
            out
        }
        None => {
            // Legacy prose is human-owned byte-for-byte, including trailing
            // whitespace and newline choices. Append a separator after those
            // bytes rather than normalizing or removing any of them.
            let mut out = readme.to_string();
            if !out.is_empty() {
                if out.ends_with("\n\n") {
                    // Existing blank-line separation is sufficient.
                } else if out.ends_with('\n') {
                    out.push('\n');
                } else {
                    out.push_str("\n\n");
                }
            }
            out.push_str(fresh_block);
            out
        }
    }
}

fn remove_marked_block(readme: &str, begin_marker: &str, end_marker: &str) -> String {
    let Some((begin, end)) = marked_block_offsets(readme, begin_marker, end_marker) else {
        return readme.to_string();
    };
    let mut out = String::with_capacity(readme.len() - (end - begin));
    out.push_str(&readme[..begin]);
    out.push_str(&readme[end..]);
    out
}

fn marked_block_offsets(
    readme: &str,
    begin_marker: &str,
    end_marker: &str,
) -> Option<(usize, usize)> {
    inspect_marked_region(readme, begin_marker, end_marker)
        .ok()
        .flatten()
}

fn source_ids(source_ledger: &Value) -> Vec<String> {
    let yaml_value: YamlValue = serde_yaml::to_value(source_ledger).unwrap_or(YamlValue::Null);
    yaml_value["sources"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str().map(str::to_string))
        .collect()
}

fn card_kind_token(kind: &CardKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn section(out: &mut String, title: &str) {
    line(out, "");
    line(out, &format!("## {title}"));
    line(out, "");
}

fn bullet(out: &mut String, value: &str) {
    line(out, &format!("- {value}"));
}

fn line(out: &mut String, value: &str) {
    out.push_str(value);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CardKind, Entry, Manifest};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn manifest_with(name: &str) -> Manifest {
        Manifest {
            format: "mdp.v0".into(),
            id: "pack".into(),
            name: name.into(),
            version: "0.1.0".into(),
            description: Some("A pack".into()),
            target: None,
            profile: None,
            personas: vec!["PMM".into()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![],
            lead_input_requirements: Default::default(),
            qualification_gates: None,
            required_primitives: vec![],
            primitive_map: BTreeMap::new(),
            decision_input_contracts: vec![],
            input_contracts: vec![],
            jobs: vec![],
            profile_eval: Default::default(),
            cards: vec![],
            policy: crate::models::Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route: 4,
                json_contract: "mdp.json.v0".into(),
                no_auth_required: true,
            },
            provenance: crate::models::Provenance {
                owner: "test".into(),
                created_by: "test".into(),
                notes: vec![],
            },
        }
    }

    fn card(id: &str, kind: CardKind, entries: usize) -> Card {
        Card {
            id: id.into(),
            kind,
            title: format!("{id} title"),
            description: "desc".into(),
            personas: vec![],
            tags: vec![],
            entries: (0..entries)
                .map(|index| Entry {
                    id: format!("{id}-{index}"),
                    title: format!("entry {index}"),
                    body: "body".into(),
                    applies_to: vec![],
                    scope: BTreeMap::new(),
                    evidence: vec![],
                    avoid: vec![],
                    exact_paragraphs: None,
                    constraints: Default::default(),
                    metadata: BTreeMap::new(),
                })
                .collect(),
        }
    }

    fn source_ledger(ids: &[&str]) -> Value {
        json!({
            "sources": ids.iter().map(|id| json!({"id": id, "locator": "synthetic"})).collect::<Vec<_>>()
        })
    }

    #[test]
    fn bundled_starter_readmes_mark_machine_and_human_ownership() {
        for readme in [
            include_str!("../../plugin/assets/templates/basic/.mdp/README.md"),
            include_str!("../../plugin/assets/templates/proposal/.mdp/README.md"),
        ] {
            assert!(extract_ownership_block(readme).is_some());
            assert!(extract_inventory_block(readme).is_some());
            assert!(readme.contains("Human-owned: every other README byte"));
        }
    }

    #[test]
    fn inventory_block_is_deterministic_and_byte_stable_for_unchanged_authority() {
        let manifest = manifest_with("Deterministic Pack");
        let cards = vec![
            card("hooks", CardKind::Hooks, 10),
            card("pains", CardKind::Pains, 6),
            card("positioning", CardKind::Positioning, 3),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity", "public-web"]);
        let prompts = vec![
            "generate-outbound-copy".to_string(),
            "review-outbound-copy".to_string(),
        ];

        let first = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        let second = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        assert_eq!(first, second, "byte-stable for unchanged authority");
        assert!(first.contains(README_INVENTORY_BEGIN));
        assert!(first.contains(README_INVENTORY_END));
        assert!(first.contains("- cards: 3"));
        assert!(first.contains("- card entries: 19"));
        assert!(first.contains("- prompts: 2"));
        assert!(first.contains("- sources: 2"));
        assert!(first.contains("`pains` (pains): 6 entries"));
        assert!(first.contains("`hooks` (hooks): 10 entries"));
        // cards are sorted by id, so pains precedes positioning precedes hooks? No:
        // sorted ids: hooks, pains, positioning. Confirm ordering is stable.
        let pains_pos = first.find("`pains`").unwrap();
        let hooks_pos = first.find("`hooks`").unwrap();
        assert!(
            hooks_pos < pains_pos,
            "cards sorted by id: hooks before pains"
        );
    }

    #[test]
    fn inventory_block_order_is_independent_of_input_card_order() {
        let manifest = manifest_with("Order Independent");
        let cards_a = vec![
            card("zeta", CardKind::Pains, 1),
            card("alpha", CardKind::Hooks, 2),
        ];
        let cards_b = vec![
            card("alpha", CardKind::Hooks, 2),
            card("zeta", CardKind::Pains, 1),
        ];
        let ledger = source_ledger(&[]);
        let prompts: Vec<String> = vec![];
        let refs_a = cards_a.iter().collect::<Vec<_>>();
        let refs_b = cards_b.iter().collect::<Vec<_>>();
        assert_eq!(
            render_inventory_block(&manifest, &refs_a, &ledger, &prompts),
            render_inventory_block(&manifest, &refs_b, &ledger, &prompts),
        );
    }

    #[test]
    fn render_pack_readme_emits_owned_inventory_block() {
        let manifest = manifest_with("Full Readme");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity"]);
        let prompts = vec!["generate-outbound-copy".to_string()];
        let readme = render_pack_readme(&manifest, &card_refs, &ledger, &prompts);
        assert!(readme.contains("## Authority"));
        assert!(readme.contains(README_OWNERSHIP_BEGIN));
        assert!(readme.contains(README_OWNERSHIP_END));
        assert!(readme.contains("Human-owned: every other README byte"));
        assert!(readme.contains(README_INVENTORY_BEGIN));
        assert!(readme.contains(README_INVENTORY_END));
        let extracted = extract_inventory_block(&readme).expect("block should extract");
        assert!(extracted.contains("`pains` (pains): 6 entries"));
        assert!(extracted.contains("`hooks` (hooks): 10 entries"));
    }

    #[test]
    fn extract_returns_none_for_legacy_readme_without_marker() {
        let legacy = "# Human Pack\n\nOrientation prose only.\n";
        assert!(extract_inventory_block(legacy).is_none());
    }

    #[test]
    fn marker_text_in_inline_quotes_and_fences_remains_human_prose() {
        let human = format!(
            "Inline `{README_OWNERSHIP_BEGIN}` and quoted text:\n> {README_INVENTORY_BEGIN}\n\n```markdown\n{README_OWNERSHIP_BEGIN}\n{README_OWNERSHIP_END}\n{README_INVENTORY_BEGIN}\n{README_INVENTORY_END}\n```\n"
        );
        assert!(validate_readme_regions(&human).is_ok());
        assert!(extract_ownership_block(&human).is_none());
        assert!(extract_inventory_block(&human).is_none());

        let manifest = manifest_with("Quoted Marker Pack");
        let cards = vec![card("pains", CardKind::Pains, 1)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let refreshed = replace_readme_regions(&human, &fresh).expect("refresh legacy prose");
        assert!(
            refreshed.contains(&human),
            "refresh must retain adversarial human prose byte-for-byte"
        );
        assert_eq!(refreshed.matches(README_OWNERSHIP_BEGIN).count(), 3);
        assert_eq!(refreshed.matches(README_INVENTORY_BEGIN).count(), 3);
        assert!(validate_readme_regions(&refreshed).is_ok());
    }

    #[test]
    fn invalid_backtick_info_string_does_not_hide_owned_marker_lines() {
        let readme = format!(
            "```markdown`invalid\n{README_OWNERSHIP_BEGIN}\nlegend\n{README_OWNERSHIP_END}\n```\n"
        );
        assert!(validate_readme_regions(&readme).is_ok());
        let extracted = extract_ownership_block(&readme).expect("visible ownership region");
        assert!(extracted.starts_with(README_OWNERSHIP_BEGIN));
        assert!(extracted.ends_with(&format!("{README_OWNERSHIP_END}\n")));
    }

    #[test]
    fn missing_inventory_refuses_append_after_unterminated_fence() {
        let manifest = manifest_with("Open Fence Pack");
        let cards = vec![card("pains", CardKind::Pains, 1)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        for human in [
            "# Human\n\n```markdown\nkeep these bytes\n",
            "# Human\n\n~~~text\nkeep these bytes\n",
        ] {
            for _ in 0..2 {
                assert_eq!(
                    replace_readme_regions(human, &fresh),
                    Err(README_FENCE_DIAGNOSTIC),
                    "an open fence must fail closed on every refresh"
                );
            }
            assert!(extract_ownership_block(human).is_none());
            assert!(extract_inventory_block(human).is_none());
        }
    }

    #[test]
    fn unicode_whitespace_does_not_close_a_fence() {
        let manifest = manifest_with("Unicode Fence Pack");
        let cards = vec![card("pains", CardKind::Pains, 1)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let human = "# Human\n\n```markdown\nkeep bytes\n```\u{00a0}\n";
        assert_eq!(
            replace_readme_regions(human, &fresh),
            Err(README_FENCE_DIAGNOSTIC),
            "CommonMark closing suffix permits ASCII space/tab only"
        );
        assert!(open_fence_at_eof(human).is_some());
    }

    #[test]
    fn standalone_crlf_marker_lines_are_recognized_with_exact_offsets() {
        let ownership = render_ownership_block().replace('\n', "\r\n");
        let inventory =
            format!("{README_INVENTORY_BEGIN}\r\n## Inventory\r\n{README_INVENTORY_END}\r\n");
        let readme = format!("{ownership}\r\nHuman bytes.\r\n\r\n{inventory}");
        assert!(validate_readme_regions(&readme).is_ok());
        assert_eq!(
            extract_ownership_block(&readme).as_deref(),
            Some(ownership.as_str())
        );
        assert_eq!(
            extract_inventory_block(&readme).as_deref(),
            Some(inventory.as_str())
        );

        let fresh_inventory =
            format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let refreshed = replace_readme_regions(&readme, &fresh_inventory).expect("refresh CRLF");
        assert!(refreshed.contains("Human bytes.\r\n\r\n"));
        assert_eq!(
            extract_ownership_block(&refreshed),
            Some(render_ownership_block())
        );
        assert_eq!(extract_inventory_block(&refreshed), Some(fresh_inventory));
    }

    #[test]
    fn standalone_bare_cr_markers_and_fences_keep_exact_offsets() {
        let ownership = render_ownership_block().replace('\n', "\r");
        let inventory = format!("{README_INVENTORY_BEGIN}\r## Inventory\r{README_INVENTORY_END}\r");
        let readme = format!("{ownership}\rHuman bytes.\r\r{inventory}");
        assert!(validate_readme_regions(&readme).is_ok());
        assert_eq!(
            extract_ownership_block(&readme).as_deref(),
            Some(ownership.as_str())
        );
        assert_eq!(
            extract_inventory_block(&readme).as_deref(),
            Some(inventory.as_str())
        );

        let fenced_markers = format!(
            "# Legacy\r\r```markdown\r{README_OWNERSHIP_BEGIN}\r{README_OWNERSHIP_END}\r{README_INVENTORY_BEGIN}\r{README_INVENTORY_END}\r```\rHuman bytes.\r"
        );
        let manifest = manifest_with("Bare CR Pack");
        let cards = vec![card("pains", CardKind::Pains, 1)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let once = replace_readme_regions(&fenced_markers, &fresh).expect("first refresh");
        assert!(once.contains(&fenced_markers));
        assert!(validate_readme_regions(&once).is_ok());
        let twice = replace_readme_regions(&once, &fresh).expect("second refresh");
        assert_eq!(
            twice, once,
            "bare-CR fenced marker prose must not duplicate regions"
        );
    }

    #[test]
    fn list_container_exit_closes_fence_before_owned_markers() {
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        for prefix in [
            "- example\n  ```markdown\n  unclosed list fence\n",
            "- > ```markdown\n  > unclosed nested fence\n",
            "- outer\n  - ```markdown\n    unclosed nested-list fence\n",
        ] {
            let readme = format!("{prefix}Root prose.\n\n{ownership}\n{inventory}");
            assert!(validate_readme_regions(&readme).is_ok());
            assert_eq!(extract_ownership_block(&readme), Some(ownership.clone()));
            assert_eq!(extract_inventory_block(&readme), Some(inventory.clone()));
            assert!(open_fence_at_eof(&readme).is_none());
        }
        assert!(
            open_fence_at_eof("- ```markdown\n underindented body").is_none(),
            "a bullet continuation includes marker width plus padding"
        );
        assert!(
            open_fence_at_eof("Paragraph\n2. ```markdown\n   body").is_none(),
            "an ordered list not starting at one cannot interrupt a paragraph"
        );
        assert!(
            open_fence_at_eof("Paragraph\n1. ```markdown\n   body").is_some(),
            "an ordered list starting at one may interrupt a paragraph"
        );
        let after_heading =
            format!("# Heading\n2. ```markdown\n   fenced body\n   ```\n{ownership}\n{inventory}");
        assert!(validate_readme_regions(&after_heading).is_ok());
        assert!(open_fence_at_eof(&after_heading).is_none());
        assert_eq!(extract_ownership_block(&after_heading), Some(ownership));
        assert_eq!(extract_inventory_block(&after_heading), Some(inventory));
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let after_definition = format!(
            "[ref]: /url\n2. ```markdown\n   fenced body\n   ```\n{ownership}\n{inventory}"
        );
        assert!(validate_readme_regions(&after_definition).is_ok());
        assert!(open_fence_at_eof(&after_definition).is_none());
        assert_eq!(extract_ownership_block(&after_definition), Some(ownership));
        assert_eq!(extract_inventory_block(&after_definition), Some(inventory));
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let invalid_definition =
            format!("[ref]: <foo bar>\n2. ```markdown\n   body\n   ```\n{ownership}\n{inventory}");
        assert!(validate_readme_regions(&invalid_definition).is_ok());
        assert!(open_fence_at_eof(&invalid_definition).is_some());
        assert_eq!(extract_ownership_block(&invalid_definition), None);
        assert_eq!(extract_inventory_block(&invalid_definition), None);
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let multiline_definition = format!(
            "[ref]: /url\n  \"title\"\n2. ```markdown\n   body\n   ```\n{ownership}\n{inventory}"
        );
        assert!(validate_readme_regions(&multiline_definition).is_ok());
        assert!(open_fence_at_eof(&multiline_definition).is_none());
        assert_eq!(
            extract_ownership_block(&multiline_definition),
            Some(ownership)
        );
        assert_eq!(
            extract_inventory_block(&multiline_definition),
            Some(inventory)
        );
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let quoted_definition = format!(
            "> [ref]: /url\n>   \"title\"\n> 2. ```markdown\n>    body\n>    ```\n{ownership}\n{inventory}"
        );
        assert!(validate_readme_regions(&quoted_definition).is_ok());
        assert!(open_fence_at_eof(&quoted_definition).is_none());
        assert_eq!(extract_ownership_block(&quoted_definition), Some(ownership));
        assert_eq!(extract_inventory_block(&quoted_definition), Some(inventory));
        let ownership = render_ownership_block();
        let inventory = format!("{README_INVENTORY_BEGIN}\n## Inventory\n{README_INVENTORY_END}\n");
        let escaped_definition = format!(
            "> [ref]: /url\n\"title\"\n2. ```markdown\n   visible paragraph\n   ```\n{ownership}\n{inventory}"
        );
        assert!(validate_readme_regions(&escaped_definition).is_ok());
        assert!(open_fence_at_eof(&escaped_definition).is_some());
        assert_eq!(extract_ownership_block(&escaped_definition), None);
        assert_eq!(extract_inventory_block(&escaped_definition), None);
    }

    #[test]
    fn replace_preserves_human_prose_outside_owned_block() {
        let mut human = String::from("# Human Pack\n\n");
        human.push_str("Free-form orientation the author wrote by hand.\n\n");
        human.push_str("<!-- mdp:readme-inventory v1 begin -->\n## Inventory\n\n- stale: yes\n\n<!-- mdp:readme-inventory v1 end -->\n");
        human.push_str("\nMore human notes after the block.\n");

        let manifest = manifest_with("Replace Pack");
        let cards = vec![card("pains", CardKind::Pains, 6)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let updated = replace_inventory_block(&human, &fresh);

        assert!(updated.contains("Free-form orientation the author wrote by hand."));
        assert!(updated.contains("More human notes after the block."));
        assert!(!updated.contains("stale: yes"));
        assert!(updated.contains("`pains` (pains): 6 entries"));
        // The owned block appears exactly once after replacement.
        assert_eq!(
            updated.matches(README_INVENTORY_BEGIN).count(),
            1,
            "exactly one owned block"
        );
    }

    #[test]
    fn replace_appends_block_when_absent_to_migrate_legacy_readme() {
        let human = "# Legacy Pack\n\nOrientation only.\n";
        let manifest = manifest_with("Migrate Pack");
        let cards = vec![card("pains", CardKind::Pains, 6)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let updated = replace_inventory_block(&human, &fresh);
        assert!(updated.starts_with("# Legacy Pack"));
        assert!(updated.contains(README_INVENTORY_BEGIN));
        assert_eq!(updated.matches(README_INVENTORY_BEGIN).count(), 1);
    }

    #[test]
    fn appending_missing_inventory_preserves_every_legacy_trailing_byte() {
        let manifest = manifest_with("Trailing Byte Pack");
        let cards = vec![card("pains", CardKind::Pains, 1)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let cases = [
            "# Legacy\nprose without newline",
            "# Legacy\nprose with one newline\n",
            "# Legacy\nprose with three newlines\n\n\n",
            "# Legacy\nprose with trailing whitespace\n \t\n",
        ];
        for human in cases {
            let updated = replace_inventory_block(human, &fresh);
            assert!(
                updated.as_bytes().starts_with(human.as_bytes()),
                "legacy bytes must remain an exact prefix: {human:?}"
            );
            assert_eq!(updated.matches(README_INVENTORY_BEGIN).count(), 1);
        }
    }

    #[test]
    fn refreshing_twice_is_byte_stable_when_authority_unchanged() {
        let manifest = manifest_with("Stable Refresh");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity"]);
        let prompts = vec!["generate-outbound-copy".to_string()];
        let fresh = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        let mut readme = "# Stable Pack\n\nOrientation.\n".to_string();
        readme = replace_inventory_block(&readme, &fresh);
        let once = readme.clone();
        readme = replace_inventory_block(&readme, &fresh);
        assert_eq!(once, readme, "second refresh is byte-stable");
    }

    #[test]
    fn inventory_drift_regression_fixture_catches_mismatched_counts() {
        // Reproduces the audited mismatch class: a README claims pains 14 and
        // hooks 21, while the loaded structured authority reports 6 and 10. The
        // fresh block must disagree and the stale authored block must fail to
        // equal the projection.
        let manifest = manifest_with("Mismatched Pack");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let mut stale = String::new();
        line(&mut stale, README_INVENTORY_BEGIN);
        line(&mut stale, "");
        line(&mut stale, "## Inventory");
        line(&mut stale, "");
        bullet(&mut stale, "- pains: 14 entries");
        bullet(&mut stale, "- hooks: 21 entries");
        line(&mut stale, "");
        line(&mut stale, README_INVENTORY_END);
        assert_ne!(
            stale, fresh,
            "stale authored counts must not match the projection"
        );
        assert!(fresh.contains("`pains` (pains): 6 entries"));
        assert!(fresh.contains("`hooks` (hooks): 10 entries"));
    }
}
